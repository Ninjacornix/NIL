use nil_compiler::{
    SourceProfile, compile_with_profile,
    evaluator::{Limits, Value, execute_values},
};
fn result(s: &str) -> Value {
    let p = compile_with_profile(s, SourceProfile::ExprV5).unwrap();
    execute_values(&p.hir, nil_hir::FunctionId(0), &[], Limits::default()).unwrap()
}
#[test]
fn std_bytes_map_filter_fold_are_auto_loaded_and_ordered() {
    assert_eq!(
        result(":s=!map(&b,\"abc\")\n1=a+1"),
        Value::Bytes(b"bcd".to_vec().into())
    );
    assert_eq!(
        result(":s=!filter(&b,\"abca\")\n1:b=a==97"),
        Value::Bytes(b"aa".to_vec().into())
    );
    assert_eq!(result("=!fold(&b,\"abc\",0)\n2=a+b"), Value::I64(294));
}
#[test]
fn std_buffer_overloads_and_empty_inputs_preserve_callback_laziness() {
    assert_eq!(
        result("=!fold(&b,!map(&c,!buffer(3,2)),0)\n2=a+b\n1=a*3"),
        Value::I64(18)
    );
    assert_eq!(
        result(":s=!map(&b,\"\")\n1=a/0"),
        Value::Bytes(vec![].into())
    );
    assert_eq!(result("=!fold(&b,!buffer(0,0),7)\n2=a/0"), Value::I64(7));
    assert_eq!(
        result("=#!filter(&b,!buffer(3,2))\n1:b=false"),
        Value::I64(0)
    );
}
#[test]
fn std_arity_signature_and_sequence_errors_keep_structured_spans() {
    for (s, code) in [
        ("=!filter(&b)", "E006"),
        ("=!map(&b,\"a\")\n1:b=true", "E007"),
        ("=!fold(&b,1,0)\n2=a+b", "E007"),
    ] {
        let e = compile_with_profile(s, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(e.code, code, "{e}");
        assert!(e.span.is_some());
    }
}
#[test]
fn std_preserves_map_constructor_and_rejects_earlier_profiles() {
    assert_eq!(result("=!size(!map())"), Value::I64(0));
    for p in [
        SourceProfile::ExprV0,
        SourceProfile::ExprV1,
        SourceProfile::ExprV2,
        SourceProfile::ExprV3,
        SourceProfile::ExprV4,
    ] {
        assert!(compile_with_profile("=!map(&b,\"a\")\n1=a", p).is_err());
    }
}
#[test]
fn nested_std_source_depth_is_rejected_without_stack_exhaustion() {
    let s = format!("={}\"a\"{}\n1=a", "!map(&b,".repeat(512), ")".repeat(512));
    assert_eq!(
        compile_with_profile(&s, SourceProfile::ExprV5)
            .unwrap_err()
            .code,
        "E001"
    );
}
#[test]
fn std_callback_lexical_binding_survives_each_and_conditional_regions() {
    assert_eq!(
        result("=!each([1,2],0;c+!fold(&b,!buffer(2,b),0);a)\n2=a+b"),
        Value::I64(6)
    );
    assert_eq!(
        result("=!fold(&b,false?!map(&c,\"x\"):\"ab\",0)\n2=a+b\n1=a/0"),
        Value::I64(195)
    );
}

#[test]
fn numeric_qualified_names_share_visibility_collision_and_callback_resolution() {
    let dir = std::env::temp_dir().join(format!("nil-std-aliases-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("source.nil"), "1=a+1").unwrap();
    for id in [7, 8] {
        std::fs::write(
            dir.join(format!("{id}.nil-module")),
            format!("nil-module 1\nid {id}\nsource source.nil\nexport 0 0\n"),
        )
        .unwrap();
    }
    let manifests = [dir.join("7.nil-module"), dir.join("8.nil-module")];
    for (source, expected) in [
        ("=!7.0(4)+!8.0(5)", 11),
        ("=b(&!7.0,3)\n([i:i],i)=^a(b)", 4),
    ] {
        let p =
            nil_compiler::compile_with_plugins(source, SourceProfile::ExprV5, &manifests).unwrap();
        assert_eq!(
            execute_values(&p.hir, nil_hir::FunctionId(0), &[], Limits::default()).unwrap(),
            Value::I64(expected)
        );
    }
    for source in ["=!9.0(1)", "=!7.9(1)"] {
        assert_eq!(
            nil_compiler::compile_with_plugins(source, SourceProfile::ExprV5, &manifests)
                .unwrap_err()
                .code,
            "E024"
        );
    }
    assert_eq!(
        nil_compiler::compile_with_plugins(
            "=!7.0(1)",
            SourceProfile::ExprV5,
            &[manifests[0].clone(), manifests[0].clone()]
        )
        .unwrap_err()
        .code,
        "E024"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn numeric_namespace_syntax_requires_canonical_ids_with_spans() {
    for source in ["=!07.0(1)", "=!7.00(1)", "=!7.(1)"] {
        let e = compile_with_profile(source, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(e.code, "E001");
        assert!(e.span.is_some());
    }
}

#[test]
fn std_overload_discovery_rejects_invalid_embedding_record_ids_without_panicking() {
    let mut module = nil_compiler::parser::parse_with_profile(
        "record R(text:s)\n(R):s=!map(&b,a.text)\n1=a+1",
        SourceProfile::ExprV5,
    )
    .unwrap();
    module.functions[0].parameters[0] =
        nil_compiler::syntax::Parameter::Value(nil_hir::Type::Record(99, 1));
    let error = nil_compiler::lower(module).unwrap_err();
    assert_eq!(error.code, "E023");
    assert!(error.span.is_some());
}
