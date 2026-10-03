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
            Diagnostic::new("E006", Phase::Check, span, "arity mismatch")
                .mismatch(expected, actual),
        );
    }
    Ok(())
}
fn array_length(len: usize, span: Option<Span>) -> Result<(), Diagnostic> {
    if len > MAX_ARRAY_LEN {
        return Err(Diagnostic::new(
            "E008",
            Phase::Check,
            span,
            "array length exceeds 256",
        ));
    }
    Ok(())
}
fn array_type(types: &[Type], id: ValueId, span: Option<Span>) -> Result<(), Diagnostic> {
    match value_type(types, id, span)? {
        Type::Buffer | Type::Bytes => Ok(()),
        Type::Array(len) => array_length(len, span),
        actual => Err(
            Diagnostic::new("E007", Phase::Check, span, "array operand required")
                .mismatch("Array", format!("{actual:?}")),
        ),
    }
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
        Operation::Bytes(bytes) => {
            if bytes.len() > MAX_DYNAMIC_BYTES {
                return Err(Diagnostic::new(
                    "E008",
                    Phase::Check,
                    span,
                    "byte literal exceeds allocation limit",
                ));
            }
            Ok(Type::Bytes)
        }
        Operation::Intrinsic { op, arguments } => {
            let types = arguments
                .iter()
                .map(|id| value_type(types, *id, span))
                .collect::<Result<Vec<_>, _>>()?;
            let (expected, result) = match op {
                Intrinsic::Buffer => (vec![Type::I64, Type::I64], Type::Buffer),
                Intrinsic::Bytes => (vec![Type::I64, Type::I64], Type::Bytes),
                Intrinsic::Concat | Intrinsic::Equal => {
                    let ty = types.first().copied().unwrap_or(Type::Bytes);
                    if !matches!(ty, Type::Buffer | Type::Bytes) {
                        return Err(Diagnostic::new(
                            "E007",
                            Phase::Check,
                            span,
                            "sequence required",
                        ));
                    }
                    (
                        vec![ty, ty],
                        if *op == Intrinsic::Equal {
                            Type::Bool
                        } else {
                            ty
                        },
                    )
                }
                Intrinsic::Slice | Intrinsic::Find => {
                    let ty = types.first().copied().unwrap_or(Type::Bytes);
                    if !matches!(ty, Type::Buffer | Type::Bytes) {
                        return Err(Diagnostic::new(
                            "E007",
                            Phase::Check,
                            span,
                            "sequence required",
                        ));
                    }
                    (
                        vec![ty, Type::I64, Type::I64],
                        if *op == Intrinsic::Find {
                            Type::I64
                        } else {
                            ty
                        },
                    )
                }
                Intrinsic::Format => (vec![Type::I64], Type::Bytes),
                Intrinsic::Parse => (vec![Type::Bytes], Type::I64),
                Intrinsic::ParseBuffer => (vec![Type::Bytes, Type::Bytes], Type::Buffer),
                Intrinsic::Read => (vec![Type::Bytes], Type::Bytes),
                Intrinsic::Write => (vec![Type::Bytes, Type::Bytes], Type::I64),
                Intrinsic::Out => (vec![Type::Bytes], Type::I64),
            };
            arity(expected.len(), types.len(), span)?;
            for (expected, actual) in expected.into_iter().zip(types) {
                require_type(expected, actual, span)?;
            }
            Ok(result)
        }
        Operation::Constant(_) => Ok(Type::I64),
        Operation::Boolean(_) => Ok(Type::Bool),
        Operation::Array(elements) => {
            array_length(elements.len(), span)?;
            for id in elements {
                require_type(Type::I64, value_type(types, *id, span)?, span)?;
            }
            Ok(Type::Array(elements.len()))
        }
        Operation::Repeat { value, len } => {
            array_length(*len, span)?;
            require_type(Type::I64, value_type(types, *value, span)?, span)?;
            Ok(Type::Array(*len))
        }
        Operation::Length(array) => {
            array_type(types, *array, span)?;
            Ok(Type::I64)
        }
        Operation::Index { array, index } => {
            array_type(types, *array, span)?;
            require_type(Type::I64, value_type(types, *index, span)?, span)?;
            Ok(Type::I64)
        }
        Operation::Replace {
            array,
            index,
            value,
        } => {
            array_type(types, *array, span)?;
            require_type(Type::I64, value_type(types, *index, span)?, span)?;
            require_type(Type::I64, value_type(types, *value, span)?, span)?;
            Ok(value_type(types, *array, span)?)
        }
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
    // Empty arrays consume no slots, so cap parameter count independently.
    if let Some(function) = program
        .functions
        .iter()
        .find(|function| function.parameters.len() > 4096)
    {
        return Err(Diagnostic::new(
            "E008",
            Phase::Check,
            function.return_span,
            "function input exceeds 4096 slots",
        ));
    }
    if program.functions.is_empty() {
        return Err(Diagnostic::new(
            "E007",
            Phase::Check,
            None,
            "program has no functions",
        ));
    }
    for function in &program.functions {
        if function
            .parameters
            .iter()
            .try_fold(0usize, |sum, ty| sum.checked_add(ty.slots()))
            .is_none_or(|slots| slots > 4096)
        {
            return Err(Diagnostic::new(
                "E008",
                Phase::Check,
                function.return_span,
                "function input exceeds 4096 slots",
            ));
        }
        for ty in function
            .parameters
            .iter()
            .chain(std::iter::once(&function.result_type))
        {
            if let Type::Array(len) = ty {
                array_length(*len, function.return_span)?;
            }
        }
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
