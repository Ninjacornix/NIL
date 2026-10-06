//! Frontend-only static function arguments. The output is strictly first-order.
use crate::syntax::{self, FunctionReference as Ref, InstructionKind as K, Parameter};
use nil_hir::{Diagnostic, Phase, Span};
use std::collections::BTreeMap;

const MAX_SPECIALIZATIONS: usize = 1024;
const MAX_PER_TEMPLATE: usize = 256;
const MAX_FUNCTIONS: usize = 4096;
const MAX_INSTRUCTIONS: usize = 1_048_576;

#[derive(Clone, Copy, Debug)]
enum Binding {
    Value(u32),
    Function(u32),
}
fn error(code: &'static str, span: Span, message: &str) -> Diagnostic {
    Diagnostic::new(code, Phase::Check, Some(span), message)
}
fn value(bindings: &[Binding], id: u32, span: Span) -> Result<u32, Diagnostic> {
    match bindings.get(id as usize) {
        Some(Binding::Value(id)) => Ok(*id),
        Some(Binding::Function(_)) => Err(error(
            "E007",
            span,
            "function reference is not a runtime value",
        )),
        None => Err(error("E005", span, "undefined source value")),
    }
}
fn count(items: &[syntax::Instruction]) -> usize {
    items
        .iter()
        .map(|i| {
            1 + match &i.kind {
                K::If(_, a, b) => count(&a.instructions) + count(&b.instructions),
                K::Loop(_, a, b, c) => {
                    count(&a.instructions) + count(&b.instructions) + count(&c.instructions)
                }
                K::Each(_, _, a, b) => count(&a.instructions) + count(&b.instructions),
                _ => 0,
            }
        })
        .sum()
}
struct Specializer {
    templates: BTreeMap<u32, syntax::Function>,
    instances: BTreeMap<(u32, Vec<u32>), u32>,
    per_template: BTreeMap<u32, usize>,
    functions: Vec<syntax::Function>,
    queue: Vec<(u32, Vec<u32>)>,
    next_label: u32,
    specialized: usize,
    instructions: usize,
}
impl Specializer {
    fn signature(&self, label: u32, span: Span) -> Result<syntax::FunctionSignature, Diagnostic> {
        let f = self
            .templates
            .get(&label)
            .ok_or_else(|| error("E004", span, "unknown function reference"))?;
        let parameters = f
            .parameters
            .iter()
            .map(|p| match p {
                Parameter::Value(ty) => Ok(*ty),
                Parameter::Function(_) => Err(error(
                    "E007",
                    span,
                    "callback must have a first-order signature",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(syntax::FunctionSignature {
            parameters,
            result: f.result_type,
        })
    }
    fn request(
        &mut self,
        label: u32,
        args: &[Binding],
        span: Span,
    ) -> Result<(u32, Vec<u32>), Diagnostic> {
        let f = self
            .templates
            .get(&label)
            .ok_or_else(|| error("E004", span, "unknown function"))?;
        if args.len() != f.parameters.len() {
            return Err(error("E006", span, "function arity mismatch")
                .mismatch(f.parameters.len(), args.len()));
        }
        let mut callbacks = vec![];
        let mut runtime = vec![];
        for (p, arg) in f.parameters.iter().zip(args) {
            match (p, arg) {
                (Parameter::Value(_), Binding::Value(id)) => runtime.push(*id),
                (Parameter::Function(expected), Binding::Function(label)) => {
                    let actual = self.signature(*label, span)?;
                    if &actual != expected {
                        return Err(error("E007", span, "callback signature mismatch")
                            .mismatch(format!("{expected:?}"), format!("{actual:?}")));
                    }
                    callbacks.push(*label);
                }
                _ => return Err(error("E007", span, "function/value argument mismatch")),
            }
        }
        let target = self.reserve(label, callbacks, span)?;
        Ok((target, runtime))
    }
    fn reserve(&mut self, label: u32, callbacks: Vec<u32>, span: Span) -> Result<u32, Diagnostic> {
        let key = (label, callbacks.clone());
        if let Some(target) = self.instances.get(&key) {
            return Ok(*target);
        }
        let f = &self.templates[&label];
        let specialized = f
            .parameters
            .iter()
            .any(|p| matches!(p, Parameter::Function(_)));
        let size = count(&f.instructions);
        if self.functions.len() >= MAX_FUNCTIONS
            || self.instructions + size > MAX_INSTRUCTIONS
            || (specialized
                && (self.specialized >= MAX_SPECIALIZATIONS
                    || self.per_template.get(&label).copied().unwrap_or(0) >= MAX_PER_TEMPLATE))
        {
            return Err(error("E008", span, "static specialization limit exceeded"));
        }
        let target = if specialized {
            let id = self.next_label;
            self.next_label = id
                .checked_add(1)
                .ok_or_else(|| error("E008", span, "specialization label overflow"))?;
            self.specialized += 1;
            *self.per_template.entry(label).or_default() += 1;
            id
        } else {
            label
        };
        self.instructions += size;
        self.instances.insert(key.clone(), target);
        let mut f = f.clone();
        f.label = target;
        self.functions.push(f);
        self.queue.push(key);
        Ok(target)
    }
    fn reference(
        &self,
        r: &Ref,
        callbacks: &BTreeMap<u32, u32>,
        span: Span,
    ) -> Result<u32, Diagnostic> {
        match r {
            Ref::Local(label) => {
                if !self.templates.contains_key(label) {
                    return Err(error("E004", span, "unknown function reference"));
                }
                Ok(*label)
            }
            Ref::Parameter(id) => callbacks
                .get(id)
                .copied()
                .ok_or_else(|| error("E005", span, "unknown static function parameter")),
            Ref::Export(..) => Err(nil_hir::plugin::error(
                "module function reference not linked",
            )),
        }
    }
    fn region(
        &mut self,
        region: syntax::Region,
        bindings: Vec<Binding>,
        callbacks: &BTreeMap<u32, u32>,
    ) -> Result<syntax::Region, Diagnostic> {
        let span = region
            .instructions
            .last()
            .map_or(Span { start: 0, end: 0 }, |i| i.span);
        let (instructions, bindings) = self.items(region.instructions, bindings, callbacks)?;
        let results = region
            .results
            .into_iter()
            .map(|id| value(&bindings, id, span))
            .collect::<Result<_, _>>()?;
        Ok(syntax::Region {
            instructions,
            results,
        })
    }
    fn items(
        &mut self,
        items: Vec<syntax::Instruction>,
        mut bindings: Vec<Binding>,
        callbacks: &BTreeMap<u32, u32>,
    ) -> Result<(Vec<syntax::Instruction>, Vec<Binding>), Diagnostic> {
        let mut next = bindings
            .iter()
            .filter(|b| matches!(b, Binding::Value(_)))
            .count() as u32;
        let mut output = vec![];
        for i in items {
            let span = i.span;
            let v = |id| value(&bindings, id, span);
            let list = |ids: Vec<u32>| ids.into_iter().map(v).collect::<Result<Vec<_>, _>>();
            let arguments = |ids: Vec<u32>| {
                ids.into_iter()
                    .map(|id| {
                        bindings
                            .get(id as usize)
                            .copied()
                            .ok_or_else(|| error("E005", span, "undefined call argument"))
                    })
                    .collect::<Result<Vec<_>, _>>()
            };
            let kind = match i.kind {
                K::FunctionReference(r) => {
                    bindings.push(Binding::Function(self.reference(&r, callbacks, span)?));
                    continue;
                }
                K::CallbackCall(r, args) => {
                    let label = self.reference(&r, callbacks, span)?;
                    let (label, args) = self.request(label, &arguments(args)?, span)?;
                    K::Call(label, args)
                }
                K::Call(label, args) => {
                    let (label, args) = self.request(label, &arguments(args)?, span)?;
                    K::Call(label, args)
                }
                K::Std(..) => return Err(error("E007", span, "unlinked std call")),
                K::Plugin(id, op, args) => K::Plugin(id, op, list(args)?),
                K::LinkedPlugin(provider, args) => K::LinkedPlugin(provider, list(args)?),
                K::Record(ty, args) => K::Record(ty, list(args)?),
                K::Field(id, name) => K::Field(v(id)?, name),
                K::UpdateField(id, name, replacement) => {
                    K::UpdateField(v(id)?, name, v(replacement)?)
                }
                K::RecordBuffer(ty, n, fill) => K::RecordBuffer(ty, v(n)?, v(fill)?),
                K::Intrinsic(op, args) => K::Intrinsic(op, list(args)?),
                K::Array(args) => K::Array(list(args)?),
                K::Repeat(id, n) => K::Repeat(v(id)?, n),
                K::Length(id) => K::Length(v(id)?),
                K::Index(a, b) => K::Index(v(a)?, v(b)?),
                K::Replace(a, b, c) => K::Replace(v(a)?, v(b)?, v(c)?),
                K::Binary(op, a, b) => K::Binary(op, v(a)?, v(b)?),
                K::Compare(op, a, b) => K::Compare(op, v(a)?, v(b)?),
                K::If(id, a, b) => K::If(
                    v(id)?,
                    self.region(a, bindings.clone(), callbacks)?,
                    self.region(b, bindings.clone(), callbacks)?,
                ),
                K::Loop(initial, a, b, c) => {
                    let initial = list(initial)?;
                    let inputs = || (0..initial.len() as u32).map(Binding::Value).collect();
                    K::Loop(
                        initial.clone(),
                        self.region(a, inputs(), callbacks)?,
                        self.region(b, inputs(), callbacks)?,
                        self.region(c, inputs(), callbacks)?,
                    )
                }
                K::Each(input, initial, body, finish) => {
                    let input = v(input)?;
                    let initial = list(initial)?;
                    let body_inputs = (0..initial.len() as u32 + 2).map(Binding::Value).collect();
                    let finish_inputs = (0..initial.len() as u32).map(Binding::Value).collect();
                    K::Each(
                        input,
                        initial,
                        self.region(body, body_inputs, callbacks)?,
                        self.region(finish, finish_inputs, callbacks)?,
                    )
                }
                k @ (K::RecordMap(_)
                | K::Bytes(_)
                | K::Constant(_)
                | K::Unsigned(..)
                | K::Float(_)
                | K::Boolean(_)) => k,
            };
            output.push(syntax::Instruction { kind, span });
            bindings.push(Binding::Value(next));
            next += 1;
        }
        Ok((output, bindings))
    }
}
/// Reject residual higher-order constructs independently of the normalizer.
/// Typed HIR cannot represent function values; this is its frontend boundary.
pub(crate) fn first_order(module: &syntax::Module) -> Result<(), Diagnostic> {
    fn check(items: &[syntax::Instruction]) -> Result<(), Diagnostic> {
        for i in items {
            match &i.kind {
                K::Std(..) | K::FunctionReference(_) | K::CallbackCall(..) => {
                    return Err(error(
                        "E007",
                        i.span,
                        "unspecialized function value at HIR boundary",
                    ));
                }
                K::If(_, a, b) => {
                    check(&a.instructions)?;
                    check(&b.instructions)?;
                }
                K::Loop(_, a, b, c) => {
                    for r in [a, b, c] {
                        check(&r.instructions)?;
                    }
                }
                K::Each(_, _, a, b) => {
                    check(&a.instructions)?;
                    check(&b.instructions)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    for f in &module.functions {
        if f.parameters
            .iter()
            .any(|p| matches!(p, Parameter::Function(_)))
        {
            return Err(error(
                "E007",
                f.span,
                "unspecialized function parameter at HIR boundary",
            ));
        }
        check(&f.instructions)?;
    }
    Ok(())
}
pub(crate) fn specialize(module: syntax::Module) -> Result<syntax::Module, Diagnostic> {
    // Preserve every old program's function order/labels/IR exactly.
    if first_order(&module).is_ok() {
        return Ok(module);
    }
    let mut templates = BTreeMap::new();
    let mut ordinary = vec![];
    for f in module.functions {
        if !f
            .parameters
            .iter()
            .any(|p| matches!(p, Parameter::Function(_)))
        {
            ordinary.push(f.label);
        }
        let span = f.span;
        if templates.insert(f.label, f).is_some() {
            return Err(error("E003", span, "duplicate function"));
        }
    }
    let next_label = templates
        .keys()
        .last()
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            error(
                "E008",
                Span { start: 0, end: 0 },
                "specialization label overflow",
            )
        })?;
    let mut s = Specializer {
        templates,
        instances: BTreeMap::new(),
        per_template: BTreeMap::new(),
        functions: vec![],
        queue: vec![],
        next_label,
        specialized: 0,
        instructions: 0,
    };
    // Explicit references and static parameter positions must be valid even in unused templates.
    fn references(
        items: &[syntax::Instruction],
        f: &syntax::Function,
        templates: &BTreeMap<u32, syntax::Function>,
    ) -> Result<(), Diagnostic> {
        for i in items {
            match &i.kind {
                K::FunctionReference(Ref::Local(label)) | K::Call(label, _)
                    if !templates.contains_key(label) =>
                {
                    return Err(error("E004", i.span, "unknown function reference"));
                }
                K::FunctionReference(Ref::Parameter(id))
                | K::CallbackCall(Ref::Parameter(id), _)
                    if !matches!(f.parameters.get(*id as usize), Some(Parameter::Function(_))) =>
                {
                    return Err(error("E005", i.span, "unknown static function parameter"));
                }
                K::If(_, a, b) => {
                    references(&a.instructions, f, templates)?;
                    references(&b.instructions, f, templates)?;
                }
                K::Loop(_, a, b, c) => {
                    for r in [a, b, c] {
                        references(&r.instructions, f, templates)?;
                    }
                }
                K::Each(_, _, a, b) => {
                    references(&a.instructions, f, templates)?;
                    references(&b.instructions, f, templates)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
    for f in s.templates.values() {
        references(&f.instructions, f, &s.templates)?;
    }
    for label in ordinary {
        s.reserve(label, vec![], s.templates[&label].span)?;
    }
    let mut cursor = 0;
    while cursor < s.functions.len() {
        let (template, arguments) = s.queue[cursor].clone();
        let source = s.templates[&template].clone();
        let mut bindings = vec![];
        let mut callbacks = BTreeMap::new();
        let mut args = arguments.into_iter();
        let mut parameters = vec![];
        for (id, p) in source.parameters.iter().enumerate() {
            match p {
                Parameter::Value(ty) => {
                    bindings.push(Binding::Value(parameters.len() as u32));
                    parameters.push(Parameter::Value(*ty));
                }
                Parameter::Function(_) => {
                    let label = args.next().expect("reserved specialization callback tuple");
                    callbacks.insert(id as u32, label);
                    bindings.push(Binding::Function(label));
                }
            }
        }
        let (instructions, bindings) = s.items(source.instructions, bindings, &callbacks)?;
        s.functions[cursor].parameters = parameters;
        s.functions[cursor].instructions = instructions;
        s.functions[cursor].result = value(&bindings, source.result, source.return_span)?;
        cursor += 1;
    }
    let module = syntax::Module {
        records: module.records,
        functions: s.functions,
    };
    first_order(&module)?;
    Ok(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hir_boundary_rejects_unspecialized_function_arguments_and_references() {
        for source in ["=&a", "([i:i])=^a(1)", "=false?&a:1"] {
            let ast = crate::expr::parse_application(source).unwrap();
            assert_eq!(first_order(&ast).unwrap_err().code, "E007");
        }
        let ast = crate::expr::parse_application("=b(&c,2)\n([i:i],i)=^a(b)\n1=a+1").unwrap();
        let ast = specialize(ast).unwrap();
        first_order(&ast).unwrap();
    }
}
