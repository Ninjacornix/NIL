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
        Value::U64(v) => v.to_string(),
        Value::Bool(v) => v.to_string(),
        Value::Bytes(v) => String::from_utf8(v.to_vec()).unwrap(),
        _ => panic!("wrapper returned unexpected value"),
    };
    assert_eq!(rendered, expected, "reference {source}");
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
        assert_eq!(
            out.stdout,
            format!("{expected}\n").as_bytes(),
            "{optimization:?}: {source}"
        );
    }
}
fn failure(source: &str, code: &str) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let err = execute_values(&p.hir, FunctionId(0), &[], Limits::default()).unwrap_err();
    assert_eq!(err.code, code);
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
fn construction_and_field_reads_are_statically_typed() {
    parity(
        "record Pair(left:i,right:s)\n=Pair(42,\"answer\").left",
        "42",
    );
    parity(
        "record R(flag:b,array:2,value:u128)\n=R(true,[3,7],18446744073709551616u128).flag?R(false,[3,7],0u128).array[1]:0",
        "7",
    );
}
#[test]
fn functional_update_keeps_original_field_and_alias_live() {
    parity(
        "record R(text:s)\n=b(R(\"abc\"))\n(R)=c(a,a{text:a.text[0:120]})\n(R,R)=a.text[0]+b.text[0]",
        "217",
    );
}
#[test]
fn nested_dynamic_fields_survive_allocating_calls() {
    parity(
        "record Inner(data:s)\nrecord Outer(inner:Inner,other:v)\n=b(Outer(Inner(\"abc\"),!buffer(3,7)))\n(Outer)=c(a,!bytes(1000,42))\n(Outer,s)=a.inner.data[2]+a.other[1]+#b",
        "1106",
    );
}
#[test]
fn duplicate_dynamic_fields_count_aliases_independently() {
    parity(
        "record R(first:s,second:s)\n=b(\"abc\")\n(s)=c(R(a,a))\n(R)=d(a,a.first[0:120])\n(R,s)=a.second[0]+b[0]",
        "217",
    );
}
#[test]
fn dynamic_record_fields_cross_loop_state_and_allocating_regions() {
    parity(
        "record R(text:s,count:i)\n=b(@(R(\"a\",0);a.count<20;R(!concat(a.text,\"x\"),a.count+1);a))\n(R)=#a.text+a.count",
        "41",
    );
}
#[test]
fn lazy_record_arms_keep_unselected_traps_and_effects_unobservable() {
    parity(
        "record R(number:i)\n=(true?R(42):R(!out(\"bad\")/0)).number",
        "42",
    );
    failure(
        "record R(first:i,second:i)\n=R(!parse(\"bad\"),1/0).first",
        "E016",
    );
}
#[test]
fn nested_records_containing_maps_keep_old_map_values() {
    parity(
        "record R(map:m)\n=b(R(!put(!map(),\"k\",3)))\n(R)=c(a,a{map:!put(a.map,\"k\",9)})\n(R,R)=!get(a.map,\"k\")+!get(b.map,\"k\")",
        "12",
    );
}
#[test]
fn scalar_record_map_lookup_preserves_all_bit_patterns() {
    parity(
        "record R(flag:b,value:u128,bits:f64,array:2)\n=b(!put(!map[R](),\"k\",R(true,18446744073709551616u128,!floatbits(9223372036854775808u64),[7,9])))\n(map[R])=c(!get(a,\"k\"))\n(R)=a.flag?(a.value==18446744073709551616u128?(!bits(a.bits)==9223372036854775808u64?a.array[0]+a.array[1]:-1):-2):-3",
        "16",
    );
}
#[test]
fn scalar_record_map_values_cross_calls_updates_and_each() {
    parity(
        "record R(value:i)\n=b(!put(!put(!map[R](),\"b\",R(7)),\"a\",R(9)))\n(map[R])=!each(!sort(a,0),0;c+b.value;a)",
        "16",
    );
}
#[test]
fn scalar_record_map_sort_rejects_value_order_before_quota() {
    failure("record R(value:i)\n=!size(!sort(!map[R](),1))", "E018");
    failure("record R(value:i)\n=!size(!sort(!map[R](),2))", "E012");
}
#[test]
fn scalar_record_map_missing_and_duplicate_failures_preserve_codes() {
    failure(
        "record R(value:i)\n=!get(!map[R](),\"missing\").value",
        "E019",
    );
    failure(
        "record R(value:i)\n=!size(!insert(!insert(!map[R](),\"k\",R(1)),\"k\",R(2)))",
        "E020",
    );
}
#[test]
fn record_aliases_keep_nested_capacity_charged() {
    failure(
        "record R(data:s)\n=b(R(!bytes(40000000,0)))\n(R)=c(a,!bytes(30000000,1))\n(R,s)=#a.data+#b",
        "E013",
    );
}
#[test]
fn dead_record_fields_release_capacity_before_next_allocation() {
    parity(
        "record R(data:s)\n=b(R(!bytes(40000000,0)))\n(R)=#a.data+#(!bytes(30000000,0))",
        "70000000",
    );
}
#[test]
fn record_projection_is_visible_in_hir_and_llvm_without_runtime_opcode() {
    let p = compile_with_profile(
        "record R(value:i)\n=R(42){value:7}.value",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("%nil.record0 = type { i64 }"));
    assert!(ir.contains("insertvalue %nil.record0"));
    assert!(ir.contains("extractvalue %nil.record0"));
    assert!(!ir.contains("call void @nil_root_store"));
}
#[test]
fn record_native_entry_requires_explicit_wrapper() {
    let p = compile_with_profile("record R(value:i)\n:R=R(42)", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        nil_llvm::entry_runtime(&p.hir, Options::default())
            .unwrap_err()
            .code,
        "E010"
    );
}

#[test]
fn record_field_effects_and_called_lazy_arms_preserve_order() {
    use nil_compiler::{application::Host, evaluator::execute_values_with_host};
    #[derive(Default)]
    struct Capture(Vec<u8>);
    impl Host for Capture {
        fn out(&mut self, bytes: &[u8]) -> Result<(), nil_hir::Diagnostic> {
            self.0.extend_from_slice(bytes);
            Ok(())
        }
    }
    let source = "record R(left:i,right:i)\n=!out(\"A\")+b(true?R(!out(\"B\"),!out(\"C\")):R(!out(\"BAD\"),1/0))+!out(\"E\")\n(R)=!out(\"D\")+a.left+a.right";
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let mut host = Capture::default();
    let value =
        execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host).unwrap();
    assert_eq!(value, Value::I64(5));
    assert_eq!(host.0, b"ABCDE");
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
        assert!(out.status.success());
        assert_eq!(out.stdout, b"ABCDE5\n");
    }
}
#[test]
fn nested_scalar_record_maps_and_zero_slot_fields_round_trip() {
    parity(
        "record Empty(values:0)\nrecord Inner(value:i,empty:Empty)\nrecord Outer(inner:Inner)\n=!get(!put(!map[Outer](),\"k\",Outer(Inner(42,Empty([])))),\"k\").inner.value",
        "42",
    );
    parity(
        "record Empty(values:0)\n=#!get(!put(!map[Empty](),\"k\",Empty([])),\"k\").values",
        "0",
    );
}
