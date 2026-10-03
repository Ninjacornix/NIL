use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
    numeric::NAN_BITS,
};
use nil_hir::{FunctionId, Operation, Type};
#[test]
fn numeric_literals_and_signatures_have_explicit_types() {
    let p = compile_with_profile(
        "(u64,u128,f64):f64=!f64(a)+!f64(b)+c",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert_eq!(
        p.hir.program().functions[0].parameters,
        vec![Type::U64, Type::U128, Type::F64]
    );
    assert_eq!(p.hir.program().functions[0].result_type, Type::F64);
}
#[test]
fn mixed_width_arithmetic_requires_explicit_conversion() {
    for s in [
        ":u64=1u64+1",
        ":u128=1u128+1u64",
        ":f64=1.0+1",
        ":b=1.0==1u64",
    ] {
        assert_eq!(
            compile_with_profile(s, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E007"
        );
    }
}
#[test]
fn numeric_literal_ranges_and_spellings_are_checked() {
    for s in [
        ":u64=18446744073709551616u64",
        ":u128=340282366920938463463374607431768211456u128",
        ":u64=01u64",
        ":f64=1e9999",
        ":f64=1.",
    ] {
        assert_eq!(
            compile_with_profile(s, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E001",
            "{s}"
        );
    }
}
#[test]
fn numeric_surface_is_unavailable_in_earlier_profiles() {
    for profile in [
        SourceProfile::ExprV0,
        SourceProfile::ExprV1,
        SourceProfile::ExprV2,
        SourceProfile::ExprV3,
        SourceProfile::ExprV4,
    ] {
        for s in [":u64=1u64", ":f64=1.0", "=!i64(1)"] {
            assert!(compile_with_profile(s, profile).is_err(), "{profile:?} {s}");
        }
    }
}
#[test]
fn hir_rejects_invalid_unsigned_and_noncanonical_nan_constants() {
    let p = compile_with_profile(":u64=1u64", SourceProfile::ExprV5).unwrap();
    let mut raw = p.hir.program().clone();
    raw.functions[0].instructions[0].operation = Operation::Unsigned {
        value: u64::MAX as u128 + 1,
        ty: Type::U64,
    };
    assert_eq!(nil_hir::validate(raw).unwrap_err().code, "E007");
    let p = compile_with_profile(":f64=1.0", SourceProfile::ExprV5).unwrap();
    let mut raw = p.hir.program().clone();
    raw.functions[0].instructions[0].operation = Operation::Float(0x7ff0000000000001);
    assert_eq!(nil_hir::validate(raw).unwrap_err().code, "E007");
}
#[test]
fn reference_entry_nan_payload_is_canonicalized() {
    let p = compile_with_profile("(f64):u64=!bits(a)", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        execute_values(
            &p.hir,
            FunctionId(0),
            &[Value::F64(0xfff0000000000001)],
            Limits::default()
        )
        .unwrap(),
        Value::U64(NAN_BITS)
    );
}

#[test]
fn numeric_literals_do_not_bypass_the_parser_nesting_limit() {
    for atom in ["1u128", "1.0"] {
        let source = format!("={}{atom}{}", "(".repeat(129), ")".repeat(129));
        let error = compile_with_profile(&source, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(error.code, "E001");
        assert!(error.message.contains("nesting limit"));
    }
}
