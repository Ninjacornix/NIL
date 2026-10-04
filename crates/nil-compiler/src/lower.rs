use crate::{SourceProfile, parser, syntax};
use nil_hir::*;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct CompiledProgram {
    pub hir: ValidatedProgram,
    labels: BTreeMap<u32, FunctionId>,
}
impl CompiledProgram {
    pub fn function(&self, label: u32) -> Option<FunctionId> {
        self.labels.get(&label).copied()
    }
}

pub fn compile(source: &str) -> Result<CompiledProgram, Diagnostic> {
    compile_with_profile(source, SourceProfile::default())
}

pub fn compile_with_profile(
    source: &str,
    profile: SourceProfile,
) -> Result<CompiledProgram, Diagnostic> {
    let arithmetic = if matches!(
        profile,
        SourceProfile::ExprV3 | SourceProfile::ExprV4 | SourceProfile::ExprV5
    ) {
        Arithmetic::Wrapping
    } else {
        Arithmetic::Checked
    };
    lower_with_arithmetic(parser::parse_with_profile(source, profile)?, arithmetic)
}

pub fn lower(module: syntax::Module) -> Result<CompiledProgram, Diagnostic> {
    lower_with_arithmetic(module, Arithmetic::Checked)
}

pub fn lower_with_arithmetic(
    module: syntax::Module,
    arithmetic: Arithmetic,
) -> Result<CompiledProgram, Diagnostic> {
    nil_hir::records::validate_definitions(&module.records)?;
    let mut labels = BTreeMap::new();
    for (index, function) in module.functions.iter().enumerate() {
        if labels.insert(function.label, FunctionId(index)).is_some() {
            return Err(Diagnostic::new(
                "E003",
                Phase::Check,
                Some(function.span),
                format!("duplicate function {}", function.label),
            ));
        }
    }
    let signatures: Vec<Function> = module
        .functions
        .iter()
        .map(|f| Function {
            parameters: f.parameters.clone(),
            result_type: f.result_type,
            instructions: vec![],
            result: ValueId(0),
            return_span: Some(f.return_span),
        })
        .collect();
    let mut functions = Vec::new();
    for function in module.functions {
        let scope = lower_scope(
            function.instructions,
            Scope::inputs(&function.parameters),
            &signatures,
            &module.records,
            &labels,
            0,
        )?;
        let result = scope.binding(function.result, Some(function.return_span))?;
        functions.push(Function {
            parameters: function.parameters,
            result_type: function.result_type,
            instructions: scope.instructions,
            result,
            return_span: Some(function.return_span),
        });
    }
    Ok(CompiledProgram {
        hir: validate(Program {
            records: module.records,
            arithmetic,
            functions,
        })?,
        labels,
    })
}

/// Source bindings need not coincide with HIR ids: each binds an index/key and
/// element/value but lowers into a loop carrying a snapshot and hidden index.
struct Scope {
    types: Vec<Type>,
    instructions: Vec<Instruction>,
    bindings: Vec<ValueId>,
}
impl Scope {
    fn inputs(types: &[Type]) -> Self {
        Self {
            types: types.to_vec(),
            instructions: vec![],
            bindings: (0..types.len()).map(ValueId).collect(),
        }
    }
    fn binding(&self, id: u32, span: Option<Span>) -> Result<ValueId, Diagnostic> {
        self.bindings.get(id as usize).copied().ok_or_else(|| {
            Diagnostic::new(
                "E005",
                Phase::Check,
                span,
                format!("value {id} is not defined before use"),
            )
        })
    }
    fn emit(
        &mut self,
        operation: Operation,
        signatures: &[Function],
        records: &[RecordDefinition],
        span: Option<Span>,
        depth: usize,
    ) -> Result<ValueId, Diagnostic> {
        let ty =
            operation_type_with_records(signatures, records, &operation, &self.types, span, depth)?;
        let id = ValueId(self.types.len());
        self.types.push(ty);
        self.instructions.push(Instruction {
            operation,
            ty,
            span,
        });
        Ok(id)
    }
}
fn lower_region(
    region: syntax::Region,
    inputs: &[Type],
    signatures: &[Function],
    records: &[RecordDefinition],
    labels: &BTreeMap<u32, FunctionId>,
    depth: usize,
) -> Result<Region, Diagnostic> {
    lower_region_mapped(
        region,
        inputs,
        (0..inputs.len()).map(ValueId).collect(),
        signatures,
        records,
        labels,
        depth,
    )
}
fn lower_region_mapped(
    region: syntax::Region,
    inputs: &[Type],
    bindings: Vec<ValueId>,
    signatures: &[Function],
    records: &[RecordDefinition],
    labels: &BTreeMap<u32, FunctionId>,
    depth: usize,
) -> Result<Region, Diagnostic> {
    let scope = lower_scope(
        region.instructions,
        Scope {
            types: inputs.to_vec(),
            instructions: vec![],
            bindings,
        },
        signatures,
        records,
        labels,
        depth,
    )?;
    let results = region
        .results
        .into_iter()
        .map(|id| scope.binding(id, None))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Region {
        instructions: scope.instructions,
        results,
    })
}
fn lower_scope(
    source: Vec<syntax::Instruction>,
    mut scope: Scope,
    signatures: &[Function],
    records: &[RecordDefinition],
    labels: &BTreeMap<u32, FunctionId>,
    depth: usize,
) -> Result<Scope, Diagnostic> {
    if depth > MAX_REGION_DEPTH {
        return Err(Diagnostic::new(
            "E008",
            Phase::Check,
            None,
            "AST region nesting limit exceeded",
        ));
    }
    for instruction in source {
        let span = Some(instruction.span);
        let operation = match instruction.kind {
            syntax::InstructionKind::Record(ty, fields) => Operation::Record {
                ty,
                fields: fields
                    .into_iter()
                    .map(|id| scope.binding(id, span))
                    .collect::<Result<_, _>>()?,
            },
            syntax::InstructionKind::Field(record, name) => {
                let record = scope.binding(record, span)?;
                let definition =
                    nil_hir::records::definition(records, scope.types[record.0], span)?;
                let field = definition
                    .fields
                    .iter()
                    .position(|f| f.name == name)
                    .ok_or_else(|| nil_hir::records::error(span, "unknown record field"))?;
                Operation::Field { record, field }
            }
            syntax::InstructionKind::UpdateField(record, name, value) => {
                let record = scope.binding(record, span)?;
                let definition =
                    nil_hir::records::definition(records, scope.types[record.0], span)?;
                let field = definition
                    .fields
                    .iter()
                    .position(|f| f.name == name)
                    .ok_or_else(|| nil_hir::records::error(span, "unknown record field"))?;
                Operation::UpdateField {
                    record,
                    field,
                    value: scope.binding(value, span)?,
                }
            }
            syntax::InstructionKind::RecordMap(ty) => Operation::RecordMap(ty),
            syntax::InstructionKind::Bytes(bytes) => Operation::Bytes(bytes),
            syntax::InstructionKind::Intrinsic(op, ids) => Operation::Intrinsic {
                op,
                arguments: ids
                    .into_iter()
                    .map(|id| scope.binding(id, span))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            syntax::InstructionKind::Constant(v) => Operation::Constant(v),
            syntax::InstructionKind::Unsigned(value, ty) => Operation::Unsigned {
                value: (value[0] as u128) | ((value[1] as u128) << 64),
                ty,
            },
            syntax::InstructionKind::Float(v) => Operation::Float(v),
            syntax::InstructionKind::Boolean(v) => Operation::Boolean(v),
            syntax::InstructionKind::Array(ids) => Operation::Array(
                ids.into_iter()
                    .map(|id| scope.binding(id, span))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            syntax::InstructionKind::Repeat(value, len) => Operation::Repeat {
                value: scope.binding(value, span)?,
                len,
            },
            syntax::InstructionKind::Length(array) => {
                Operation::Length(scope.binding(array, span)?)
            }
            syntax::InstructionKind::Index(array, index) => Operation::Index {
                array: scope.binding(array, span)?,
                index: scope.binding(index, span)?,
            },
            syntax::InstructionKind::Replace(array, index, value) => Operation::Replace {
                array: scope.binding(array, span)?,
                index: scope.binding(index, span)?,
                value: scope.binding(value, span)?,
            },
            syntax::InstructionKind::Binary(op, a, b) => Operation::Binary {
                op,
                lhs: scope.binding(a, span)?,
                rhs: scope.binding(b, span)?,
            },
            syntax::InstructionKind::Compare(op, a, b) => Operation::Compare {
                op,
                lhs: scope.binding(a, span)?,
                rhs: scope.binding(b, span)?,
            },
            syntax::InstructionKind::Call(label, args) => Operation::Call {
                function: *labels.get(&label).ok_or_else(|| {
                    Diagnostic::new(
                        "E004",
                        Phase::Check,
                        span,
                        format!("unknown function {label}"),
                    )
                })?,
                arguments: args
                    .into_iter()
                    .map(|id| scope.binding(id, span))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            syntax::InstructionKind::If(c, yes, no) => Operation::If {
                condition: scope.binding(c, span)?,
                then_region: lower_region_mapped(
                    yes,
                    &scope.types,
                    scope.bindings.clone(),
                    signatures,
                    records,
                    labels,
                    depth + 1,
                )?,
                else_region: lower_region_mapped(
                    no,
                    &scope.types,
                    scope.bindings.clone(),
                    signatures,
                    records,
                    labels,
                    depth + 1,
                )?,
            },
            syntax::InstructionKind::Loop(initial, cond, body, finish) => {
                let initial: Vec<ValueId> = initial
                    .into_iter()
                    .map(|id| scope.binding(id, span))
                    .collect::<Result<Vec<_>, _>>()?;
                let state = initial
                    .iter()
                    .map(|id| value_type(&scope.types, *id, span))
                    .collect::<Result<Vec<_>, _>>()?;
                Operation::Loop {
                    initial,
                    condition: lower_region(cond, &state, signatures, records, labels, depth + 1)?,
                    body: lower_region(body, &state, signatures, records, labels, depth + 1)?,
                    finish: lower_region(finish, &state, signatures, records, labels, depth + 1)?,
                }
            }
            syntax::InstructionKind::Each(input, initial, body, finish) => {
                let input = scope.binding(input, span)?;
                let initial = initial
                    .into_iter()
                    .map(|id| scope.binding(id, span))
                    .collect::<Result<Vec<_>, _>>()?;
                let zero = scope.emit(Operation::Constant(0), signatures, records, span, depth)?;
                lower_each(
                    input,
                    zero,
                    initial,
                    body,
                    finish,
                    &scope.types,
                    signatures,
                    records,
                    labels,
                    depth,
                    span,
                )?
            }
        };
        let id = scope.emit(operation, signatures, records, span, depth)?;
        scope.bindings.push(id);
    }
    Ok(scope)
}
#[allow(clippy::too_many_arguments)]
fn lower_each(
    input: ValueId,
    zero: ValueId,
    initial: Vec<ValueId>,
    body: syntax::Region,
    finish: syntax::Region,
    outer: &[Type],
    signatures: &[Function],
    records: &[RecordDefinition],
    labels: &BTreeMap<u32, FunctionId>,
    depth: usize,
    span: Option<Span>,
) -> Result<Operation, Diagnostic> {
    let input_ty = value_type(outer, input, span)?;
    if !matches!(
        input_ty,
        Type::Buffer
            | Type::Bytes
            | Type::Array(_)
            | Type::MapI64
            | Type::MapBytes
            | Type::MapRecord(..)
    ) {
        return Err(Diagnostic::new(
            "E007",
            Phase::Check,
            span,
            "each requires a collection",
        ));
    }
    let mut state = vec![input_ty, Type::I64];
    for id in &initial {
        state.push(value_type(outer, *id, span)?);
    }
    let mut condition = Scope::inputs(&state);
    let length = condition.emit(
        if input_ty.map_value().is_some() {
            Operation::Intrinsic {
                op: Intrinsic::Size,
                arguments: vec![ValueId(0)],
            }
        } else {
            Operation::Length(ValueId(0))
        },
        signatures,
        records,
        span,
        depth + 1,
    )?;
    let test = condition.emit(
        Operation::Compare {
            op: CompareOp::Lt,
            lhs: ValueId(1),
            rhs: length,
        },
        signatures,
        records,
        span,
        depth + 1,
    )?;
    let condition = Region {
        instructions: condition.instructions,
        results: vec![test],
    };
    let mut step = Scope::inputs(&state);
    let (key, value) = if input_ty.map_value().is_some() {
        let key = step.emit(
            Operation::Intrinsic {
                op: Intrinsic::Key,
                arguments: vec![ValueId(0), ValueId(1)],
            },
            signatures,
            records,
            span,
            depth + 1,
        )?;
        let value = step.emit(
            Operation::Intrinsic {
                op: Intrinsic::Get,
                arguments: vec![ValueId(0), key],
            },
            signatures,
            records,
            span,
            depth + 1,
        )?;
        (key, value)
    } else {
        let value = step.emit(
            Operation::Index {
                array: ValueId(0),
                index: ValueId(1),
            },
            signatures,
            records,
            span,
            depth + 1,
        )?;
        (ValueId(1), value)
    };
    step.bindings = vec![key, value];
    step.bindings.extend((2..state.len()).map(ValueId));
    let mut step = lower_scope(
        body.instructions,
        step,
        signatures,
        records,
        labels,
        depth + 1,
    )?;
    let next = body
        .results
        .into_iter()
        .map(|id| step.binding(id, span))
        .collect::<Result<Vec<_>, _>>()?;
    if next.len() != initial.len() {
        return Err(Diagnostic::new(
            "E006",
            Phase::Check,
            span,
            "each state result count mismatch",
        ));
    }
    let one = step.emit(Operation::Constant(1), signatures, records, span, depth + 1)?;
    let advance = step.emit(
        Operation::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(1),
            rhs: one,
        },
        signatures,
        records,
        span,
        depth + 1,
    )?;
    let mut results = vec![ValueId(0), advance];
    results.extend(next);
    let body = Region {
        instructions: step.instructions,
        results,
    };
    let finish = lower_region_mapped(
        finish,
        &state,
        (2..state.len()).map(ValueId).collect(),
        signatures,
        records,
        labels,
        depth + 1,
    )?;
    let mut all_initial = vec![input, zero];
    all_initial.extend(initial);
    Ok(Operation::Loop {
        initial: all_initial,
        condition,
        body,
        finish,
    })
}

// Source offsets are diagnostic metadata, including inside nested regions.
fn semantic_operation(operation: &Operation) -> Operation {
    let mut operation = operation.clone();
    let regions: Vec<&mut Region> = match &mut operation {
        Operation::If {
            then_region,
            else_region,
            ..
        } => vec![then_region, else_region],
        Operation::Loop {
            condition,
            body,
            finish,
            ..
        } => vec![condition, body, finish],
        _ => vec![],
    };
    for region in regions {
        for instruction in &mut region.instructions {
            instruction.span = None;
            instruction.operation = semantic_operation(&instruction.operation);
        }
    }
    operation
}

/// Deterministic debugging projection, not a stable serialization or source profile.
pub fn dump(program: &ValidatedProgram) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if program.program().arithmetic == Arithmetic::Wrapping {
        out.push_str("arithmetic Wrapping\n");
    }
    for record in &program.program().records {
        writeln!(out, "record {} {:?}", record.name, record.fields).unwrap();
    }
    for (index, function) in program.program().functions.iter().enumerate() {
        writeln!(
            out,
            "function {index} {:?} -> {:?}",
            function.parameters, function.result_type
        )
        .unwrap();
        for (n, instruction) in function.instructions.iter().enumerate() {
            writeln!(
                out,
                "  v{}: {:?} = {:?}",
                n + function.parameters.len(),
                instruction.ty,
                semantic_operation(&instruction.operation)
            )
            .unwrap();
        }
        writeln!(out, "  return v{}", function.result.0).unwrap();
    }
    out
}
