use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId, Type};
use std::collections::BTreeMap;

fn compile(source: &str) -> nil_compiler::CompiledProgram {
    compile_with_profile(source, SourceProfile::ExprV5).unwrap()
}
fn execute(source: &str, args: &[Value]) -> Result<Value, Diagnostic> {
    execute_values(&compile(source).hir, FunctionId(0), args, Limits::default())
}

#[test]
fn runtime_buffers_exceed_fixed_array_capacity_and_keep_aliases() {
    assert_eq!(
        execute("1:v=!buffer(a,7)", &[Value::I64(1024)]).unwrap(),
        Value::Buffer(vec![7; 1024].into())
    );
    assert_eq!(
        execute(
            "(v)=b(a[0:9])+a[0]\n(v)=a[0]",
            &[Value::Buffer(vec![3].into())]
        )
        .unwrap(),
        Value::I64(12)
    );
    assert_eq!(
        execute(
            include_str!("../../../examples/expr-v5/sum.nil"),
            &[Value::Buffer((1..=1024).collect::<Vec<_>>().into())]
        )
        .unwrap(),
        Value::I64(524800)
    );
}

#[test]
fn bytes_literals_utf8_escapes_slices_and_decimal_conversion_are_exact() {
    for (source, expected) in [
        (":s=\"hé\\n\\0\\\"\\\\\"", "hé\n\0\"\\".as_bytes().to_vec()),
        (":s=!slice(!concat(\"abc\",\"def\"),2,3)", b"cde".to_vec()),
        (
            ":s=!format(-9223372036854775808)",
            b"-9223372036854775808".to_vec(),
        ),
        (":s=\"abc\"[1:0]", vec![b'a', 0, b'c']),
        (":s=!bytes(0,0)", vec![]),
    ] {
        assert_eq!(execute(source, &[]).unwrap(), Value::Bytes(expected.into()));
    }
    assert_eq!(
        execute("=!parse(\"9223372036854775807\")", &[]).unwrap(),
        Value::I64(i64::MAX)
    );
    assert_eq!(execute("=#!bytes(3,0)", &[]).unwrap(), Value::I64(3));
}

#[test]
fn dynamic_regions_and_forward_calls_preserve_types_and_lazy_failures() {
    assert_eq!(
        execute("1:s=a>0?b(a):\"none\"\n1:s=!format(a)", &[Value::I64(42)]).unwrap(),
        Value::Bytes(b"42".to_vec().into())
    );
    assert_eq!(
        execute("=false?!parse(\"bad\"):7", &[]).unwrap(),
        Value::I64(7)
    );
    assert_eq!(
        execute(
            include_str!("../../../examples/expr-v5/uppercase.nil"),
            &[Value::Bytes("héllo".as_bytes().to_vec().into())]
        )
        .unwrap(),
        Value::Bytes("HéLLO".as_bytes().to_vec().into())
    );
}

#[test]
fn typecheck_and_parser_reject_invalid_application_operations() {
    for source in [
        "=!read(1)",
        "=!write(\"p\",1)",
        "=!concat(!buffer(2,0),\"x\")",
        "=!buffer(true,0)",
        "=!slice(\"x\",0)",
        "1=a?\"x\":!buffer(0,0)",
        "=!unknown()",
        ":s=\"bad\\q\"",
        ":s=\"unfinished",
    ] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV5).is_err(),
            "{source}"
        );
    }
    for profile in [
        SourceProfile::ExprV0,
        SourceProfile::ExprV1,
        SourceProfile::ExprV2,
        SourceProfile::ExprV3,
        SourceProfile::ExprV4,
    ] {
        assert!(compile_with_profile(":s=\"text\"", profile).is_err());
        assert!(compile_with_profile("=!buffer(2,0)", profile).is_err());
    }
}

#[test]
fn bounds_byte_range_parse_and_memory_failures_are_structured() {
    for (source, code) in [
        ("=!buffer(2,0)[2]", "E012"),
        ("=\"x\"[-1]", "E012"),
        (":s=!slice(\"x\",1,1)", "E012"),
        (":s=\"x\"[0:256]", "E014"),
        (":s=!bytes(1,-1)", "E014"),
        (":v=!buffer(-1,0)", "E013"),
        (":v=!buffer(9223372036854775807,0)", "E013"),
        (":v=!buffer(8388608,0)", "E013"),
        (":s=!bytes(67108864,0)", "E013"),
        ("=!parse(\"01\")", "E016"),
        ("=!parse(\"+1\")", "E016"),
        ("=!parse(\"-0\")", "E016"),
        ("=!parse(\"9223372036854775808\")", "E016"),
        (":s=!read(\"a\\0b\")", "E017"),
    ] {
        let e = execute(source, &[]).unwrap_err();
        assert_eq!(e.code, code, "{source}");
        assert!(e.span.is_some());
    }
}

#[derive(Default)]
struct MemoryHost {
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    events: Vec<&'static str>,
    output: Vec<u8>,
}
impl Host for MemoryHost {
    fn read(&mut self, path: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        self.events.push("read");
        Ok(self.files[path].clone())
    }
    fn write(&mut self, path: &[u8], data: &[u8]) -> Result<(), Diagnostic> {
        self.events.push("write");
        self.files.insert(path.to_vec(), data.to_vec());
        Ok(())
    }
    fn out(&mut self, data: &[u8]) -> Result<(), Diagnostic> {
        self.events.push("out");
        self.output.extend_from_slice(data);
        Ok(())
    }
}
#[test]
fn host_access_is_explicit_ordered_and_lazy() {
    assert_eq!(
        execute(":s=!read(\"input\")", &[]).unwrap_err().code,
        "E018"
    );
    assert_eq!(
        execute("=false?!write(\"p\",\"x\"):7", &[]).unwrap(),
        Value::I64(7)
    );
    let mut host = MemoryHost::default();
    host.files.insert(b"input".to_vec(), vec![0, 255, b'x']);
    let p = compile("=!out(!format(!write(\"output\",!read(\"input\"))))");
    assert_eq!(
        execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut host).unwrap(),
        Value::I64(1)
    );
    assert_eq!(host.events, vec!["read", "write", "out"]);
    assert_eq!(host.files[b"output".as_slice()], vec![0, 255, b'x']);
    assert_eq!(host.output, b"3");
}

#[test]
fn external_hir_cannot_forge_intrinsic_signatures_or_byte_types() {
    let mut p = compile(":v=!buffer(2,0)").hir.program().clone();
    p.functions[0].instructions.last_mut().unwrap().ty = Type::Bytes;
    assert_eq!(nil_hir::validate(p).unwrap_err().code, "E007");
    let mut p = compile("=#!buffer(2,0)").hir.program().clone();
    if let nil_hir::Operation::Intrinsic { arguments, .. } =
        &mut p.functions[0].instructions[2].operation
    {
        arguments.push(nil_hir::ValueId(0));
    }
    assert_eq!(nil_hir::validate(p).unwrap_err().code, "E006");
}

#[test]
fn application_parentheses_and_intrinsics_enforce_depth_before_stack_exhaustion() {
    for source in [
        format!("1={}a{}", "(".repeat(129), ")".repeat(129)),
        format!(":s={}0{}", "!format(".repeat(129), ")".repeat(129)),
    ] {
        let error = compile_with_profile(&source, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(error.code, "E001");
        assert!(error.message.contains("nesting limit"));
    }
}

#[test]
fn dead_allocations_are_reclaimed_instead_of_charged_cumulatively() {
    let source = "=@(0;a<8192;a+1+#!bytes(8192,0)*0;a)";
    let p = compile(source);
    let result = execute_values(
        &p.hir,
        FunctionId(0),
        &[],
        Limits {
            steps: 1_000_000,
            ..Default::default()
        },
    );
    assert_eq!(result.unwrap(), Value::I64(8192));
}

#[test]
fn hexadecimal_literals_preserve_non_utf8_and_reject_malformed_escapes() {
    assert_eq!(
        execute(r#":s="\xFF\x00\x80\x7f""#, &[]).unwrap(),
        Value::Bytes(vec![255, 0, 128, 127].into())
    );
    for source in [r#":s="\x""#, r#":s="\x1""#, r#":s="\xGG""#, r#":s="\x💥0""#] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV5).is_err(),
            "{source}"
        );
    }
    // Two digits only: the trailing character remains a literal byte.
    assert_eq!(
        execute(r#":s="\x414""#, &[]).unwrap(),
        Value::Bytes(b"A4".to_vec().into())
    );
}

#[test]
fn byte_construction_checks_value_before_quota_but_after_negative_length() {
    assert_eq!(
        execute(":s=!bytes(67108864,256)", &[]).unwrap_err().code,
        "E014"
    );
    assert_eq!(execute(":s=!bytes(-1,256)", &[]).unwrap_err().code, "E013");
}

#[test]
fn simultaneously_live_dynamic_allocations_still_obey_the_quota() {
    let source = "=b(!buffer(3000000,1),!buffer(3000000,2),!buffer(3000000,3))\n(v,v,v)=#a+#b+#c";
    assert_eq!(execute(source, &[]).unwrap_err().code, "E013");
}
#[test]
fn byte_transform_reuses_dead_state_but_preserves_live_aliases() {
    let source = "1:s=@(!bytes(a,0),0;b<#a;a[b:255],b+1;a)";
    assert_eq!(
        execute(source, &[Value::I64(4096)]).unwrap(),
        Value::Bytes(vec![255; 4096].into())
    );
    let source = "(s):s=@(a,0,a;b<#a;a[b:255],b+1,c;!concat(a,c))";
    assert_eq!(
        execute(source, &[Value::Bytes(vec![1, 2, 3].into())]).unwrap(),
        Value::Bytes(vec![255, 255, 255, 1, 2, 3].into())
    );
}

#[test]
fn aliased_entry_arguments_charge_one_distinct_live_allocation() {
    let bytes = Value::Bytes(vec![0; 34 * 1024 * 1024].into());
    let result = execute("(s,s)=#a+#b", &[bytes.clone(), bytes]).unwrap();
    assert_eq!(result, Value::I64(2 * 34 * 1024 * 1024));
}

#[test]
fn append_capacity_is_charged_even_when_logical_payloads_would_fit() {
    let source = "=#!concat(!concat(!bytes(10000000,0),!bytes(1,0)),!bytes(19000000,0))";
    assert_eq!(execute(source, &[]).unwrap_err().code, "E013");
}
#[test]
fn append_grows_without_mutating_external_argument_aliases() {
    let source = "(s):s=@(a,0;b<257;!concat(a,\"\\xFF\"),b+1;a)";
    let arg = Value::Bytes(b"original".to_vec().into());
    let expected = [b"original".as_slice(), &vec![255; 257]].concat();
    assert_eq!(
        execute(source, std::slice::from_ref(&arg)).unwrap(),
        Value::Bytes(expected.into())
    );
    assert_eq!(arg, Value::Bytes(b"original".to_vec().into()));
}

#[test]
fn literal_admission_ignores_hir_vec_spare_capacity() {
    let mut program = compile(":s=\"x\"").hir.program().clone();
    let nil_hir::Operation::Bytes(bytes) = &mut program.functions[0].instructions[0].operation
    else {
        panic!("expected byte literal");
    };
    let mut spare = Vec::with_capacity(nil_hir::MAX_DYNAMIC_BYTES);
    spare.extend_from_slice(bytes);
    *bytes = spare;
    assert_eq!(
        execute_values(
            &nil_hir::validate(program).unwrap(),
            FunctionId(0),
            &[],
            Limits::default()
        )
        .unwrap(),
        Value::Bytes(vec![b'x'].into())
    );
}

#[test]
fn equality_and_search_handle_binary_bytes_buffers_and_end_sentinel() {
    for (source, answer) in [
        (":b=!equal(\"\\xFF\\0\",\"\\xFF\\0\")", Value::Bool(true)),
        (":b=!equal(\"x\",\"xx\")", Value::Bool(false)),
        (":b=!equal(\"ab\",\"ac\")", Value::Bool(false)),
        (":b=!equal(!buffer(0,9),!buffer(0,1))", Value::Bool(true)),
        (":b=!equal(!buffer(2,-1),!buffer(2,1))", Value::Bool(false)),
        ("=!find(\"x\\0\\xFF\",255,0)", Value::I64(2)),
        ("=!find(\"aba\",97,1)", Value::I64(2)),
        ("=!find(\"aba\",97,3)", Value::I64(3)),
        ("=!find(!buffer(2,-7),-7,1)", Value::I64(1)),
        ("=!find(!buffer(2,-7),9,0)", Value::I64(2)),
        ("=!find(\"\",0,0)", Value::I64(0)),
    ] {
        assert_eq!(execute(source, &[]).unwrap(), answer, "{source}");
    }
}

#[test]
fn bulk_decimal_parsing_supports_separator_sets_empty_inputs_and_i64_extrema() {
    for (source, values) in [
        (":v=!parsebuf(\"\",\"\\n\")", vec![]),
        (":v=!parsebuf(\"1,-2\\n3\\n\",\",\\n\")", vec![1, -2, 3]),
        (":v=!parsebuf(\"1\\xFF2\",\"\\xFF\")", vec![1, 2]),
        (":v=!parsebuf(\"42\",\"\")", vec![42]),
        (
            ":v=!parsebuf(\"-9223372036854775808,9223372036854775807\",\",\")",
            vec![i64::MIN, i64::MAX],
        ),
    ] {
        assert_eq!(
            execute(source, &[]).unwrap(),
            Value::Buffer(values.into()),
            "{source}"
        );
    }
}

#[test]
fn new_intrinsics_reject_wrong_types_arity_and_earlier_profiles() {
    for source in [
        ":b=!equal(1,2)",
        ":b=!equal(\"x\",!buffer(1,0))",
        "=!find(\"x\",true,0)",
        "=!find(\"x\",0)",
        ":v=!parsebuf(\"1\",10)",
        ":v=!parsebuf(\"1\")",
        ":s=!parsebuf(\"1\",\"\")",
    ] {
        assert!(
            compile_with_profile(source, SourceProfile::ExprV5).is_err(),
            "{source}"
        );
    }
    for profile in [
        SourceProfile::ExprV0,
        SourceProfile::ExprV3,
        SourceProfile::ExprV4,
    ] {
        assert!(compile_with_profile("=!find(\"x\",120,0)", profile).is_err());
    }
}

#[test]
fn new_intrinsic_failures_preserve_bounds_byte_and_quota_priority() {
    for (source, code) in [
        ("=!find(\"x\",256,-1)", "E012"),
        ("=!find(\"\",256,0)", "E014"),
        ("=!find(\"x\",0,2)", "E012"),
        (":v=!parsebuf(\"1,,2\",\",\")", "E016"),
        (":v=!parsebuf(\",1\",\",\")", "E016"),
        (":v=!parsebuf(\"1,,\",\",\")", "E016"),
        (":v=!parsebuf(\"01\",\",\")", "E016"),
        (":v=!parsebuf(\"-0\",\",\")", "E016"),
        (":v=!parsebuf(\"9223372036854775808\",\",\")", "E016"),
        (":v=!parsebuf(!bytes(8388608,10),\"\\n\")", "E013"),
        (
            "=b(!bytes(33554432,0))\n(s)=#!parsebuf(!bytes(4194304,10),\"\\n\")+#a",
            "E013",
        ),
    ] {
        assert_eq!(execute(source, &[]).unwrap_err().code, code, "{source}");
    }
}

#[test]
fn parsing_results_compose_with_concat_and_keep_caller_aliases_immutable() {
    assert_eq!(
        execute(
            "=b(!parsebuf(\"1,2\",\",\"))\n(v)=#!concat(a,!parsebuf(\"3\",\",\"))+a[1]",
            &[]
        )
        .unwrap(),
        Value::I64(5)
    );
    assert_eq!(
        execute("=false?#!parsebuf(\"bad\",\",\"):!find(\"x\",120,0)", &[]).unwrap(),
        Value::I64(0)
    );
}
