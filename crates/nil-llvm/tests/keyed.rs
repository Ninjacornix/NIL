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
        Value::Buffer(v) => format!(
            "[{}]",
            v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
        ),
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

#[test]
fn map_owns_key_and_value_bytes_and_lookup_returns_independent_storage() {
    parity(
        ":s=b(\"key\",\"old\")\n(s,s):s=c(!insert(!bytemap(),a,b),a[0:88],b[0:78])\n(t,s,s):s=!concat(!get(a,\"key\"),!get(a,\"key\")[0:90])",
        "oldZld",
    );
}

#[test]
fn map_entry_tooling_accepts_only_empty_maps_and_roots_them() {
    let p = compile_with_profile("(m)=!size(!put(a,\"k\",9))", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        execute_values(
            &p.hir,
            FunctionId(0),
            &[Value::Map(nil_compiler::keyed::Map::empty(false))],
            Limits::default()
        )
        .unwrap(),
        Value::I64(1)
    );
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                ..Default::default()
            },
            &["{}".into()],
        )
        .unwrap();
        assert!(out.status.success());
        assert_eq!(out.stdout, b"1\n");
        let bad = nil_llvm::run_arguments(
            &p.hir,
            Options {
                optimization,
                ..Default::default()
            },
            &["{k:1}".into()],
        )
        .unwrap();
        assert!(!bad.status.success());
        assert!(bad.stderr.starts_with(b"E010"));
    }
}

#[test]
fn every_allocating_map_operation_checks_its_result_charge() {
    for source in [
        "=b(!bytes(67108600,0))\n(s)=!size(!map())+#a",
        "=b(!bytes(67108600,0))\n(s)=!size(!bytemap())+#a",
        "=!size(!put(!map(),!bytes(33554400,0),1))",
        "=b(!insert(!map(),!bytes(8388608,0),1))\n(m)=c(a,!bytes(50331648,0))\n(m,s)=#!key(a,0)+#b",
        "=b(!insert(!bytemap(),\"k\",!bytes(4194304,0)))\n(t)=c(a,!bytes(54525952,0))\n(t,s)=#!get(a,\"k\")+#b",
    ] {
        failure(source, "E013");
    }
}

#[test]
fn duplicate_key_precedes_a_result_quota_failure() {
    failure(
        "=b(!insert(!map(),!bytes(8388608,0),1))\n(m)=c(a,!bytes(41943040,0))\n(m,s)=!size(!insert(a,!bytes(8388608,0),2))+#b",
        "E020",
    );
}

#[test]
fn sort_signed_extrema_duplicates_and_empty_sequences_matches_o0_o2() {
    parity(
        ":v=!sort(!parsebuf(\"3,-9223372036854775808,3,9223372036854775807,0\",\",\"),0)",
        "[-9223372036854775808,0,3,3,9223372036854775807]",
    );
    parity(":v=!sort(!parsebuf(\"3,-2,3,0\",\",\"),1)", "[3,3,0,-2]");
    parity(":s=!sort(\"caba\",0)", "aabc");
    parity(":s=!sort(\"caba\",1)", "cbaa");
    parity(":v=!sort(!buffer(0,0),0)", "[]");
    parity(":s=!sort(\"x\",1)", "x");
}

#[test]
fn sort_map_values_use_key_ties_and_rebuild_lookup() {
    parity(
        ":m=!sort(!insert(!insert(!insert(!map(),\"b\",1),\"a\",1),\"z\",-1),1)",
        "[[\"7a\",-1],[\"61\",1],[\"62\",1]]",
    );
    parity(
        "=!get(!sort(!insert(!insert(!map(),\"b\",7),\"a\",8),0),\"b\")",
        "7",
    );
    parity(
        ":t=!sort(!insert(!insert(!bytemap(),\"b\",\"x\"),\"a\",\"x\"),1)",
        "[[\"61\",\"78\"],[\"62\",\"78\"]]",
    );
    parity(
        ":m=!insert(!sort(!insert(!insert(!map(),\"b\",2),\"a\",1),0),\"c\",3)",
        "[[\"61\",1],[\"62\",2],[\"63\",3]]",
    );
}

#[test]
fn sort_byte_keys_and_values_compare_unsigned_prefixes() {
    parity(
        ":m=!sort(!insert(!insert(!insert(!map(),\"\\xff\",1),\"\\0x\",2),\"\\0\",3),0)",
        "[[\"00\",3],[\"0078\",2],[\"ff\",1]]",
    );
    parity(
        ":t=!sort(!insert(!insert(!insert(!bytemap(),\"b\",\"\\xff\"),\"c\",\"\\0x\"),\"a\",\"\\0\"),1)",
        "[[\"61\",\"00\"],[\"63\",\"0078\"],[\"62\",\"ff\"]]",
    );
}

#[test]
fn sort_retained_aliases_and_callers_keep_original_order() {
    parity(
        "=b(!parsebuf(\"3,1\",\",\"))\n(v)=c(a)[0]+a[0]\n(v):v=!sort(a,0)",
        "4",
    );
    parity(
        ":s=b(!insert(!insert(!map(),\"b\",1),\"a\",1))\n(m):s=!concat(!key(!sort(a,0),0),!key(a,0))",
        "ab",
    );
}

#[test]
fn each_empty_single_and_fixed_inputs_have_typed_finish_bindings() {
    parity("=!each(!buffer(0,0),42;1/0;a)", "42");
    parity("=!each([4,5,6],0;c+b;a)", "15");
    parity("=!each(\"x\",0;c+a+b;a)", "120");
    parity("=!each(!map(),7;c+b;a)", "7");
}

#[test]
fn each_map_sorted_order_and_owned_byte_values_match_o0_o2() {
    parity(
        ":s=!each(!sort(!insert(!insert(!map(),\"b\",1),\"a\",1),1),\"\";!concat(c,a);a)",
        "ab",
    );
    parity(
        ":s=!each(!insert(!insert(!bytemap(),\"b\",\"2\"),\"a\",\"1\"),\"\";!concat(c,!concat(a,b));a)",
        "b2a1",
    );
}

#[test]
fn each_snapshot_survives_updates_and_parallel_state_uses_old_values() {
    parity(
        "=b(!parsebuf(\"1,2,3\",\",\"))\n(v)=!each(a,a,0;c[a:9],d+b;b)",
        "6",
    );
    parity("=!each([1,2],1,2;d,c;a*10+b)", "12");
    parity(
        ":s=b(!insert(!bytemap(),\"a\",\"old\"))\n(t):s=!each(a,a,\"\";!put(c,a,\"new\"),!concat(d,b);b)",
        "old",
    );
}

#[test]
fn each_nested_regions_and_following_expressions_remap_values_correctly() {
    parity("=!each([1,2],0;c+!each([3,4],0;c+b;a);a)+5", "19");
    parity("=!each([1,2],0;b==1?c+10:c+20;a)+!each([3],0;c+b;a)", "33");
    parity("=!each([1],0;c+@(0;a<3;a+1;a);a)", "3");
}

#[test]
fn each_lazy_arms_keep_unselected_traps_unobservable() {
    parity("=false?!each([1],0;1/0;a):7", "7");
    parity("=!each([1],0;true?c+b:1/0;a)", "1");
}

#[test]
fn sort_invalid_order_precedes_quota_and_aliases_do_not_bypass_reservation() {
    failure(
        "=b(!insert(!map(),!bytes(8388608,0),1))\n(m)=c(a,!bytes(50331648,0))\n(m,s)=!size(!sort(a,0))+#b",
        "E013",
    );
    failure("=#!sort(!bytes(40000000,0),2)", "E012");
    failure("=#!sort(!bytes(40000000,0),0)", "E013");
    failure("=!size(!sort(!map(),-1))", "E012");
    failure("=#!sort(!buffer(0,0),2)", "E012");
    failure(
        "=!size(!sort(!insert(!insert(!map(),\"x\",1),\"x\",2),2))",
        "E020",
    );
}

#[test]
fn sort_ir_uses_unique_storage_only_when_last_use_is_proven() {
    let unique = compile_with_profile(":v=!sort(!buffer(5,2),0)", SourceProfile::ExprV5).unwrap();
    let ir = nil_llvm::emit_llvm(&unique.hir);
    assert!(ir.contains("call ptr @nil_sort_unique("));
    let alias = compile_with_profile(
        "=b(!buffer(5,2))\n(v)=!sort(a,0)[0]+a[0]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&alias.hir);
    assert!(ir.contains("call ptr @nil_sort("));
}

#[test]
fn each_byte_values_reserve_owned_materialization_even_when_ignored() {
    failure(
        "=b(!insert(!bytemap(),\"k\",!bytes(16777216,0)))\n(t)=!each(a,!bytes(16777000,0);c;#a)",
        "E013",
    );
}
#[test]
fn each_append_state_keeps_unique_proof_while_snapshot_aliases_copy() {
    parity(
        ":s=!each(\"abcdef\",\"\";!concat(c,!bytes(1,b));a)",
        "abcdef",
    );
    let p = compile_with_profile(
        ":s=!each(\"abc\",\"\";!concat(c,!bytes(1,b));a)",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(nil_llvm::emit_llvm(&p.hir).contains("call ptr @nil_concat_unique("));
    parity(
        "=b(!insert(!map(),\"z\",3))\n(m)=!each(a,a,0;!put(c,\"new\",9),d+b;!size(a)*10+b)",
        "23",
    );
}
