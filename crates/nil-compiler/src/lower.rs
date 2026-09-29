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
    lower(parser::parse_with_profile(source, profile)?)
}

pub fn lower(module: syntax::Module) -> Result<CompiledProgram, Diagnostic> {
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
        let (instructions, _) = lower_instructions(
            function.instructions,
            &function.parameters,
            &signatures,
            &labels,
            0,
        )?;
        functions.push(Function {
            parameters: function.parameters,
            result_type: function.result_type,
            instructions,
            result: ValueId(function.result as usize),
            return_span: Some(function.return_span),
        });
    }
    Ok(CompiledProgram {
        hir: validate(Program { functions })?,
        labels,
    })
}

fn lower_region(
    region: syntax::Region,
    inputs: &[Type],
    signatures: &[Function],
    labels: &BTreeMap<u32, FunctionId>,
    depth: usize,
) -> Result<Region, Diagnostic> {
    let (instructions, _) =
        lower_instructions(region.instructions, inputs, signatures, labels, depth)?;
    Ok(Region {
        instructions,
        results: region
            .results
            .into_iter()
            .map(|id| ValueId(id as usize))
            .collect(),
    })
}
fn lower_instructions(
    source: Vec<syntax::Instruction>,
    inputs: &[Type],
    signatures: &[Function],
    labels: &BTreeMap<u32, FunctionId>,
    depth: usize,
) -> Result<(Vec<Instruction>, Vec<Type>), Diagnostic> {
    if depth > MAX_REGION_DEPTH {
        return Err(Diagnostic::new(
            "E008",
            Phase::Check,
            None,
            "AST region nesting limit exceeded",
        ));
    }
    let mut types = inputs.to_vec();
    let mut instructions = Vec::new();
    for instruction in source {
        let span = Some(instruction.span);
        let operation = match instruction.kind {
            syntax::InstructionKind::Constant(v) => Operation::Constant(v),
            syntax::InstructionKind::Boolean(v) => Operation::Boolean(v),
            syntax::InstructionKind::Binary(op, a, b) => Operation::Binary {
                op,
                lhs: ValueId(a as usize),
                rhs: ValueId(b as usize),
            },
            syntax::InstructionKind::Compare(op, a, b) => Operation::Compare {
                op,
                lhs: ValueId(a as usize),
                rhs: ValueId(b as usize),
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
                arguments: args.into_iter().map(|id| ValueId(id as usize)).collect(),
            },
            syntax::InstructionKind::If(c, yes, no) => Operation::If {
                condition: ValueId(c as usize),
                then_region: lower_region(yes, &types, signatures, labels, depth + 1)?,
                else_region: lower_region(no, &types, signatures, labels, depth + 1)?,
            },
            syntax::InstructionKind::Loop(initial, cond, body, finish) => {
                let initial: Vec<ValueId> =
                    initial.into_iter().map(|id| ValueId(id as usize)).collect();
                let state = initial
                    .iter()
                    .map(|id| value_type(&types, *id, span))
                    .collect::<Result<Vec<_>, _>>()?;
                Operation::Loop {
                    initial,
                    condition: lower_region(cond, &state, signatures, labels, depth + 1)?,
                    body: lower_region(body, &state, signatures, labels, depth + 1)?,
                    finish: lower_region(finish, &state, signatures, labels, depth + 1)?,
                }
            }
        };
        let ty = operation_type(signatures, &operation, &types, span, depth)?;
        types.push(ty);
        instructions.push(Instruction {
            operation,
            ty,
            span,
        });
    }
    Ok((instructions, types))
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
