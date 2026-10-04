use nil_hir::*;
fn program() -> Program {
    Program {
        records: vec![RecordDefinition {
            name: "R".into(),
            fields: vec![RecordField {
                name: "x".into(),
                ty: Type::I64,
            }],
        }],
        arithmetic: Arithmetic::Wrapping,
        functions: vec![Function {
            parameters: vec![Type::Record(0, 1)],
            result_type: Type::I64,
            instructions: vec![Instruction {
                operation: Operation::Field {
                    record: ValueId(0),
                    field: 0,
                },
                ty: Type::I64,
                span: None,
            }],
            result: ValueId(1),
            return_span: None,
        }],
    }
}
#[test]
fn independent_hir_validation_rejects_bad_field_indices_and_cached_slots() {
    let mut p = program();
    assert!(validate(p.clone()).is_ok());
    p.functions[0].instructions[0].operation = Operation::Field {
        record: ValueId(0),
        field: 1,
    };
    assert_eq!(validate(p).unwrap_err().code, "E023");
    let mut p = program();
    p.functions[0].parameters[0] = Type::Record(0, 2);
    assert_eq!(validate(p).unwrap_err().code, "E023");
}
#[test]
fn independent_hir_validation_rejects_cycles_and_untyped_map_children() {
    let mut p = program();
    p.records[0].fields[0].ty = Type::Record(0, 1);
    assert_eq!(validate(p).unwrap_err().code, "E023");
    let mut p = program();
    p.records[0].fields[0].ty = Type::Bytes;
    p.functions[0].parameters[0] = Type::MapRecord(0, 1);
    assert_eq!(validate(p).unwrap_err().code, "E023");
}
#[test]
fn record_liveness_exposes_every_constructor_update_and_projection_operand() {
    let instructions = vec![
        Instruction {
            operation: Operation::Record {
                ty: Type::Record(0, 1),
                fields: vec![ValueId(0), ValueId(1)],
            },
            ty: Type::Record(0, 1),
            span: None,
        },
        Instruction {
            operation: Operation::UpdateField {
                record: ValueId(2),
                field: 0,
                value: ValueId(1),
            },
            ty: Type::Record(0, 1),
            span: None,
        },
    ];
    assert_eq!(
        liveness::last_uses(&instructions, &[ValueId(3)], 2),
        vec![Some(0), Some(1), Some(1), Some(2)]
    );
}
