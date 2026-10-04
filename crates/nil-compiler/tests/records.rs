use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::{FunctionId, Type};

fn reject(source: &str, code: &str) {
    assert_eq!(
        compile_with_profile(source, SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        code,
        "{source}"
    );
}
#[test]
fn record_constructor_arity_and_field_types_are_checked() {
    reject("record R(x:i)\n=R().x", "E006");
    reject("record R(x:i)\n=R(1,2).x", "E006");
    reject("record R(x:i)\n=R(true).x", "E007");
    reject("record R(x:i)\n=R(1){x:true}.x", "E007");
}
#[test]
fn unknown_duplicate_and_forward_record_fields_report_e023() {
    for source in [
        "record R(x:i,x:s)\n=0",
        "record R(x:i)\nrecord R(y:i)\n=0",
        "record R(x:R)\n=0",
        "record R(x:Later)\nrecord Later(y:i)\n=0",
        "record R(x:i)\n=R(1).missing",
        "record R(x:i)\n=R(1){missing:2}.x",
        "record R(x:i)\n=1.x",
        "record R(x:i)\n=(1).x",
        "=0\nrecord R(x:i)",
    ] {
        reject(
            source,
            if source == "record R(x:i)\n=1.x" {
                "E001"
            } else {
                "E023"
            },
        );
    }
}
#[test]
fn records_are_nominal_and_earlier_profiles_remain_unchanged() {
    reject("record R(x:i)\nrecord S(x:i)\n=b(R(1))\n(S)=a.x", "E007");
    for profile in [
        SourceProfile::ExprV0,
        SourceProfile::ExprV1,
        SourceProfile::ExprV2,
        SourceProfile::ExprV3,
        SourceProfile::ExprV4,
        SourceProfile::LinesV0,
    ] {
        assert!(compile_with_profile("record R(x:i)\n=R(1).x", profile).is_err());
    }
}
#[test]
fn dynamic_record_map_values_are_rejected_without_serialization() {
    reject("record R(x:s)\n=!size(!map[R]())", "E023");
    reject(
        "record Inner(x:v)\nrecord Outer(inner:Inner)\n=!size(!map[Outer]())",
        "E023",
    );
    reject("record R(x:i)\n=!buffer(1,R(2))", "E007");
}
#[test]
fn malformed_reference_entry_records_fail_before_projection() {
    let p = compile_with_profile("record R(x:i)\n(R)=a.x", SourceProfile::ExprV5).unwrap();
    for fields in [vec![], vec![Value::Bool(true)]] {
        let error = execute_values(
            &p.hir,
            FunctionId(0),
            &[Value::Record(Type::Record(0, 1), fields.into())],
            Limits::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, "E007");
    }
}
#[test]
fn bounded_record_layouts_prevent_recursive_or_exponential_expansion() {
    let fields = (0..17)
        .map(|i| format!("x{i}:256"))
        .collect::<Vec<_>>()
        .join(",");
    reject(&format!("record R({fields})\n=0"), "E023");
    let fields = (0..65)
        .map(|i| format!("x{i}:i"))
        .collect::<Vec<_>>()
        .join(",");
    reject(&format!("record R({fields})\n=0"), "E023");
    let mut source = "record R0(x:i)\n".to_string();
    for i in 1..33 {
        source.push_str(&format!("record R{i}(x:R{})\n", i - 1));
    }
    source.push_str("=0");
    reject(&source, "E023");
}

#[test]
fn record_constructor_nesting_reaches_diagnostic_before_stack_exhaustion() {
    let source = format!("record R(x:i)\n={}0{}", "R(".repeat(129), ")".repeat(129));
    let error = compile_with_profile(&source, SourceProfile::ExprV5).unwrap_err();
    assert_eq!(error.code, "E001");
    assert!(error.message.contains("nesting limit"));
}
