//! Shipped library implementation; not an intrinsic evaluator/backend switch.
//! A portable HIR recipe is used by both reference and native plugin linking.
use crate::*;

pub fn equality(ty: Type, records: &[RecordDefinition]) -> plugin::Provider {
    let i = |operation, ty| Instruction {
        operation,
        ty,
        span: None,
    };
    let v = ValueId;
    // State: lhs, rhs, index, still_equal. Stop on the first mismatch.
    let condition = Region {
        instructions: vec![
            i(Operation::Length(v(0)), Type::I64),
            i(
                Operation::Compare {
                    op: CompareOp::Lt,
                    lhs: v(2),
                    rhs: v(4),
                },
                Type::Bool,
            ),
            i(
                Operation::If {
                    condition: v(5),
                    then_region: Region {
                        instructions: vec![],
                        results: vec![v(3)],
                    },
                    else_region: Region {
                        instructions: vec![i(Operation::Boolean(false), Type::Bool)],
                        results: vec![v(6)],
                    },
                },
                Type::Bool,
            ),
        ],
        results: vec![v(6)],
    };
    let body = Region {
        instructions: vec![
            i(
                Operation::Index {
                    array: v(0),
                    index: v(2),
                },
                Type::I64,
            ),
            i(
                Operation::Index {
                    array: v(1),
                    index: v(2),
                },
                Type::I64,
            ),
            i(
                Operation::Compare {
                    op: CompareOp::Eq,
                    lhs: v(4),
                    rhs: v(5),
                },
                Type::Bool,
            ),
            i(Operation::Constant(1), Type::I64),
            i(
                Operation::Binary {
                    op: BinaryOp::Add,
                    lhs: v(2),
                    rhs: v(7),
                },
                Type::I64,
            ),
        ],
        results: vec![v(0), v(1), v(8), v(6)],
    };
    let entry = Function {
        parameters: vec![ty, ty],
        result_type: Type::Bool,
        instructions: vec![
            i(Operation::Length(v(0)), Type::I64),
            i(Operation::Length(v(1)), Type::I64),
            i(
                Operation::Compare {
                    op: CompareOp::Eq,
                    lhs: v(2),
                    rhs: v(3),
                },
                Type::Bool,
            ),
            i(Operation::Constant(0), Type::I64),
            i(Operation::Boolean(true), Type::Bool),
            i(
                Operation::If {
                    condition: v(4),
                    then_region: Region {
                        instructions: vec![i(
                            Operation::Loop {
                                initial: vec![v(0), v(1), v(5), v(6)],
                                condition,
                                body,
                                finish: Region {
                                    instructions: vec![],
                                    results: vec![v(3)],
                                },
                            },
                            Type::Bool,
                        )],
                        results: vec![v(7)],
                    },
                    else_region: Region {
                        instructions: vec![i(Operation::Boolean(false), Type::Bool)],
                        results: vec![v(7)],
                    },
                },
                Type::Bool,
            ),
        ],
        result: v(7),
        return_span: None,
    };
    plugin::Provider::new(
        0,
        0,
        validate(Program {
            records: records.to_vec(),
            arithmetic: Arithmetic::Wrapping,
            functions: vec![entry],
        })
        .expect("shipped equality HIR"),
        FunctionId(0),
    )
    .expect("shipped borrowing provider")
}
