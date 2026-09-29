use nil_hir::*;
fn instruction(operation: Operation, ty: Type) -> Instruction {
    Instruction {
        operation,
        ty,
        span: None,
    }
}
fn region(instructions: Vec<Instruction>, results: &[usize]) -> Region {
    Region {
        instructions,
        results: results.iter().copied().map(ValueId).collect(),
    }
}
fn program(operation: Operation) -> Program {
    Program {
        functions: vec![Function {
            parameters: vec![Type::I64],
            result_type: Type::I64,
            instructions: vec![instruction(operation, Type::I64)],
            result: ValueId(1),
            return_span: None,
        }],
    }
}
fn loop_operation() -> Operation {
    Operation::Loop {
        initial: vec![ValueId(0)],
        condition: region(
            vec![instruction(Operation::Boolean(false), Type::Bool)],
            &[1],
        ),
        body: region(vec![], &[0]),
        finish: region(vec![], &[0]),
    }
}
#[test]
fn valid_regions_are_checked_without_source_syntax() {
    assert!(validate(program(loop_operation())).is_ok());
}
#[test]
fn loop_region_state_arity_and_types_are_enforced() {
    let mut operation = loop_operation();
    if let Operation::Loop { body, .. } = &mut operation {
        body.results.clear();
    }
    assert_eq!(validate(program(operation)).unwrap_err().code, "E006");
    let mut operation = loop_operation();
    if let Operation::Loop { body, .. } = &mut operation {
        *body = region(
            vec![instruction(Operation::Boolean(true), Type::Bool)],
            &[1],
        );
    }
    assert_eq!(validate(program(operation)).unwrap_err().code, "E007");
}
#[test]
fn loop_region_cannot_access_outer_temporaries() {
    let mut operation = loop_operation();
    if let Operation::Loop { finish, .. } = &mut operation {
        finish.results = vec![ValueId(1)];
    }
    assert_eq!(validate(program(operation)).unwrap_err().code, "E005");
}
#[test]
fn branch_values_cannot_escape_to_sibling_or_parent() {
    let yes = region(vec![instruction(Operation::Constant(42), Type::I64)], &[2]);
    let no = region(vec![], &[2]);
    let function = Function {
        parameters: vec![Type::I64],
        result_type: Type::I64,
        instructions: vec![
            instruction(Operation::Boolean(true), Type::Bool),
            instruction(
                Operation::If {
                    condition: ValueId(1),
                    then_region: yes,
                    else_region: no,
                },
                Type::I64,
            ),
        ],
        result: ValueId(2),
        return_span: None,
    };
    assert_eq!(
        validate(Program {
            functions: vec![function]
        })
        .unwrap_err()
        .code,
        "E005"
    );
    let mut p = program(loop_operation());
    p.functions[0].result = ValueId(2);
    assert_eq!(validate(p).unwrap_err().code, "E005");
}
#[test]
fn bool_annotations_and_region_condition_are_checked() {
    let mut operation = loop_operation();
    if let Operation::Loop { condition, .. } = &mut operation {
        condition.instructions[0].ty = Type::I64;
    }
    assert_eq!(validate(program(operation)).unwrap_err().code, "E007");
    let operation = Operation::If {
        condition: ValueId(0),
        then_region: region(vec![], &[0]),
        else_region: region(vec![], &[0]),
    };
    assert_eq!(validate(program(operation)).unwrap_err().code, "E007");
}
#[test]
fn missing_and_multiple_region_results_are_rejected() {
    for results in [vec![], vec![0, 0]] {
        let mut operation = loop_operation();
        if let Operation::Loop { finish, .. } = &mut operation {
            *finish = region(vec![], &results);
        }
        assert_eq!(validate(program(operation)).unwrap_err().code, "E006");
    }
}
#[test]
fn external_hir_nesting_is_bounded_independently_of_parser() {
    let mut nested = region(vec![], &[1]);
    for _ in 0..140 {
        nested = region(
            vec![instruction(
                Operation::If {
                    condition: ValueId(0),
                    then_region: nested,
                    else_region: region(vec![], &[1]),
                },
                Type::I64,
            )],
            &[2],
        );
    }
    let function = Function {
        parameters: vec![Type::Bool, Type::I64],
        result_type: Type::I64,
        instructions: nested.instructions,
        result: ValueId(2),
        return_span: None,
    };
    assert_eq!(
        validate(Program {
            functions: vec![function]
        })
        .unwrap_err()
        .code,
        "E008"
    );
}
