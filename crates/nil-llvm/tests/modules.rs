use nil_compiler::{
    SourceProfile,
    application::Host,
    compile_with_plugins,
    evaluator::{Limits, Value, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId};
use nil_llvm::{Instrumentation, Optimization, Options};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "nil-module-native-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn module(&self, id: u32, body: &str, imports: &str) -> PathBuf {
        std::fs::write(self.0.join(format!("{id}.nil")), body).unwrap();
        let path = self.0.join(format!("{id}.manifest"));
        std::fs::write(
            &path,
            format!("nil-module 1\nid {id}\nsource {id}.nil\nexport 0 0\n{imports}"),
        )
        .unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
struct MemoryHost {
    stdout: Vec<u8>,
}
impl Host for MemoryHost {
    fn read(&mut self, p: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        std::fs::read(PathBuf::from(std::str::from_utf8(p).unwrap()))
            .map_err(|_| Diagnostic::new("E015", nil_hir::Phase::Execute, None, "missing fixture"))
    }
    fn write(&mut self, p: &[u8], b: &[u8]) -> Result<(), Diagnostic> {
        std::fs::write(PathBuf::from(std::str::from_utf8(p).unwrap()), b)
            .map_err(|_| Diagnostic::new("E015", nil_hir::Phase::Execute, None, "fixture write"))
    }
    fn out(&mut self, b: &[u8]) -> Result<(), Diagnostic> {
        self.stdout.extend_from_slice(b);
        Ok(())
    }
}
fn parity(source: &str, body: &str, expected: &[u8], code: Option<&str>, depth: usize) {
    let f = Fixture::new();
    let path = f.module(1, body, "");
    check(source, &[path], expected, code, depth);
}
fn check(source: &str, paths: &[PathBuf], expected: &[u8], code: Option<&str>, depth: usize) {
    let p = compile_with_plugins(source, SourceProfile::ExprV5, paths).unwrap();
    let mut host = MemoryHost::default();
    let result = execute_values_with_host(
        &p.hir,
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
        let Value::I64(v) = result.unwrap() else {
            panic!("scalar test entry")
        };
        assert_eq!(
            [host.stdout, format!("{v}\n").into_bytes()].concat(),
            expected
        );
    }
    for optimization in [Optimization::O0, Optimization::O2] {
        let out = nil_llvm::run_arguments(
            &p.hir,
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
        assert_eq!(out.stdout, expected, "{optimization:?}: {source}");
        if let Some(code) = code {
            assert!(!out.status.success());
            assert!(String::from_utf8_lossy(&out.stderr).starts_with(code));
        } else {
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(out.stderr.is_empty());
        }
    }
}
#[test]
fn imported_allocations_return_immutable_values_and_preserve_old_aliases() {
    for (root, body, expected) in [
        (
            "=#!plugin(1,0,\"abc\")",
            "(s):s=!concat(a,\"xyz\")",
            b"6\n".as_slice(),
        ),
        (
            "=b(\"abc\")\n(s)=!plugin(1,0,a)[0]+a[0]",
            "(s):s=a[0:255]",
            b"352\n",
        ),
        (
            "=@(\"\",0;b<100;!plugin(1,0,a),b+1;#a)",
            "(s):s=!concat(a,\"x\")",
            b"100\n",
        ),
        (
            "record R(data:s)\n=!plugin(1,0,R(\"old\")).data[0]",
            "(R):R=a{data:!concat(a.data,\"new\")}",
            b"111\n",
        ),
        (
            "record R(value:i)\n=#!plugin(1,0,!buffer[R](1,R(7)))",
            "(v[R]):v[R]=!concat(a,!buffer[R](1,R(9)))",
            b"2\n",
        ),
        (
            "=!get(!plugin(1,0,!map()),\"k\")",
            "(m):m=!put(a,\"k\",42)",
            b"42\n",
        ),
    ] {
        parity(root, body, expected, None, 256);
    }
}
#[test]
fn imported_effects_are_lazy_and_left_to_right() {
    for (root, body, expected, code) in [
        (
            "=!out(\"A\")+!plugin(1,0,!out(\"B\"))+!out(\"D\")",
            "1=!out(\"C\")+a",
            b"ABCD4\n".as_slice(),
            None,
        ),
        (
            "=!out(\"A\")+(false?!plugin(1,0,0):42)+!out(\"C\")",
            "1=!out(\"BAD\")+1/0",
            b"AC44\n",
            None,
        ),
        (
            "=!out(\"A\")+!plugin(1,0,0)+!out(\"C\")",
            "1=!out(\"B\")+1/0",
            b"AB",
            Some("E009"),
        ),
        (
            "=!plugin(1,0,!parse(\"bad\"),!bytes(-1,256))",
            "(i,s)=!out(\"BAD\")",
            b"",
            Some("E016"),
        ),
    ] {
        parity(root, body, expected, code, 256);
    }
}
#[test]
fn imported_failure_codes_and_trap_priority_match_at_o0_o2() {
    for (body, code) in [
        ("=\"x\"[1]", "E012"),
        (":s=!bytes(-1,256)", "E013"),
        (":s=\"x\"[0:256]", "E014"),
        (":s=!read(\"/nil-module-missing-fixture\")", "E015"),
        ("=!parse(\"bad\")", "E016"),
        (":s=!read(\"bad\\0path\")", "E017"),
        ("=!get(!map(),\"missing\")", "E019"),
    ] {
        let root = if body.starts_with(":s") {
            "=#!plugin(1,0)"
        } else {
            "=!plugin(1,0)"
        };
        parity(root, body, b"", Some(code), 256);
    }
}
#[test]
fn imported_live_aliases_enforce_quota_and_dead_values_release_it() {
    parity(
        "=b(!bytes(40000000,0))\n(s)=c(!plugin(1,0,a),!bytes(30000000,0))\n(s,s)=#a+#b",
        "(s):s=a",
        b"",
        Some("E013"),
        256,
    );
    parity(
        "=b(!bytes(40000000,0))\n(s)=#!plugin(1,0,a)+#!bytes(30000000,0)",
        "(s):s=a",
        b"70000000\n",
        None,
        256,
    );
    parity(
        "=#!plugin(1,0)",
        ":s=!bytes(67108864,0)",
        b"",
        Some("E013"),
        256,
    );
}
#[test]
fn imported_self_and_mutual_recursion_preserve_depth_limits() {
    parity("=!plugin(1,0,240)", "1=a==0?42:a(a-1)", b"42\n", None, 256);
    parity(
        "=!plugin(1,0,240)",
        "1=a==0?42:b(a-1)\n1=a==0?42:a(a-1)",
        b"42\n",
        None,
        256,
    );
    parity(
        "=!plugin(1,0,256)",
        "1=a==0?42:a(a-1)",
        b"",
        Some("E008"),
        256,
    );
}
#[test]
fn transitive_modules_share_arena_effects_and_ordinary_calls() {
    let f = Fixture::new();
    f.module(2, "(s):s=!concat(a,\"x\")", "");
    let path = f.module(1, "(s):s=!plugin(2,0,a)", "import 2.manifest\n");
    check("=#!plugin(1,0,\"abc\")", &[path], b"4\n", None, 256);
}
#[test]
fn imported_file_pipeline_preserves_fixture_bytes() {
    let f = Fixture::new();
    let input = f.0.join("input");
    let output = f.0.join("output");
    std::fs::write(&input, b"a\0\xff").unwrap();
    let path = f.module(1, "(s,s)=!write(b,!concat(!read(a),\"x\"))", "");
    let source = format!(
        "=!plugin(1,0,\"{}\",\"{}\")",
        input.display(),
        output.display()
    );
    check(&source, &[path], b"4\n", None, 256);
    assert_eq!(std::fs::read(output).unwrap(), b"a\0\xffx");
}
#[test]
fn module_calls_keep_borrowing_and_rootless_scan_proofs_before_inlining() {
    let f = Fixture::new();
    let path = f.module(1, "(s,i)=a[b]==10?1:0", "");
    let p = compile_with_plugins(
        "(s)=@(a,0,0;b<#a;a,b+1,c+!plugin(1,0,a,b);c)",
        SourceProfile::ExprV5,
        &[path],
    )
    .unwrap();
    assert!(nil_hir::borrowing::Summaries::analyze(&p.hir).function(0));
    let ir = nil_llvm::emit_llvm(&p.hir);
    assert!(!ir.contains("call ptr @nil_roots_enter("));
    assert!(nil_hir::plugin::providers(&p.hir).is_empty());
    let path = f.module(1, "(s):s=!concat(a,\"x\")", "");
    let p = compile_with_plugins("(s)=#!plugin(1,0,a)", SourceProfile::ExprV5, &[path]).unwrap();
    assert!(!nil_hir::borrowing::Summaries::analyze(&p.hir).function(0));
    assert!(nil_llvm::emit_llvm(&p.hir).contains("call ptr @nil_roots_enter("));
}
