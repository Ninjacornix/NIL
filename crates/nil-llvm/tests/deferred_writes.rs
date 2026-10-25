//! ADR 055 D semantic assertions independent of the shared HIR scheduler.
use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_profile,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
use nil_llvm::{Instrumentation, Optimization, Options};

#[derive(Default)]
struct Output(Vec<u8>);
impl Host for Output {
    fn out(&mut self, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.0.extend_from_slice(bytes);
        Ok(())
    }
}

fn check(
    source: &str,
    defer: bool,
    tail: Option<i64>,
    fault: Option<(&str, &str)>,
    effects: &[u8],
    copying: Option<bool>,
) {
    let program = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let mut host = Output::default();
    let reference = execute_values_with_host(
        &program.hir,
        FunctionId(0),
        &[],
        Limits {
            defer_slot_writes: defer,
            memory_bytes: nil_hir::MAX_DYNAMIC_BYTES,
            ..Default::default()
        },
        &mut host,
    );
    assert_eq!(host.0, effects);
    if let Some(tail) = tail {
        assert_eq!(reference, Ok(Value::I64(tail)));
    }
    if let Some((code, site)) = fault {
        let error = reference.as_ref().unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.span.unwrap().start, source.find(site).unwrap() + 1);
    }
    for instrumentation in [Instrumentation::Bounded, Instrumentation::Unbounded] {
        for optimization in [Optimization::O0, Optimization::O2] {
            let actual = nil_llvm::run_arguments(
                &program.hir,
                Options {
                    defer_slot_writes: defer,
                    instrumentation,
                    optimization,
                    explain_copies: copying.is_some(),
                    ..Default::default()
                },
                &[],
            )
            .unwrap();
            let stderr = String::from_utf8(actual.stderr).unwrap();
            if let Some((code, _)) = fault {
                let span = reference.as_ref().unwrap_err().span.unwrap();
                assert!(!actual.status.success());
                assert!(
                    stderr.starts_with(&format!("{code} @{}..{} ", span.start, span.end)),
                    "{stderr}"
                );
                assert_eq!(actual.stdout, effects);
            } else {
                assert!(actual.status.success(), "{stderr}");
                assert_eq!(
                    actual.stdout,
                    [effects, format!("{}\n", tail.unwrap()).as_bytes()].concat()
                );
                if let Some(copying) = copying {
                    assert_eq!(
                        stderr.contains("\"kind\":\"replace\""),
                        copying,
                        "{source}\n{instrumentation:?} {optimization:?}: {stderr}"
                    );
                }
            }
        }
    }
}

#[test]
fn sibling_reads_and_nonleaf_calls_observe_old_values_without_copying() {
    let callee = "(v,i)=@(a,b,0;c<1;a,b,c+1;a[b])\n";
    for source in [
        format!("=b(!buffer(8,0))\n(v)=@(a,0,0;b<4;a[b:b+100],b+1,c*1000+c(a,b);c)\n{callee}"),
        format!("=b(!buffer(8,0))\n(v)=@(0,a,0;c<4;a*1000+c(b,c),b[c:c+100],c+1;a)\n{callee}"),
        format!(
            "=b(!buffer(8,0))\n(v)=@(a,0,0;b<4;a[b:b+100][b:200],b+1,c*1000+c(a,b);c)\n{callee}"
        ),
        "=b(!buffer(8,0))\n(v)=@(a,0,0;b<4;a[b:100],b+1,c+a[b];c)\n".into(),
    ] {
        check(&source, true, Some(0), None, b"", Some(false));
    }
}

#[test]
fn allocation_between_staging_and_read_preserves_the_owner() {
    let source = "=b(!buffer(8,0))\n(v)=@(a,0,0;b<4;a[b:b+100],b+1,c*1000+c(a,b)+#!buffer(4096,b);c)\n(v,i)=@(a,b,0;c<1;a,b,c+1;a[b])\n";
    check(
        source,
        true,
        Some(4_100_100_100_096),
        None,
        b"",
        Some(false),
    );
}

#[test]
fn read_faults_precede_all_staged_bounds_and_byte_checks() {
    for (source, eager_code, eager_site) in [
        (
            "=b(!buffer(8,0))\n(v)=@(a,0,0;b<1;a[100:1],b+1,c+a[200];c)\n",
            "E012",
            "a[100:1]",
        ),
        (
            "=b(!buffer(8,0))\n(v)=@(a,0,0;b<1;a[100:1][0:2],b+1,c+a[200];c)\n",
            "E012",
            "a[100:1]",
        ),
        (
            "=b(\"abc\")\n(s)=@(a,0,0;b<1;a[0:256],b+1,c+a[200];c)\n",
            "E014",
            "a[0:256]",
        ),
    ] {
        check(
            source,
            false,
            None,
            Some((eager_code, eager_site)),
            b"",
            None,
        );
        check(source, true, None, Some(("E012", "a[200]")), b"", None);
    }
}

#[test]
fn write_faults_preserve_chain_and_slot_order_after_host_effects() {
    check(
        "=b(\"abc\")\n(s)=@(a,0,0;b<1;a[0:256][100:1],b+1,!out(\"read\");c)\n",
        true,
        None,
        Some(("E014", "a[0:256]")),
        b"read",
        None,
    );
    check(
        "=b(!buffer(8,0),\"abc\")\n(v,s)=@(a,b,0;c<1;a[100:1],b[0:256],c+1;0)\n",
        true,
        None,
        Some(("E012", "a[100:1]")),
        b"",
        None,
    );
}

#[test]
fn nested_updates_and_shared_chain_results_stay_eager() {
    // A lazy capture of the intermediate updated value also prevents staging.
    check(
        "=b(!buffer(8,0))\n(v)=@(a,0,0;b<1;(next=a[100:1];next[0:(true?next[0]:0)]),b+1,c+a[200];c)\n",
        true,
        None,
        Some(("E012", "a[100:1]")),
        b"",
        None,
    );
    check(
        "=#b(!buffer(8,0))\n(v):v=@(a,0,0;b<1;!slice(a[100:1],0,1),b+1,c+a[200];a)\n",
        true,
        None,
        Some(("E012", "a[100:1]")),
        b"",
        None,
    );
    // One carried input updated by two result slots: no deferral.
    check(
        "=b(!buffer(8,0))\n(v)=@(a,a,0,0;c<1;a[100:1],a[0:2],c+1,d+a[200];d)\n",
        true,
        None,
        Some(("E012", "a[100:1]")),
        b"",
        None,
    );
}

#[test]
fn independent_runtime_aliases_keep_distinct_next_values() {
    check(
        "=b(!buffer(8,0))\n(v)=@(a,a,0;c<1;a[0:10],b[0:20],c+1;a[0]*100+b[0])\n",
        true,
        Some(1020),
        None,
        b"",
        Some(true),
    );
    // An escaping old-state result is an independent owner at commit time.
    check(
        "=b(!buffer(8,0))\n(v)=@(a,a,0;c<1;a[0:10],a,c+1;a[0]*100+b[0])\n",
        true,
        Some(1000),
        None,
        b"",
        Some(true),
    );
}
