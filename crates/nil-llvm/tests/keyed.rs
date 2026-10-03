use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
use nil_hir::FunctionId;
use nil_llvm::{Optimization, Options};

fn parity(source: &str, expected: &str) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let value = execute_values(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            steps: 1_000_000,
            ..Default::default()
        },
    )
    .unwrap();
    let rendered = match value {
        Value::I64(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Bytes(v) => String::from_utf8(v.to_vec()).unwrap(),
        Value::Map(v) => v.render(),
        _ => panic!("unexpected result"),
    };
    assert_eq!(rendered, expected, "reference: {source}");
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                steps: 1_000_000,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(
            out.status.success(),
            "{source}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.stdout, format!("{expected}\n").as_bytes(), "{source}");
    }
}
fn failure(source: &str, code: &str) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let err = execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap_err();
    assert_eq!(err.code, code, "{source}");
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert!(!out.status.success());
        assert!(
            out.stderr.starts_with(code.as_bytes()),
            "{source}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
#[test]
fn empty_maps_and_membership_are_typed() {
    parity("=!size(!map())", "0");
    parity(":b=!has(!bytemap(),\"x\")", "false");
}
#[test]
fn map_integer_lookup_and_update_preserve_order() {
    parity(
        ":m=!put(!insert(!insert(!map(),\"b\",2),\"a\",1),\"b\",7)",
        "[[\"62\",7],[\"61\",1]]",
    );
    parity("=!get(!put(!map(),\"x\",42),\"x\")", "42");
}
#[test]
fn map_byte_values_are_nested_owned_sequences() {
    parity(
        ":t=!put(!put(!bytemap(),\"k\",\"a\"),\"k\",\"longer\")",
        "[[\"6b\",\"6c6f6e676572\"]]",
    );
    parity(":s=!get(!put(!bytemap(),\"k\",\"abc\"),\"k\")", "abc");
}
#[test]
fn map_keys_compare_all_bytes_including_non_utf8_and_nul() {
    parity(
        "=!get(!insert(!insert(!map(),\"\\xff\\0\",8),\"\\xff\",9),\"\\xff\\0\")",
        "8",
    );
    parity("=!get(!insert(!map(),\"\",3),\"\")", "3");
}
#[test]
fn map_growth_and_hash_collisions_preserve_insertion_iteration() {
    parity(
        "=!get(@(!map(),0;b<200;!insert(a,!format(b),b*3),b+1;a),\"199\")",
        "597",
    );
    parity(
        ":s=!key(@(!map(),0;b<200;!put(a,!format(b),b),b+1;a),139)",
        "139",
    );
}
#[test]
fn replaced_map_read_after_update_keeps_old_value() {
    parity(
        "=b(!put(!map(),\"k\",3))\n(m)=!get(!put(a,\"k\",9),\"k\")+!get(a,\"k\")",
        "12",
    );
}
#[test]
fn caller_alias_survives_callee_map_update() {
    parity(
        "=b(!insert(!map(),\"k\",3))\n(m)=!get(c(a),\"k\")+!get(a,\"k\")\n(m):m=!put(a,\"k\",9)",
        "12",
    );
}
#[test]
fn byte_map_alias_survives_equal_and_different_length_updates() {
    parity(
        ":s=b(!insert(!bytemap(),\"k\",\"old\"))\n(t):s=!concat(!get(!put(a,\"k\",\"new\"),\"k\"),!get(a,\"k\"))",
        "newold",
    );
    parity(
        ":s=b(!insert(!bytemap(),\"k\",\"old\"))\n(t):s=!concat(!get(!put(a,\"k\",\"newer\"),\"k\"),!get(a,\"k\"))",
        "newerold",
    );
}
#[test]
fn loop_state_map_alias_and_returned_alias_remain_immutable() {
    parity(
        "=b(!put(!map(),\"k\",3))\n(m)=!get(@(a,0;b<10;!put(a,\"k\",b),b+1;a),\"k\")+!get(a,\"k\")",
        "12",
    );
    parity(
        "=b(!put(!map(),\"k\",3))\n(m)=!get(!put(c(a),\"k\",9),\"k\")+!get(a,\"k\")\n(m):m=a",
        "12",
    );
}
#[test]
fn map_lazy_arms_do_not_evaluate_missing_or_duplicate_keys() {
    parity("=false?!get(!map(),\"missing\"):42", "42");
    parity(
        "=b(!insert(!map(),\"k\",3))\n(m)=!get(true?a:!insert(a,\"k\",4),\"k\")",
        "3",
    );
}
#[test]
fn missing_duplicate_and_iteration_traps_have_distinct_codes() {
    failure("=!get(!map(),\"x\")", "E019");
    failure("=!size(!insert(!insert(!map(),\"k\",3),\"k\",4))", "E020");
    failure(":s=!key(!map(),0)", "E012");
    failure(":s=!key(!insert(!map(),\"k\",3),-1)", "E012");
}
#[test]
fn map_argument_traps_precede_missing_and_duplicate_checks() {
    failure("=!get(!map(),!format(1/0))", "E009");
    failure(
        "=!size(!insert(!insert(!map(),\"k\",3),\"k\",!parse(\"bad\")))",
        "E016",
    );
}
#[test]
fn map_capacity_and_copied_byte_results_respect_quota() {
    failure("=!size(!insert(!map(),!bytes(33554400,0),1))", "E013");
    // Missing/duplicate checks do not allocate a map result first.
    failure("=!get(!map(),!bytes(33554400,0))", "E019");
}
#[test]
fn last_use_map_update_emits_unique_path_and_live_alias_emits_copy() {
    for (source, call) in [
        (
            "=!size(@(!map(),0;b<10;!insert(a,!format(b),b),b+1;a))",
            "call ptr @nil_map_insert_unique_int",
        ),
        (
            "=b(!insert(!map(),\"k\",3))\n(m)=!get(!put(a,\"k\",9),\"k\")+!get(a,\"k\")",
            "call ptr @nil_map_put_int",
        ),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert!(nil_llvm::emit_llvm(&p.hir).contains(call));
    }
}
#[test]
fn invalid_map_field_value_and_operand_types_fail_before_execution() {
    for source in [
        "=!get(!map(),1)",
        "=!size(!insert(!map(),\"k\",\"v\"))",
        "=!size(!put(!bytemap(),\"k\",1))",
        "=!map()[0]",
        "(m)=#a",
    ] {
        assert_eq!(
            compile_with_profile(source, SourceProfile::ExprV5)
                .unwrap_err()
                .code,
            "E007",
            "{source}"
        );
    }
    assert_eq!(
        compile_with_profile("=!size(!map(1))", SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E006"
    );
    assert!(compile_with_profile(":m=!map()", SourceProfile::ExprV4).is_err());
}
