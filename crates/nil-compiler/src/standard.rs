//! Compiler-versioned source std. Type discovery selects an overload; normal HIR
//! validation remains authoritative. No purity or ownership annotations are added.
use crate::syntax::{self, InstructionKind as K, Parameter};
use nil_hir::{Diagnostic, Phase, Span, Type};
use std::collections::BTreeMap;

fn error(span: Span, message: &str) -> Diagnostic {
    Diagnostic::new("E007", Phase::Check, Some(span), message)
}
struct Loader {
    signatures: BTreeMap<u32, Type>,
    callbacks: BTreeMap<u32, Type>,
    records: Vec<nil_hir::RecordDefinition>,
    cache: BTreeMap<(String, bool), u32>,
    functions: Vec<syntax::Function>,
    next_label: u32,
}
impl Loader {
    fn function(&mut self, name: &str, ty: Type, span: Span) -> Result<u32, Diagnostic> {
        let key = (name.to_owned(), ty == Type::Bytes);
        if let Some(label) = self.cache.get(&key) {
            return Ok(*label);
        }
        let text = match ty {
            Type::Bytes => include_str!("../../../std/bytes.nil"),
            Type::Buffer => include_str!("../../../std/buffer.nil"),
            _ => return Err(error(span, "std sequence must be Bytes or Buffer")),
        };
        let mut module = crate::parser::parse_with_profile(text, crate::SourceProfile::ExprV5)?;
        let index = match name {
            "map" => 0,
            "filter" => 1,
            "fold" => 2,
            _ => return Err(error(span, "unknown std operation")),
        };
        let mut f = module.functions.remove(index);
        let label = self.next_label;
        self.next_label = label
            .checked_add(1)
            .ok_or_else(|| error(span, "std label overflow"))?;
        f.label = label;
        self.signatures.insert(label, f.result_type);
        self.cache.insert(key, label);
        self.functions.push(f);
        Ok(label)
    }
    fn region(
        &mut self,
        r: &mut syntax::Region,
        types: Vec<Type>,
    ) -> Result<Vec<Type>, Diagnostic> {
        let values = self.items(&mut r.instructions, types)?;
        r.results
            .iter()
            .map(|id| {
                values
                    .get(*id as usize)
                    .copied()
                    .ok_or_else(|| error(Span { start: 0, end: 0 }, "undefined std region value"))
            })
            .collect()
    }
    fn items(
        &mut self,
        items: &mut [syntax::Instruction],
        mut types: Vec<Type>,
    ) -> Result<Vec<Type>, Diagnostic> {
        for i in items {
            let span = i.span;
            let get = |id: u32| {
                types
                    .get(id as usize)
                    .copied()
                    .ok_or_else(|| error(span, "undefined std argument"))
            };
            let ty = match &mut i.kind {
                K::Std(name, args) => {
                    let expected = if name == "fold" { 3 } else { 2 };
                    if args.len() != expected {
                        return Err(Diagnostic::new(
                            "E006",
                            Phase::Check,
                            Some(span),
                            "std arity mismatch",
                        )
                        .mismatch(expected, args.len()));
                    }
                    let input = get(args[1])?;
                    let label = self.function(name, input, span)?;
                    let result = self.signatures[&label];
                    i.kind = K::Call(label, args.clone());
                    result
                }
                K::FunctionReference(_) => Type::I64, // Never escapes; specialization checks it.
                K::CallbackCall(reference, _) => match reference {
                    syntax::FunctionReference::Local(label) => *self
                        .signatures
                        .get(label)
                        .ok_or_else(|| error(span, "unknown callback"))?,
                    // Callback result types are installed in the lexical map below.
                    syntax::FunctionReference::Parameter(id) => *self
                        .callbacks
                        .get(id)
                        .ok_or_else(|| error(span, "unknown static parameter"))?,
                    syntax::FunctionReference::Export(..) => {
                        return Err(nil_hir::plugin::error("unlinked std callback"));
                    }
                },
                K::Call(label, _) => *self.signatures.get(label).ok_or_else(|| {
                    Diagnostic::new("E004", Phase::Check, Some(span), "unknown function")
                })?,
                K::Plugin(..) => {
                    return Err(nil_hir::plugin::error("unlinked std module reference"));
                }
                K::LinkedPlugin(p, _) => p.signature().result_type,
                K::Record(ty, _) | K::RecordMap(ty) | K::RecordBuffer(ty, ..) => *ty,
                K::Field(id, name) => {
                    let Type::Record(record, _) = get(*id)? else {
                        return Err(error(span, "field on nonrecord"));
                    };
                    self.records
                        .get(record)
                        .ok_or_else(|| nil_hir::records::error(Some(span), "unknown record type"))?
                        .fields
                        .iter()
                        .find(|f| f.name == *name)
                        .ok_or_else(|| nil_hir::records::error(Some(span), "unknown record field"))?
                        .ty
                }
                K::UpdateField(id, ..) | K::Replace(id, ..) => get(*id)?,
                K::Bytes(_) => Type::Bytes,
                K::Constant(_) | K::Length(_) => Type::I64,
                K::Unsigned(_, ty) => *ty,
                K::Float(_) => Type::F64,
                K::Boolean(_) | K::Compare(..) => Type::Bool,
                K::Binary(_, a, _) => get(*a)?,
                K::Array(ids) => Type::Array(ids.len()),
                K::Repeat(_, n) => Type::Array(*n),
                K::Index(id, _) => match get(*id)? {
                    Type::RecordBuffer(r, n) => Type::Record(r, n),
                    _ => Type::I64,
                },
                K::Intrinsic(op, args) => {
                    let args = args
                        .iter()
                        .map(|id| get(*id))
                        .collect::<Result<Vec<_>, _>>()?;
                    let operation = nil_hir::Operation::Intrinsic {
                        op: *op,
                        arguments: (0..args.len()).map(nil_hir::ValueId).collect(),
                    };
                    nil_hir::operation_type_with_records(
                        &[],
                        &self.records,
                        &operation,
                        &args,
                        Some(span),
                        0,
                    )?
                }
                K::If(_, yes, no) => {
                    let result = self.region(yes, types.clone())?;
                    self.region(no, types.clone())?;
                    *result.first().ok_or_else(|| error(span, "empty branch"))?
                }
                K::Loop(initial, condition, body, finish) => {
                    let inputs = initial
                        .iter()
                        .map(|id| get(*id))
                        .collect::<Result<Vec<_>, _>>()?;
                    self.region(condition, inputs.clone())?;
                    self.region(body, inputs.clone())?;
                    *self
                        .region(finish, inputs)?
                        .first()
                        .ok_or_else(|| error(span, "empty loop finish"))?
                }
                K::Each(input, initial, body, finish) => {
                    let input = get(*input)?;
                    let state = initial
                        .iter()
                        .map(|id| get(*id))
                        .collect::<Result<Vec<_>, _>>()?;
                    let mut body_types = Vec::new();
                    body_types.extend(if let Some(value) = input.map_value() {
                        [Type::Bytes, value]
                    } else {
                        [
                            Type::I64,
                            match input {
                                Type::RecordBuffer(r, n) => Type::Record(r, n),
                                _ => Type::I64,
                            },
                        ]
                    });
                    body_types.extend(state.clone());
                    self.region(body, body_types)?;
                    *self
                        .region(finish, state)?
                        .first()
                        .ok_or_else(|| error(span, "empty each finish"))?
                }
            };
            types.push(ty);
        }
        Ok(types)
    }
}
fn contains(items: &[syntax::Instruction]) -> bool {
    items.iter().any(|i| match &i.kind {
        K::Std(..) => true,
        K::If(_, a, b) => contains(&a.instructions) || contains(&b.instructions),
        K::Loop(_, a, b, c) => [a, b, c].iter().any(|r| contains(&r.instructions)),
        K::Each(_, _, a, b) => contains(&a.instructions) || contains(&b.instructions),
        _ => false,
    })
}
pub(crate) fn load(mut module: syntax::Module) -> Result<syntax::Module, Diagnostic> {
    if !module.functions.iter().any(|f| contains(&f.instructions)) {
        return Ok(module);
    }
    let next_label = module
        .functions
        .iter()
        .map(|f| f.label)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| nil_hir::plugin::error("std label overflow"))?;
    let mut loader = Loader {
        callbacks: BTreeMap::new(),
        signatures: module
            .functions
            .iter()
            .map(|f| (f.label, f.result_type))
            .collect(),
        records: module.records.clone(),
        cache: BTreeMap::new(),
        functions: vec![],
        next_label,
    };
    for f in &mut module.functions {
        let mut inputs = vec![];
        for (id, p) in f.parameters.iter().enumerate() {
            match p {
                Parameter::Value(ty) => inputs.push(*ty),
                Parameter::Function(sig) => {
                    loader.callbacks.insert(id as u32, sig.result);
                    inputs.push(Type::I64);
                }
            }
        }
        loader.items(&mut f.instructions, inputs)?;
        for id in 0..f.parameters.len() {
            loader.callbacks.remove(&(id as u32));
        }
    }
    module.functions.extend(loader.functions);
    Ok(module)
}
