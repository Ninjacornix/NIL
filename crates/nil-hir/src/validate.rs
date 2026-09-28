use crate::*;

fn value_type(types: &[Type], id: ValueId, span: Option<Span>) -> Result<Type, Diagnostic> {
    types.get(id.0).copied().ok_or_else(|| {
        Diagnostic::new(
            "E005",
            Phase::Check,
            span,
            format!("value {} is not defined before use", id.0),
        )
    })
}

fn require_type(expected: Type, actual: Type, span: Option<Span>) -> Result<(), Diagnostic> {
    if expected != actual {
        return Err(Diagnostic::new("E007", Phase::Check, span, "type mismatch")
            .mismatch(format!("{expected:?}"), format!("{actual:?}")));
    }
    Ok(())
}

pub fn validate(program: Program) -> Result<ValidatedProgram, Diagnostic> {
    if program.functions.is_empty() {
        return Err(Diagnostic::new(
            "E007",
            Phase::Check,
            None,
            "program has no functions",
        ));
    }
    for function in &program.functions {
        let mut types = function.parameters.clone();
        for instruction in &function.instructions {
            let span = instruction.span;
            let result = match &instruction.operation {
                Operation::Constant(_) => Type::I64,
                Operation::Binary { lhs, rhs, .. } => {
                    require_type(Type::I64, value_type(&types, *lhs, span)?, span)?;
                    require_type(Type::I64, value_type(&types, *rhs, span)?, span)?;
                    Type::I64
                }
                Operation::Call {
                    function,
                    arguments,
                } => {
                    let callee = program.functions.get(function.0).ok_or_else(|| {
                        Diagnostic::new("E004", Phase::Check, span, "unknown HIR function")
                    })?;
                    if arguments.len() != callee.parameters.len() {
                        return Err(Diagnostic::new(
                            "E006",
                            Phase::Check,
                            span,
                            "call arity mismatch",
                        )
                        .mismatch(callee.parameters.len(), arguments.len()));
                    }
                    for (argument, expected) in arguments.iter().zip(&callee.parameters) {
                        require_type(*expected, value_type(&types, *argument, span)?, span)?;
                    }
                    callee.result_type
                }
            };
            require_type(instruction.ty, result, span)?;
            types.push(result);
        }
        require_type(
            function.result_type,
            value_type(&types, function.result, function.return_span)?,
            function.return_span,
        )?;
    }
    Ok(ValidatedProgram(program))
}
