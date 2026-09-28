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
    Program { functions: vec![f] }
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
        validate(Program { functions: vec![] }).unwrap_err().code,
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
            functions: vec![function(), bad]
        })
        .unwrap_err()
        .code,
        "E005"
    );
}
