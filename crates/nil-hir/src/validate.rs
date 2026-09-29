use crate::*;

pub fn value_type(types: &[Type], id: ValueId, span: Option<Span>) -> Result<Type, Diagnostic> {
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
fn arity(expected: usize, actual: usize, span: Option<Span>) -> Result<(), Diagnostic> {
    if expected != actual {
        return Err(
            Diagnostic::new("E006", Phase::Check, span, "region/call arity mismatch")
                .mismatch(expected, actual),
        );
    }
    Ok(())
}
fn instructions_types(
    functions: &[Function],
    instructions: &[Instruction],
    inputs: &[Type],
    depth: usize,
) -> Result<Vec<Type>, Diagnostic> {
    if depth > MAX_REGION_DEPTH {
        return Err(Diagnostic::new(
            "E008",
            Phase::Check,
            None,
            "HIR region nesting limit exceeded",
        ));
    }
    let mut types = inputs.to_vec();
    for instruction in instructions {
        let ty = operation_type(
            functions,
            &instruction.operation,
            &types,
            instruction.span,
            depth,
        )?;
        require_type(instruction.ty, ty, instruction.span)?;
        types.push(ty);
    }
    Ok(types)
}
fn region_types(
    functions: &[Function],
    region: &Region,
    inputs: &[Type],
    depth: usize,
    span: Option<Span>,
) -> Result<Vec<Type>, Diagnostic> {
    let types = instructions_types(functions, &region.instructions, inputs, depth)?;
    region
        .results
        .iter()
        .map(|id| value_type(&types, *id, span))
        .collect()
}

/// Infer and independently validate one semantic operation against its available
/// operands and complete function signatures. Used by typed lowering as well.
pub fn operation_type(
    functions: &[Function],
    operation: &Operation,
    types: &[Type],
    span: Option<Span>,
    depth: usize,
) -> Result<Type, Diagnostic> {
    match operation {
        Operation::Constant(_) => Ok(Type::I64),
        Operation::Boolean(_) => Ok(Type::Bool),
        Operation::Binary { lhs, rhs, .. } | Operation::Compare { lhs, rhs, .. } => {
            require_type(Type::I64, value_type(types, *lhs, span)?, span)?;
            require_type(Type::I64, value_type(types, *rhs, span)?, span)?;
            Ok(if matches!(operation, Operation::Compare { .. }) {
                Type::Bool
            } else {
                Type::I64
            })
        }
        Operation::Call {
            function,
            arguments,
        } => {
            let callee = functions.get(function.0).ok_or_else(|| {
                Diagnostic::new("E004", Phase::Check, span, "unknown HIR function")
            })?;
            arity(callee.parameters.len(), arguments.len(), span)?;
            for (argument, expected) in arguments.iter().zip(&callee.parameters) {
                require_type(*expected, value_type(types, *argument, span)?, span)?;
            }
            Ok(callee.result_type)
        }
        Operation::If {
            condition,
            then_region,
            else_region,
        } => {
            require_type(Type::Bool, value_type(types, *condition, span)?, span)?;
            let yes = region_types(functions, then_region, types, depth + 1, span)?;
            let no = region_types(functions, else_region, types, depth + 1, span)?;
            arity(1, yes.len(), span)?;
            arity(1, no.len(), span)?;
            require_type(yes[0], no[0], span)?;
            Ok(yes[0])
        }
        Operation::Loop {
            initial,
            condition,
            body,
            finish,
        } => {
            let state: Vec<Type> = initial
                .iter()
                .map(|id| value_type(types, *id, span))
                .collect::<Result<_, _>>()?;
            let cond = region_types(functions, condition, &state, depth + 1, span)?;
            arity(1, cond.len(), span)?;
            require_type(Type::Bool, cond[0], span)?;
            let next = region_types(functions, body, &state, depth + 1, span)?;
            arity(state.len(), next.len(), span)?;
            for (expected, actual) in state.iter().zip(next) {
                require_type(*expected, actual, span)?;
            }
            let result = region_types(functions, finish, &state, depth + 1, span)?;
            arity(1, result.len(), span)?;
            Ok(result[0])
        }
    }
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
        let types = instructions_types(
            &program.functions,
            &function.instructions,
            &function.parameters,
            0,
        )?;
        require_type(
            function.result_type,
            value_type(&types, function.result, function.return_span)?,
            function.return_span,
        )?;
    }
    Ok(ValidatedProgram(program))
}
