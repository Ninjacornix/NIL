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
fn allocation_accounting_is_cumulative_even_when_values_are_discarded() {
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
    assert_eq!(result.unwrap_err().code, "E013");
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
