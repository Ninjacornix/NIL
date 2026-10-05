use nil_compiler::{
    SourceProfile,
    application::{FileHost, Host, SeededHost},
    compile_with_profile,
    evaluator::{Limits, Value, execute_values, execute_values_with_host},
};
use nil_hir::{Diagnostic, FunctionId, Phase};
use nil_llvm::{Optimization, Options};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
#[derive(Default)]
struct MemoryHost {
    env: BTreeMap<Vec<u8>, Vec<u8>>,
    denied: bool,
}
impl Host for MemoryHost {
    fn env(&mut self, name: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        if self.denied {
            return Err(Diagnostic::new("E018", Phase::Execute, None, "denied"));
        }
        Ok(self.env.get(name).cloned().unwrap_or_default())
    }
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nil-host-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn bytes(v: Value) -> Vec<u8> {
    match v {
        Value::Bytes(v) => [v.as_ref(), b"\n"].concat(),
        Value::Map(v) => format!("{}\n", v.render()).into_bytes(),
        _ => format!("{}\n", nil_compiler::numeric::format(&v)).into_bytes(),
    }
}
fn parity(source: &str, host: &mut dyn Host, env: &[(&str, Vec<u8>)], denied: bool) {
    let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
    let expected = execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), host);
    let f = Fixture::new();
    for optimization in [Optimization::O0, Optimization::O2] {
        let binary = f.0.join("program");
        nil_llvm::build(
            &p.hir,
            &binary,
            Options {
                optimization,
                ..Default::default()
            },
        )
        .unwrap();
        let mut cmd = Command::new(binary);
        cmd.env_remove("NIL_DENY_HOST_IO")
            .env("NIL_RANDOM_SEED", "0")
            .env_remove("NIL_EFFECT_VALUE")
            .env_remove("NIL_EFFECT_MISSING");
        if denied {
            cmd.env("NIL_DENY_HOST_IO", "1");
        }
        for (name, value) in env {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                cmd.env(name, std::ffi::OsString::from_vec(value.clone()));
            }
        }
        let actual = cmd.output().unwrap();
        match &expected {
            Ok(v) => {
                assert!(
                    actual.status.success(),
                    "{optimization:?}: {}",
                    String::from_utf8_lossy(&actual.stderr)
                );
                assert_eq!(actual.stdout, bytes(v.clone()), "{source}");
            }
            Err(d) => {
                assert!(!actual.status.success(), "{source}");
                assert!(
                    actual.stderr.starts_with(d.code.as_bytes()),
                    "{optimization:?} expected {}: {}",
                    d.code,
                    String::from_utf8_lossy(&actual.stderr)
                );
            }
        }
    }
}
#[test]
fn environment_present_absent_empty_and_os_bytes_match() {
    for value in [b"value".to_vec(), vec![], vec![0xff, 0x80, b'x']] {
        let env = BTreeMap::from([(b"NIL_EFFECT_VALUE".to_vec(), value.clone())]);
        parity(
            ":s=!env(\"NIL_EFFECT_VALUE\")",
            &mut MemoryHost { env, denied: false },
            &[("NIL_EFFECT_VALUE", value)],
            false,
        );
    }
    parity(
        ":s=!env(\"NIL_EFFECT_MISSING\")",
        &mut MemoryHost::default(),
        &[],
        false,
    );
}
#[test]
fn invalid_environment_names_precede_host_denial() {
    for source in [":s=!env(\"\")", ":s=!env(\"a=b\")", ":s=!env(\"a\\0\")"] {
        parity(
            source,
            &mut MemoryHost {
                denied: true,
                ..Default::default()
            },
            &[],
            true,
        );
    }
}
#[test]
fn new_effects_require_explicit_reference_host() {
    for source in [
        ":s=!env(\"HOME\")",
        ":u64=!random()",
        "=!size(!directory(\".\"))",
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        assert_eq!(
            execute_values(&p.hir, FunctionId(0), &[], Limits::default())
                .unwrap_err()
                .code,
            "E018"
        );
        parity(
            source,
            &mut nil_compiler::application::DeniedHost,
            &[],
            true,
        );
    }
}
#[test]
fn seeded_random_sequence_lazy_arms_and_called_state_are_exact() {
    for source in [
        ":u64=!random()",
        ":s=!concat(!format(!random()),!format(!random()))",
        ":u64=false?!random():!random()",
        ":s=b(!random())\n(u64):s=!concat(!format(a),!format(!random()))",
        ":s=@(0,\"\";a<8;a+1,!concat(b,!format(!random()));b)",
    ] {
        parity(
            source,
            &mut SeededHost::new(MemoryHost::default(), 0),
            &[],
            false,
        );
    }
}
#[test]
fn directory_order_empty_and_utf8_os_names_are_exact() {
    let f = Fixture::new();
    let dir = f.0.join("entries");
    std::fs::create_dir(&dir).unwrap();
    let source = format!("=!size(!directory(\"{}\"))", dir.display());
    parity(&source, &mut FileHost::default(), &[], false);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        for name in [
            b"z".to_vec(),
            b"a".to_vec(),
            "é".as_bytes().to_vec(),
            b"space name".to_vec(),
        ] {
            std::fs::write(dir.join(std::ffi::OsString::from_vec(name)), b"").unwrap();
        }
    }
    let source = format!(
        ":s=!each(!directory(\"{}\"),\"\";!concat(c,!concat(a,\"/\"));a)",
        dir.display()
    );
    parity(&source, &mut FileHost::default(), &[], false);
}
#[test]
fn directory_missing_regular_file_and_nul_fail_identically() {
    let f = Fixture::new();
    let file = f.0.join("file");
    std::fs::write(&file, b"").unwrap();
    for path in [
        file.to_string_lossy().to_string(),
        f.0.join("missing").to_string_lossy().to_string(),
        "a\\0b".into(),
    ] {
        parity(
            &format!("=!size(!directory(\"{path}\"))"),
            &mut FileHost::default(),
            &[],
            false,
        );
    }
}
#[test]
fn environment_result_quota_is_checked_after_lookup() {
    struct Large;
    impl Host for Large {
        fn env(&mut self, _: &[u8]) -> Result<Vec<u8>, Diagnostic> {
            Ok(vec![0; nil_hir::MAX_DYNAMIC_BYTES])
        }
    }
    let p = compile_with_profile("=#!env(\"LARGE\")", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        execute_values_with_host(&p.hir, FunctionId(0), &[], Limits::default(), &mut Large)
            .unwrap_err()
            .code,
        "E013"
    );
}
#[test]
fn malformed_seed_traps_only_in_selected_random_arm() {
    let f = Fixture::new();
    for (source, success) in [
        (":u64=!random()", false),
        (":u64=false?!random():7u64", true),
    ] {
        let p = compile_with_profile(source, SourceProfile::ExprV5).unwrap();
        for optimization in [Optimization::O0, Optimization::O2] {
            let binary = f.0.join("program");
            nil_llvm::build(
                &p.hir,
                &binary,
                Options {
                    optimization,
                    ..Default::default()
                },
            )
            .unwrap();
            let out = Command::new(binary)
                .env_remove("NIL_DENY_HOST_IO")
                .env("NIL_RANDOM_SEED", "01")
                .output()
                .unwrap();
            assert_eq!(out.status.success(), success);
            if !success {
                assert!(out.stderr.starts_with(b"E016"));
            } else {
                assert_eq!(out.stdout, b"7\n");
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[test]
fn unavailable_os_entropy_matches_reference_io_failure_at_o0_and_o2() {
    struct Unavailable;
    impl Host for Unavailable {
        fn random(&mut self) -> Result<u64, Diagnostic> {
            Err(Diagnostic::new(
                "E015",
                Phase::Execute,
                None,
                "entropy unavailable",
            ))
        }
    }
    let p = compile_with_profile(":u64=!random()", SourceProfile::ExprV5).unwrap();
    assert_eq!(
        execute_values_with_host(
            &p.hir,
            FunctionId(0),
            &[],
            Limits::default(),
            &mut Unavailable
        )
        .unwrap_err()
        .code,
        "E015"
    );
    let f = Fixture::new();
    for optimization in [Optimization::O0, Optimization::O2] {
        let binary = f.0.join("program");
        nil_llvm::build(
            &p.hir,
            &binary,
            Options {
                optimization,
                ..Default::default()
            },
        )
        .unwrap();
        // Force a real OS failure, without a production failure-injection escape hatch.
        let out = Command::new("/usr/bin/sandbox-exec")
            .args([
                "-p",
                "(version 1)(allow default)(deny file-read-data (literal \"/dev/urandom\"))",
            ])
            .arg(binary)
            .env_remove("NIL_RANDOM_SEED")
            .env_remove("NIL_DENY_HOST_IO")
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(
            out.stderr.starts_with(b"E015"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
