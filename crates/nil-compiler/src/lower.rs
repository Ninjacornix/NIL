use crate::{SourceProfile, expr, parser, syntax};
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
    compile_with_profile(source, SourceProfile::LinesV0)
}

pub fn compile_with_profile(
    source: &str,
    profile: SourceProfile,
) -> Result<CompiledProgram, Diagnostic> {
    lower(match profile {
        SourceProfile::LinesV0 => parser::parse(source)?,
        SourceProfile::ExprV0 => expr::parse(source)?,
    })
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
    let mut functions = Vec::new();
    for function in module.functions {
        let mut instructions = Vec::new();
        for instruction in function.instructions {
            let span = Some(instruction.span);
            let operation = match instruction.kind {
                syntax::InstructionKind::Constant(value) => Operation::Constant(value),
                syntax::InstructionKind::Binary(op, lhs, rhs) => Operation::Binary {
                    op,
                    lhs: ValueId(lhs as usize),
                    rhs: ValueId(rhs as usize),
                },
                syntax::InstructionKind::Call(label, arguments) => Operation::Call {
                    function: *labels.get(&label).ok_or_else(|| {
                        Diagnostic::new(
                            "E004",
                            Phase::Check,
                            span,
                            format!("unknown function {label}"),
                        )
                    })?,
                    arguments: arguments
                        .into_iter()
                        .map(|id| ValueId(id as usize))
                        .collect(),
                },
            };
            // Every M1 operation produces i64. The independent validator checks
            // signatures, operand availability and result annotations below.
            instructions.push(Instruction {
                operation,
                ty: Type::I64,
                span,
            });
        }
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
                instruction.operation
            )
            .unwrap();
        }
        writeln!(out, "  return v{}", function.result.0).unwrap();
    }
    out
}
