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
// ADR 038: root counts are reconciled only at observation points. These programs churn
// root slots between observations; every result, trap and span must match the
// reference evaluator at O0 and O2.
#[test]
fn deferred_roots_keep_exact_quota_boundary_in_conditional_builder() {
    // A live buffer leaves ~100 KB of headroom; the builder's geometric growth must hit
    // E013 at the same instruction as the reference evaluator.
    parity(
        "=#b(!bytes(67008000,1))\n(s):s=@(a,\"\",0;c<200000;a,a[c]==1?!concat(b,!bytes(1,a[c])):b,c+1;b)",
        b"",
        Some("E013"),
        256,
    );
    parity(
        "=#b(!bytes(66900000,1))\n(s):s=@(a,\"\",0;c<20000;a,a[c]==1?!concat(b,!bytes(1,a[c])):b,c+1;b)",
        b"20000\n",
        None,
        256,
    );
}
#[test]
fn deferred_roots_survive_collections_inside_callees() {
    // `a` is rooted only by a pending slot store when the callee starts; the callee
    // allocates far beyond the collection budget before the caller reads `a` again.
    parity(
        "=b(!bytes(5000,7))\n(s)=c(3000)+a[4999]+#a\n1=#@(!bytes(0,0),0,a;b<c;!slice(!bytes(200,1),0,1),b+1,c;a)",
        b"5008\n",
        None,
        256,
    );
}
#[test]
fn deferred_roots_keep_builder_aliases_copying() {
    parity(
        ":s=@(\"x\",\"\",0;c<3;!concat(a,!bytes(1,97+c)),a,c+1;!concat(a,b))",
        b"xabcxab\n",
        None,
        256,
    );
    parity(
        ":s=@(\"x\",\"\",0;c<3;c==1?!concat(a,!bytes(1,97+c)):a,a,c+1;!concat(a,b))",
        b"xbxb\n",
        None,
        256,
    );
}
#[test]
fn deferred_roots_relocate_growing_builders() {
    parity(
        "=#@(\"\",0;b<30000;!push(a,1),b+1;a)",
        b"30000\n",
        None,
        256,
    );
    parity(
        "=#@(!buffer(0,0),0;b<10000;!push(a,b),b+1;a)",
        b"10000\n",
        None,
        256,
    );
}
#[test]
fn std_push_matches_concat_singleton_semantics() {
    parity(":s=!push(\"ab\",99)", b"abc\n", None, 256);
    parity(":v=!push(!push(!buffer(0,0),-5),7)", b"[-5,7]\n", None, 256);
    parity(
        ":s=b(\"ab\")\n(s):s=!concat(!push(a,90),a)",
        b"abZab\n",
        None,
        256,
    );
    parity(":s=!push(\"ab\",256)", b"", Some("E014"), 256);
    parity("=#!push(!bytes(67108824,0),1)", b"", Some("E013"), 256);
    parity(":s=!push(\"\",0)", b"\0\n", None, 256);
}
#[test]
fn std_push_reports_arity_and_type_errors() {
    for (source, code) in [
        (":s=!push(\"ab\")", "E006"),
        (":s=!push(\"ab\",1,2)", "E006"),
        ("=!push(1,2)", "E007"),
    ] {
        let error = compile_with_profile(source, SourceProfile::ExprV5).unwrap_err();
        assert_eq!(error.code, code, "{source}");
    }
}
