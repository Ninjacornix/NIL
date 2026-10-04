use nil_compiler::{SourceProfile, compile_with_profile};
#[test]
fn record_collection_signatures_reject_wrong_fill_and_elements() {
    for source in [
        "record Cell(value:i)\n=#!buffer[Cell](1,2)",
        "record Cell(value:i)\nrecord Other(value:i)\n=#!buffer[Cell](1,Cell(1))[0:Other(2)]",
    ] {
        assert_eq!(
            compile_with_profile(source, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E007"
        );
    }
}
#[test]
fn cyclic_and_forward_record_collection_types_are_rejected() {
    for source in [
        "record Node(children:v[Node])\n=0",
        "record Row(cells:v[Cell])\nrecord Cell(value:i)\n=0",
    ] {
        assert_eq!(
            compile_with_profile(source, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E023"
        );
    }
}
#[test]
fn map_dynamic_record_boundary_remains_explicitly_rejected() {
    assert_eq!(
        compile_with_profile(
            "record Row(data:v)\n=!size(!map[Row]())",
            SourceProfile::ExprV5
        )
        .unwrap_err()
        .code,
        "E023"
    );
}
#[test]
fn earlier_profiles_do_not_gain_record_collection_syntax() {
    for profile in [SourceProfile::ExprV3, SourceProfile::ExprV4] {
        assert!(
            compile_with_profile("record Cell(value:i)\n=#!buffer[Cell](0,Cell(0))", profile)
                .is_err()
        );
    }
}
#[test]
fn hir_revalidates_collection_layout_instead_of_trusting_cached_slots() {
    let mut p = compile_with_profile(
        "record Cell(value:i)\n=#!buffer[Cell](1,Cell(0))",
        SourceProfile::ExprV5,
    )
    .unwrap()
    .hir
    .program()
    .clone();
    let i = p.functions[0]
        .instructions
        .iter_mut()
        .find(|i| matches!(i.operation, nil_hir::Operation::RecordBuffer { .. }))
        .unwrap();
    let nil_hir::Operation::RecordBuffer { ty, .. } = &mut i.operation else {
        unreachable!()
    };
    *ty = nil_hir::Type::RecordBuffer(0, usize::MAX);
    assert_eq!(nil_hir::validate(p).unwrap_err().code, "E023");
}
