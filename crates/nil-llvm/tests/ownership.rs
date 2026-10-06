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
    let diagnostic = result.as_ref().err().cloned();
    if let Some(code) = code {
        assert_eq!(result.unwrap_err().code, code);
        assert_eq!(host.stdout, expected);
    } else {
        let tail = match result.unwrap() {
            Value::I64(v) => format!("{v}\n").into_bytes(),
            Value::Bytes(v) => [v.as_ref(), b"\n"].concat(),
            Value::Buffer(v) => format!(
                "[{}]\n",
                v.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
            )
            .into_bytes(),
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
            if let Some(span) = diagnostic.as_ref().and_then(|d| d.span) {
                let prefix = format!("{code} @{}..{} ", span.start, span.end);
                assert!(
                    actual.stderr.starts_with(prefix.as_bytes()),
                    "{:?}",
                    actual.stderr
                );
            }
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
fn callee_final_use_updates_preserve_caller_aliases() {
    parity(":s=b(\"abc\")\n(s):s=a[1:90]", b"aZc\n", None, 256);
    parity(
        "=b(\"abc\")\n(s)=#c(a)+a[1]\n(s):s=a[1:90]",
        b"101\n",
        None,
        256,
    );
}
#[test]
fn scalar_swap_snapshot_preserves_original_values_and_checks() {
    parity(
        ":s=b(\"abcd\",2)\n(s,i):s=a[b-1:a[b]][b:a[b-1]]",
        b"acbd\n",
        None,
        256,
    );
    parity(
        "=b(\"abcd\",2)\n(s,i)=c(a,b)[1]+a[1]\n(s,i):s=a[b-1:a[b]][b:a[b-1]]",
        b"197\n",
        None,
        256,
    );
    parity(
        ":s=b(\"abcd\",0)\n(s,i):s=a[b-1:a[b]][b:a[b-1]]",
        b"",
        Some("E012"),
        256,
    );
    parity(
        ":s=b(\"abcd\",4)\n(s,i):s=a[b-1:a[b]][b:a[b-1]]",
        b"",
        Some("E012"),
        256,
    );
}
#[test]
fn swap_snapshot_is_emitted_only_across_nontrapping_scalar_work() {
    let p = compile_with_profile(
        ":s=b(\"abcd\",2)\n(s,i):s=a[b-1:a[b]][b:a[b-1]]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_set_unique_capture"));
    let p = compile_with_profile(
        ":s=b(\"ab\")\n(s):s=a[0:90][1:a[0]+a[1]]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(ir.contains("call ptr @nil_set("));
    assert!(!ir.contains("call ptr @nil_set_unique_capture"));
}

#[test]
fn singleton_builders_preserve_values_aliases_and_byte_failures() {
    parity(
        ":s=@(\"\",0;b<31;!concat(a,!bytes(1,97)),b+1;a)",
        &[vec![97; 31], vec![10]].concat(),
        None,
        256,
    );
    parity(
        "=b(!buffer(0,0))\n(v)=#!concat(a,!buffer(1,7))",
        b"1\n",
        None,
        256,
    );
    parity(
        ":s=b(\"ab\")\n(s):s=!concat(!concat(a,!bytes(1,90)),a)",
        b"abZab\n",
        None,
        256,
    );
    parity(":s=!concat(\"ab\",!bytes(1,256))", b"", Some("E014"), 256);
}
#[test]
fn singleton_builder_keeps_quota_reservation_and_diagnostic_span() {
    parity(
        "=#!concat(!bytes(33554400,0),!bytes(1,0))",
        b"",
        Some("E013"),
        256,
    );
    parity(
        "=#!concat(!bytes(67108824,0),!bytes(1,256))",
        b"",
        Some("E014"),
        256,
    );
}
#[test]
fn update_transfer_through_recursion_preserves_live_aliases() {
    parity(
        ":s=b(\"abc\",3)\n(s,i):s=b==0?a:b(a[0:65+b],b-1)",
        b"Bbc\n",
        None,
        256,
    );
    parity(
        "=b(\"abc\",3)\n(s,i)=#c(a,b)+a[0]\n(s,i):s=b==0?a:c(a[0:65+b],b-1)",
        b"100\n",
        None,
        256,
    );
}
#[test]
fn update_effects_and_unselected_failures_remain_ordered() {
    parity(
        ":s=false?b(\"ab\"): \"safe\"\n(s):s=a[!out(\"BAD\"):90]",
        b"safe\n",
        None,
        256,
    );
    parity(
        ":s=b(\"ab\")\n(s):s=a[0:90][!out(\"middle\")-6:a[0]]",
        b"middleab\n",
        None,
        256,
    );
    let p = compile_with_profile(
        ":s=b(\"ab\")\n(s):s=a[0:90][!out(\"middle\")-6:a[0]]",
        SourceProfile::ExprV5,
    )
    .unwrap();
    assert!(!nil_llvm::emit_llvm(&p.hir).contains("call ptr @nil_set_unique_capture"));
}
#[test]
fn update_empty_and_zero_slot_record_buffers_keep_layout_parity() {
    parity(":s=b(\"\")\n(s):s=a[0:0]", b"", Some("E012"), 256);
    parity(
        "record Z(x:0)\n=#!buffer[Z](2,Z([]))[0:Z([])]",
        b"2\n",
        None,
        256,
    );
}

#[test]
fn comparator_merge_sort_module_is_stable_for_duplicate_and_empty_values() {
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std/sort.nil-module");
    for (source, expected) in [
        (":v=!7.0(&b,!buffer(0,0))\n2:b=a<b", b"[]\n".as_slice()),
        (
            ":v=!7.0(&b,!concat(!buffer(2,3),!buffer(2,1)))\n2:b=a<b",
            b"[1,1,3,3]\n".as_slice(),
        ),
        (
            ":v=!7.0(&b,!concat(!buffer(2,3),!buffer(2,1)))\n2:b=a>b",
            b"[3,3,1,1]\n".as_slice(),
        ),
    ] {
        let p = nil_compiler::compile_with_plugins(
            source,
            SourceProfile::ExprV5,
            std::slice::from_ref(&manifest),
        )
        .unwrap();
        check(&p.hir, expected, None, 256);
    }
}

#[test]
fn transfer_keeps_later_arguments_live_during_left_to_right_evaluation() {
    parity(
        ":s=b(\"ab\")\n(s):s=c(d(a),a)\n(s,s):s=!concat(a,b)\n(s):s=a[0:90]",
        b"Zbab\n",
        None,
        256,
    );
}
#[test]
fn stack_root_frames_survive_near_limit_mutual_recursion_and_depth_traps() {
    parity(
        "=b(\"abc\")\n(s)=#c(a,250)+a[0]\n(s,i):s=b==0?a:d(a[0:65],b-1)\n(s,i):s=b==0?a:c(a[0:66],b-1)",
        b"100\n",
        None,
        256,
    );
    parity(
        ":s=b(\"abc\",8)\n(s,i):s=b==0?a:b(a[0:65],b-1)",
        b"",
        Some("E008"),
        4,
    );
}
#[test]
fn comparator_merge_sort_preserves_order_of_equal_keys() {
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std/sort.nil-module");
    let p=nil_compiler::compile_with_plugins(":v=!7.0(&b,!concat(!concat(!buffer(1,21),!buffer(1,22)),!concat(!buffer(1,11),!buffer(1,12))))\n2:b=a/10<b/10",SourceProfile::ExprV5,std::slice::from_ref(&manifest)).unwrap();
    check(&p.hir, b"[11,12,21,22]\n", None, 256);
}
