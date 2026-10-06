use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
use nil_llvm::{Instrumentation, Optimization, Options};
#[derive(Default)]
struct MemoryHost {
    stdout: Vec<u8>,
}
impl Host for MemoryHost {
    fn out(&mut self, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.stdout.extend_from_slice(bytes);
        Ok(())
    }
}
fn parity(source: &str, expected: &[u8], code: Option<&str>, depth: usize) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    check(&p.hir, expected, code, depth);
}
fn check(p: &nil_hir::ValidatedProgram, expected: &[u8], code: Option<&str>, depth: usize) {
    let mut host = MemoryHost::default();
    let result = execute_values_with_host(
        p,
        FunctionId(0),
        &[],
        Limits {
            steps: 1000000,
            call_depth: depth,
        },
        &mut host,
    );
    if let Some(code) = code {
        assert_eq!(result.unwrap_err().code, code);
        assert_eq!(host.stdout, expected);
    } else {
        let tail = match result.unwrap() {
            Value::I64(v) => format!("{v}\n").into_bytes(),
            Value::Bytes(v) => [v.as_ref(), b"\n"].concat(),
            v => panic!("unexpected test result {v:?}"),
        };
        assert_eq!([host.stdout, tail].concat(), expected);
    }
    for optimization in [Optimization::O0, Optimization::O2] {
        let actual = nil_llvm::run_arguments(
            p,
            Options {
                optimization,
                instrumentation: Instrumentation::Bounded,
                steps: 1000000,
                call_depth: depth as u64,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(actual.stdout, expected, "{optimization:?}");
        if let Some(code) = code {
            assert!(!actual.status.success());
            assert!(
                actual.stderr.starts_with(code.as_bytes()),
                "{:?}",
                actual.stderr
            );
        } else {
            assert!(actual.status.success(), "{:?}", actual.stderr);
            assert!(actual.stderr.is_empty());
        }
    }
}
#[test]
fn std_bytes_map_filter_fold_match_reference_at_o0_and_o2() {
    parity(":s=!map(&b,\"abc\")\n1=a+1", b"bcd\n", None, 256);
    parity(":s=!filter(&b,\"abca\")\n1:b=a==97", b"aa\n", None, 256);
    parity("=!fold(&b,\"abc\",0)\n2=a+b", b"294\n", None, 256);
}
#[test]
fn std_buffer_overloads_empty_sequences_and_aliases_match() {
    parity(
        "=!fold(&b,!map(&c,!buffer(3,2)),0)\n2=a+b\n1=a*3",
        b"18\n",
        None,
        256,
    );
    parity(":s=!filter(&b,\"\")\n1:b=1/0==0", b"\n", None, 256);
    parity("=!fold(&b,!buffer(0,0),7)\n2=a/0", b"7\n", None, 256);
    parity(
        "=b(\"ab\")\n(s)=!map(&c,a)[0]+a[0]\n1=a+1",
        b"195\n",
        None,
        256,
    );
}
#[test]
fn std_callback_effect_order_and_unselected_arms_stay_lazy() {
    parity(
        "=!fold(&b,\"ABC\",0)\n2=a+!out(!bytes(1,b))",
        b"ABC3\n",
        None,
        256,
    );
    parity(
        ":s=false?!map(&b,\"x\"):\"safe\"\n1=!out(\"BAD\")+a/0",
        b"safe\n",
        None,
        256,
    );
    parity(
        ":s=!filter(&b,\"abc\")\n1:b=!out(!bytes(1,a))>=0",
        b"abcabc\n",
        None,
        256,
    );
}
#[test]
fn std_callback_allocation_quota_and_byte_range_keep_failure_order() {
    parity(":s=!map(&b,\"a\")\n1=#!bytes(3,a)", b"\x03\n", None, 256);
    parity(
        ":s=!map(&b,\"a\")\n1=#!bytes(67108865,a)",
        b"",
        Some("E013"),
        256,
    );
    parity(":s=!map(&b,\"a\")\n1=256", b"", Some("E014"), 256);
    parity(
        "=!fold(&b,\"a\",0)\n2=a+!parse(\"invalid\")",
        b"",
        Some("E016"),
        256,
    );
}
#[test]
fn std_inside_module_uses_the_same_loader_and_caller_callback() {
    let dir = std::env::temp_dir().join(format!("nil-std-module-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("module.nil"), "([i:i],s):s=!map(^a,b)").unwrap();
    std::fs::write(
        dir.join("module.nil-module"),
        "nil-module 1\nid 7\nsource module.nil\nexport 0 0\n",
    )
    .unwrap();
    let p = nil_compiler::compile_with_plugins(
        ":s=!7.0(&b,\"ab\")\n1=a+1",
        SourceProfile::ExprV5,
        &[dir.join("module.nil-module")],
    )
    .unwrap();
    check(&p.hir, b"bc\n", None, 256);
    std::fs::remove_dir_all(dir).unwrap();
}
