use nil_hir::{
    Arithmetic, BinaryOp, Function, Instruction, Operation, Program, Type, UnaryOp, ValueId,
    validate,
};
fn check(
    op: Operation,
    inputs: Vec<Type>,
    ty: Type,
) -> Result<nil_hir::ValidatedProgram, nil_hir::Diagnostic> {
    let result = ValueId(inputs.len());
    validate(Program {
        records: vec![],
        arithmetic: Arithmetic::Wrapping,
        functions: vec![Function {
            parameters: inputs,
            result_type: ty,
            instructions: vec![Instruction {
                operation: op,
                ty,
                span: None,
            }],
            result,
            return_span: None,
        }],
    })
}
#[test]
fn unary_hir_validation_rejects_wrong_types_and_forward_values() {
    assert_eq!(
        check(
            Operation::Unary {
                op: UnaryOp::Not,
                value: ValueId(0)
            },
            vec![Type::I64],
            Type::Bool
        )
        .unwrap_err()
        .code,
        "E007"
    );
    assert_eq!(
        check(
            Operation::Unary {
                op: UnaryOp::Abs,
                value: ValueId(1)
            },
            vec![Type::I64],
            Type::I64
        )
        .unwrap_err()
        .code,
        "E005"
    );
    assert_eq!(
        check(
            Operation::Unary {
                op: UnaryOp::Sqrt,
                value: ValueId(0)
            },
            vec![Type::I64],
            Type::I64
        )
        .unwrap_err()
        .code,
        "E007"
    );
}
#[test]
fn shift_hir_validation_requires_an_i64_count() {
    assert_eq!(
        check(
            Operation::Binary {
                op: BinaryOp::Shl,
                lhs: ValueId(0),
                rhs: ValueId(1)
            },
            vec![Type::U128, Type::U64],
            Type::U128
        )
        .unwrap_err()
        .code,
        "E007"
    );
    assert!(
        check(
            Operation::Binary {
                op: BinaryOp::Shl,
                lhs: ValueId(0),
                rhs: ValueId(1)
            },
            vec![Type::U128, Type::I64],
            Type::U128
        )
        .is_ok()
    );
}
#[test]
fn float_hir_cannot_use_integer_remainder_or_bitwise_ops() {
    for op in [
        BinaryOp::Rem,
        BinaryOp::BitAnd,
        BinaryOp::BitOr,
        BinaryOp::BitXor,
        BinaryOp::Shl,
        BinaryOp::Shr,
    ] {
        assert_eq!(
            check(
                Operation::Binary {
                    op,
                    lhs: ValueId(0),
                    rhs: ValueId(1)
                },
                vec![Type::F64, Type::F64],
                Type::F64
            )
            .unwrap_err()
            .code,
            "E007"
        );
    }
}
#[test]
fn scalar_primitives_remain_borrowing_without_erasing_traps() {
    let p = check(
        Operation::Unary {
            op: UnaryOp::Abs,
            value: ValueId(0),
        },
        vec![Type::I64],
        Type::I64,
    )
    .unwrap();
    assert!(nil_hir::borrowing::Summaries::analyze(&p).function(0));
    assert_eq!(
        nil_hir::liveness::last_uses(&p.program().functions[0].instructions, &[ValueId(1)], 1),
        vec![Some(0), Some(1)]
    );
}
