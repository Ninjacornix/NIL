use nil_hir::*;

fn program(parameters: Vec<Type>) -> Program {
    let count = parameters.len();
    Program {
        arithmetic: Arithmetic::Checked,
        functions: vec![Function {
            parameters,
            result_type: Type::I64,
            instructions: vec![Instruction {
                operation: Operation::Constant(42),
                ty: Type::I64,
                span: None,
            }],
            result: ValueId(count),
            return_span: None,
        }],
    }
}

#[test]
fn function_input_limits_accept_parameter_and_slot_boundaries() {
    for parameters in [vec![Type::Array(0); 4096], vec![Type::Array(256); 16]] {
        let p = program(parameters);
        assert!(validate(p).is_ok());
    }
}

#[test]
fn function_input_limits_reject_excess_empty_parameters_and_flattened_slots() {
    for parameters in [vec![Type::Array(0); 4097], vec![Type::Array(256); 17]] {
        let p = program(parameters);
        assert_eq!(validate(p).unwrap_err().code, "E008");
    }
}
