use nil_hir::*;
fn function() -> Function {
    Function {
        parameters: vec![Type::I64],
        result_type: Type::I64,
        instructions: vec![],
        result: ValueId(0),
        return_span: None,
    }
}
fn program(instructions: Vec<Instruction>, result: usize) -> Program {
    let mut f = function();
    f.instructions = instructions;
    f.result = ValueId(result);
    Program {
        arithmetic: nil_hir::Arithmetic::Checked,
        functions: vec![f],
    }
}
fn inst(operation: Operation) -> Instruction {
    Instruction {
        operation,
        ty: Type::I64,
        span: None,
    }
}
#[test]
fn accepts_syntax_independent_hir() {
    let p = validate(program(vec![inst(Operation::Constant(42))], 1)).unwrap();
    assert_eq!(p.program().functions.len(), 1);
}
#[test]
fn rejects_empty_program() {
    assert_eq!(
        validate(Program {
            arithmetic: nil_hir::Arithmetic::Checked,
            functions: vec![]
        })
        .unwrap_err()
        .code,
        "E007"
    );
}
#[test]
fn rejects_missing_return_value() {
    assert_eq!(validate(program(vec![], 1)).unwrap_err().code, "E005");
}
#[test]
fn rejects_forward_self_and_out_of_range_values() {
    for value in [1, 2, usize::MAX] {
        let p = program(
            vec![inst(Operation::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(value),
            })],
            1,
        );
        assert_eq!(validate(p).unwrap_err().code, "E005");
    }
}
#[test]
fn rejects_invalid_call_target_and_arity_and_operand() {
    for (target, args, code) in [
        (1, vec![ValueId(0)], "E004"),
        (0, vec![], "E006"),
        (0, vec![ValueId(1)], "E005"),
    ] {
        let p = program(
            vec![inst(Operation::Call {
                function: FunctionId(target),
                arguments: args,
            })],
            1,
        );
        assert_eq!(validate(p).unwrap_err().code, code);
    }
}
#[test]
fn validates_all_functions_not_only_entry() {
    let mut bad = function();
    bad.result = ValueId(100);
    assert_eq!(
        validate(Program {
            arithmetic: nil_hir::Arithmetic::Checked,
            functions: vec![function(), bad]
        })
        .unwrap_err()
        .code,
        "E005"
    );
}

#[test]
fn call_arity_diagnostic_is_structured_and_canonical() {
    for arguments in [vec![], vec![ValueId(0), ValueId(0)]] {
        let actual = arguments.len();
        let p = program(
            vec![inst(Operation::Call {
                function: FunctionId(0),
                arguments,
            })],
            1,
        );
        assert_eq!(
            validate(p).unwrap_err(),
            Diagnostic::new("E006", Phase::Check, None, "arity mismatch").mismatch(1, actual),
        );
    }
}

#[test]
fn array_operations_cannot_bypass_operand_or_length_validation() {
    let check = |parameters: Vec<Type>, operation: Operation, ty: Type| {
        let count = parameters.len();
        validate(Program {
            arithmetic: Arithmetic::Wrapping,
            functions: vec![Function {
                parameters,
                result_type: ty,
                instructions: vec![Instruction {
                    operation,
                    ty,
                    span: None,
                }],
                result: ValueId(count),
                return_span: None,
            }],
        })
    };
    for (params, op, ty, code) in [
        (
            vec![Type::Bool],
            Operation::Array(vec![ValueId(0)]),
            Type::Array(1),
            "E007",
        ),
        (
            vec![Type::I64],
            Operation::Repeat {
                value: ValueId(0),
                len: 257,
            },
            Type::Array(257),
            "E008",
        ),
        (
            vec![Type::I64],
            Operation::Length(ValueId(0)),
            Type::I64,
            "E007",
        ),
        (
            vec![Type::Array(2), Type::Bool],
            Operation::Index {
                array: ValueId(0),
                index: ValueId(1),
            },
            Type::I64,
            "E007",
        ),
        (
            vec![Type::Array(2), Type::I64, Type::Bool],
            Operation::Replace {
                array: ValueId(0),
                index: ValueId(1),
                value: ValueId(2),
            },
            Type::Array(2),
            "E007",
        ),
        (
            vec![Type::Array(usize::MAX), Type::I64],
            Operation::Length(ValueId(0)),
            Type::I64,
            "E008",
        ),
        (
            vec![Type::Array(257)],
            Operation::Length(ValueId(0)),
            Type::I64,
            "E008",
        ),
        (
            vec![Type::Array(256); 17],
            Operation::Length(ValueId(0)),
            Type::I64,
            "E008",
        ),
        (
            vec![Type::I64],
            Operation::Array(vec![ValueId(1)]),
            Type::Array(1),
            "E005",
        ),
        (
            vec![Type::I64],
            Operation::Array(vec![ValueId(0)]),
            Type::I64,
            "E007",
        ),
    ] {
        assert_eq!(check(params, op, ty).unwrap_err().code, code);
    }
    assert!(check(vec![], Operation::Array(vec![]), Type::Array(0)).is_ok());
    assert_eq!(
        check(
            vec![Type::I64],
            Operation::Array(vec![ValueId(0); 257]),
            Type::Array(257)
        )
        .unwrap_err()
        .code,
        "E008"
    );
}

#[test]
fn validates_new_intrinsic_signatures_without_surface_syntax() {
    for (op, parameters, result_type) in [
        (
            Intrinsic::Equal,
            vec![Type::Buffer, Type::Buffer],
            Type::Bool,
        ),
        (
            Intrinsic::Find,
            vec![Type::Bytes, Type::I64, Type::I64],
            Type::I64,
        ),
        (
            Intrinsic::ParseBuffer,
            vec![Type::Bytes, Type::Bytes],
            Type::Buffer,
        ),
    ] {
        let count = parameters.len();
        validate(Program {
            arithmetic: Arithmetic::Wrapping,
            functions: vec![Function {
                parameters,
                result_type,
                instructions: vec![Instruction {
                    operation: Operation::Intrinsic {
                        op,
                        arguments: (0..count).map(ValueId).collect(),
                    },
                    ty: result_type,
                    span: None,
                }],
                result: ValueId(count),
                return_span: None,
            }],
        })
        .unwrap();
    }
}

#[test]
fn rejects_new_intrinsic_mixed_sequences_and_misdeclared_result_types() {
    for (op, parameters, result_type) in [
        (
            Intrinsic::Equal,
            vec![Type::Buffer, Type::Bytes],
            Type::Bool,
        ),
        (Intrinsic::Equal, vec![Type::Bytes, Type::Bytes], Type::I64),
        (
            Intrinsic::Find,
            vec![Type::Bytes, Type::Bool, Type::I64],
            Type::I64,
        ),
        (
            Intrinsic::ParseBuffer,
            vec![Type::Bytes, Type::I64],
            Type::Buffer,
        ),
    ] {
        let count = parameters.len();
        assert!(
            validate(Program {
                arithmetic: Arithmetic::Wrapping,
                functions: vec![Function {
                    parameters,
                    result_type,
                    instructions: vec![Instruction {
                        operation: Operation::Intrinsic {
                            op,
                            arguments: (0..count).map(ValueId).collect()
                        },
                        ty: result_type,
                        span: None
                    }],
                    result: ValueId(count),
                    return_span: None,
                }]
            })
            .is_err()
        );
    }
}
