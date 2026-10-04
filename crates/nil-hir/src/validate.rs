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
        Type::Buffer | Type::Bytes | Type::RecordBuffer(..) => Ok(()),
        Type::Array(len) => array_length(len, span),
        actual => Err(
            Diagnostic::new("E007", Phase::Check, span, "array operand required")
                .mismatch("Array", format!("{actual:?}")),
        ),
    }
}
fn instructions_types(
    functions: &[Function],
    records: &[RecordDefinition],
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
        let ty = operation_type_with_records(
            functions,
            records,
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
    records: &[RecordDefinition],
    region: &Region,
    inputs: &[Type],
    depth: usize,
    span: Option<Span>,
) -> Result<Vec<Type>, Diagnostic> {
    let types = instructions_types(functions, records, &region.instructions, inputs, depth)?;
    region
        .results
        .iter()
        .map(|id| value_type(&types, *id, span))
        .collect()
}

/// Infer and independently validate one semantic operation against its available
/// operands and complete function signatures. Used by typed lowering as well.
pub fn operation_type_with_records(
    functions: &[Function],
    records: &[RecordDefinition],
    operation: &Operation,
    types: &[Type],
    span: Option<Span>,
    depth: usize,
) -> Result<Type, Diagnostic> {
    match operation {
        Operation::PluginCall {
            provider,
            arguments,
        } => {
            if provider.program().program().records != records
                || provider.program().program().arithmetic != Arithmetic::Wrapping
            {
                return Err(plugin::error(
                    "plugin record registry or arithmetic mismatch",
                ));
            }
            let signature = provider.signature();
            arity(signature.parameters.len(), arguments.len(), span)?;
            for (expected, id) in signature.parameters.iter().zip(arguments) {
                require_type(*expected, value_type(types, *id, span)?, span)?;
            }
            Ok(signature.result_type)
        }
        Operation::Record { ty, fields } => {
            let definition = records::definition(records, *ty, span)?;
            arity(definition.fields.len(), fields.len(), span)?;
            for (field, id) in definition.fields.iter().zip(fields) {
                require_type(field.ty, value_type(types, *id, span)?, span)?;
            }
            Ok(*ty)
        }
        Operation::Field { record, field } => {
            records::field(records, value_type(types, *record, span)?, *field, span)
        }
        Operation::UpdateField {
            record,
            field,
            value,
        } => {
            let ty = value_type(types, *record, span)?;
            require_type(
                records::field(records, ty, *field, span)?,
                value_type(types, *value, span)?,
                span,
            )?;
            Ok(ty)
        }
        Operation::RecordBuffer { ty, length, fill } => {
            records::validate_type(records, *ty, span)?;
            let Type::RecordBuffer(id, slots) = ty else {
                return Err(records::error(span, "record buffer type required"));
            };
            require_type(Type::I64, value_type(types, *length, span)?, span)?;
            require_type(
                Type::Record(*id, *slots),
                value_type(types, *fill, span)?,
                span,
            )?;
            Ok(*ty)
        }
        Operation::RecordMap(ty) => {
            records::validate_type(records, *ty, span)?;
            if !matches!(ty, Type::MapRecord(..)) {
                return Err(records::error(span, "record map type required"));
            }
            Ok(*ty)
        }

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
                Intrinsic::Sort => {
                    let ty = types.first().copied().unwrap_or(Type::Bytes);
                    if !matches!(
                        ty,
                        Type::Buffer
                            | Type::Bytes
                            | Type::MapI64
                            | Type::MapBytes
                            | Type::MapRecord(..)
                            | Type::RecordBuffer(..)
                    ) {
                        return Err(Diagnostic::new(
                            "E007",
                            Phase::Check,
                            span,
                            "sortable dynamic collection required",
                        ));
                    }
                    (vec![ty, Type::I64], ty)
                }
                Intrinsic::Map => (vec![], Type::MapI64),
                Intrinsic::ByteMap => (vec![], Type::MapBytes),
                Intrinsic::Insert
                | Intrinsic::Put
                | Intrinsic::Get
                | Intrinsic::Has
                | Intrinsic::Size
                | Intrinsic::Key => {
                    let map = types.first().copied().unwrap_or(Type::MapI64);
                    let value = map.map_value().ok_or_else(|| {
                        Diagnostic::new("E007", Phase::Check, span, "map operand required")
                    })?;
                    match op {
                        Intrinsic::Insert | Intrinsic::Put => (vec![map, Type::Bytes, value], map),
                        Intrinsic::Get => (vec![map, Type::Bytes], value),
                        Intrinsic::Has => (vec![map, Type::Bytes], Type::Bool),
                        Intrinsic::Size => (vec![map], Type::I64),
                        Intrinsic::Key => (vec![map, Type::I64], Type::Bytes),
                        _ => unreachable!(),
                    }
                }
                Intrinsic::Buffer => (vec![Type::I64, Type::I64], Type::Buffer),
                Intrinsic::Bytes => (vec![Type::I64, Type::I64], Type::Bytes),
                Intrinsic::Concat | Intrinsic::Equal => {
                    let ty = types.first().copied().unwrap_or(Type::Bytes);
                    if !matches!(ty, Type::Buffer | Type::Bytes)
                        && !matches!(
                            (op, ty),
                            (Intrinsic::Concat | Intrinsic::Slice, Type::RecordBuffer(..))
                        )
                    {
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
                    if !matches!(ty, Type::Buffer | Type::Bytes)
                        && !matches!(
                            (op, ty),
                            (Intrinsic::Concat | Intrinsic::Slice, Type::RecordBuffer(..))
                        )
                    {
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
                Intrinsic::ToI64
                | Intrinsic::ToU64
                | Intrinsic::ToU128
                | Intrinsic::ToF64
                | Intrinsic::TruncI64
                | Intrinsic::TruncU64 => {
                    let input = types.first().copied().unwrap_or(Type::I64);
                    if !input.is_numeric()
                        || (matches!(op, Intrinsic::TruncI64 | Intrinsic::TruncU64)
                            && input == Type::F64)
                    {
                        return Err(Diagnostic::new(
                            "E007",
                            Phase::Check,
                            span,
                            "numeric conversion operand required",
                        ));
                    }
                    let target = match op {
                        Intrinsic::ToI64 | Intrinsic::TruncI64 => Type::I64,
                        Intrinsic::ToU64 | Intrinsic::TruncU64 => Type::U64,
                        Intrinsic::ToU128 => Type::U128,
                        _ => Type::F64,
                    };
                    (vec![input], target)
                }
                Intrinsic::Bits => (vec![Type::F64], Type::U64),
                Intrinsic::FloatBits => (vec![Type::U64], Type::F64),
                Intrinsic::ParseF64 => (vec![Type::Bytes], Type::F64),
                Intrinsic::ParseU64 => (vec![Type::Bytes], Type::U64),
                Intrinsic::ParseU128 => (vec![Type::Bytes], Type::U128),
                Intrinsic::Format => {
                    let input = types.first().copied().unwrap_or(Type::I64);
                    if !input.is_numeric() {
                        return Err(Diagnostic::new(
                            "E007",
                            Phase::Check,
                            span,
                            "numeric format operand required",
                        ));
                    }
                    (vec![input], Type::Bytes)
                }
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
        Operation::Unsigned { value, ty }
            if matches!(ty, Type::U64 | Type::U128)
                && (*ty == Type::U128 || *value <= u64::MAX as u128) =>
        {
            Ok(*ty)
        }
        Operation::Unsigned { .. } => Err(Diagnostic::new(
            "E007",
            Phase::Check,
            span,
            "invalid unsigned constant",
        )),
        Operation::Float(bits) => {
            if f64::from_bits(*bits).is_nan() && *bits != 0x7ff8000000000000 {
                return Err(Diagnostic::new(
                    "E007",
                    Phase::Check,
                    span,
                    "noncanonical NaN constant",
                ));
            }
            Ok(Type::F64)
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
            Ok(match value_type(types, *array, span)? {
                Type::RecordBuffer(id, slots) => Type::Record(id, slots),
                _ => Type::I64,
            })
        }
        Operation::Replace {
            array,
            index,
            value,
        } => {
            array_type(types, *array, span)?;
            require_type(Type::I64, value_type(types, *index, span)?, span)?;
            let element = match value_type(types, *array, span)? {
                Type::RecordBuffer(id, slots) => Type::Record(id, slots),
                _ => Type::I64,
            };
            require_type(element, value_type(types, *value, span)?, span)?;
            Ok(value_type(types, *array, span)?)
        }
        Operation::Binary { lhs, rhs, .. } | Operation::Compare { lhs, rhs, .. } => {
            let input = value_type(types, *lhs, span)?;
            if !input.is_numeric() {
                // Preserve existing profiles' mismatch diagnostics.
                require_type(Type::I64, input, span)?;
            }
            require_type(input, value_type(types, *rhs, span)?, span)?;
            Ok(if matches!(operation, Operation::Compare { .. }) {
                Type::Bool
            } else {
                input
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
            let yes = region_types(functions, records, then_region, types, depth + 1, span)?;
            let no = region_types(functions, records, else_region, types, depth + 1, span)?;
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
            let cond = region_types(functions, records, condition, &state, depth + 1, span)?;
            arity(1, cond.len(), span)?;
            require_type(Type::Bool, cond[0], span)?;
            let next = region_types(functions, records, body, &state, depth + 1, span)?;
            arity(state.len(), next.len(), span)?;
            for (expected, actual) in state.iter().zip(next) {
                require_type(*expected, actual, span)?;
            }
            let result = region_types(functions, records, finish, &state, depth + 1, span)?;
            arity(1, result.len(), span)?;
            Ok(result[0])
        }
    }
}

pub fn operation_type(
    functions: &[Function],
    operation: &Operation,
    types: &[Type],
    span: Option<Span>,
    depth: usize,
) -> Result<Type, Diagnostic> {
    operation_type_with_records(functions, &[], operation, types, span, depth)
}

pub fn validate(mut program: Program) -> Result<ValidatedProgram, Diagnostic> {
    records::validate_definitions(&program.records)?;
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
            records::validate_type(&program.records, *ty, function.return_span)?;
            if let Type::Array(len) = ty {
                array_length(*len, function.return_span)?;
            }
        }
        let types = instructions_types(
            &program.functions,
            &program.records,
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
    plugin::normalize(&mut program);
    Ok(ValidatedProgram(program))
}
