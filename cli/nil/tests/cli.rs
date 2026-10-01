use std::process::{Command, Output};
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nil"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}
const EXAMPLE: &str = "../../examples/add.nil";
#[test]
fn help_and_version_match_capabilities() {
    let help = cli(&["--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8(help.stdout).unwrap().contains("run FILE"));
    let version = cli(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap(),
        format!(
            "nil {} (expr-v0 default, lines-v0 optional, LLVM native default)\n",
            env!("CARGO_PKG_VERSION")
        )
    );
}
#[test]
fn expression_profile_runs_and_checks_sample() {
    let file = "../../benchmarks/paired/samples/affine.expr.nil";
    let check = cli(&["check", file]);
    assert!(check.status.success(), "{check:?}");
    let run = cli(&["run", file, "0", "20", "22"]);
    assert!(run.status.success(), "{run:?}");
    assert_eq!(run.stdout, b"124\n");
    assert_eq!(
        cli(&["--profile", "unknown", "check", file]).status.code(),
        Some(2)
    );
}
#[test]
fn runs_example_and_selected_function() {
    for args in [vec!["run", EXAMPLE], vec!["run", EXAMPLE, "1", "20", "22"]] {
        let out = cli(&args);
        assert!(out.status.success(), "{:?}", out);
        assert_eq!(out.stdout, b"42\n");
        assert!(out.stderr.is_empty());
    }
}
#[test]
fn checks_and_dumps_hir() {
    let out = cli(&["check", EXAMPLE]);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"ok\n");
    let out = cli(&["hir", EXAMPLE]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("Constant(22)")
    );
}
#[test]
fn usage_errors_have_exit_two() {
    for args in [
        vec!["wat"],
        vec!["run"],
        vec!["check", EXAMPLE, "extra"],
        vec!["run", EXAMPLE, "bad"],
        vec!["run", EXAMPLE, "1", "bad"],
    ] {
        let out = cli(&args);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8(out.stderr).unwrap().starts_with("E010"));
    }
}
#[test]
fn missing_file_entry_and_wrong_arity_fail() {
    for (args, code) in [
        (vec!["run", "no-such-file.nil"], "E010"),
        (vec!["run", EXAMPLE, "999"], "E004"),
        (vec!["run", EXAMPLE, "1"], "E006"),
    ] {
        let out = cli(&args);
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert!(String::from_utf8(out.stderr).unwrap().starts_with(code));
    }
}
#[test]
fn malformed_file_has_compiler_diagnostic() {
    let out = cli(&[
        "--profile",
        "lines-v0",
        "check",
        "../../crates/nil-compiler/tests/fixtures/invalid-value.txt",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stderr).unwrap().starts_with("E001 @"));
}

#[test]
fn all_examples_execute_with_checked_in_results() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut count = 0;
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("nil") {
            continue;
        }
        let expected = std::fs::read_to_string(path.with_extension("stdout"))
            .expect("each example needs an expected .stdout result and entry function 0");
        let out = cli(&["run", path.to_str().unwrap()]);
        assert!(out.status.success(), "{}: {:?}", path.display(), out);
        assert_eq!(
            String::from_utf8(out.stdout).unwrap(),
            expected.replace("\r\n", "\n"),
            "{}",
            path.display()
        );
        assert!(out.stderr.is_empty());
        count += 1;
    }
    assert!(count > 0, "example suite must not silently become empty");
}

#[test]
fn expression_is_implicit_default_and_lines_is_explicit_compatibility() {
    let expression = "../../benchmarks/paired/samples/affine.expr.nil";
    let lines = "../../benchmarks/paired/samples/affine.nil";
    let default = cli(&["run", expression, "0", "20", "22"]);
    assert!(default.status.success(), "{default:?}");
    assert_eq!(default.stdout, b"124\n");

    let implicit_lines = cli(&["check", lines]);
    assert_eq!(implicit_lines.status.code(), Some(1));
    assert!(
        String::from_utf8(implicit_lines.stderr)
            .unwrap()
            .starts_with("E001")
    );

    let explicit_lines = cli(&["--profile", "lines-v0", "run", lines, "0", "20", "22"]);
    assert!(explicit_lines.status.success(), "{explicit_lines:?}");
    assert_eq!(explicit_lines.stdout, b"124\n");
}

#[test]
fn experimental_compact_profiles_execute_through_the_cli() {
    for (profile, file) in [
        ("expr-v1", "../../benchmarks/paired/samples/squares.v1"),
        ("expr-v2", "../../benchmarks/paired/samples/squares.v2"),
    ] {
        let out = cli(&["--profile", profile, "run", file, "0", "3", "4"]);
        assert!(out.status.success());
        assert_eq!(String::from_utf8(out.stdout).unwrap(), "25\n");
        let out = cli(&["--profile", profile, "check", file]);
        assert!(out.status.success());
    }
}

#[test]
fn compact_control_flow_runs_checks_and_dumps_through_cli() {
    let file = "../../benchmarks/paired/control-samples/factorial.v2.nil";
    let out = cli(&["--profile", "expr-v2", "run", file, "0", "10"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "3628800\n");
    assert!(
        cli(&["--profile", "expr-v2", "check", file])
            .status
            .success()
    );
    let out = cli(&["--profile", "expr-v2", "hir", file]);
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout).unwrap().contains("Loop"));
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn default_run_requires_llvm_and_has_no_interpreter_fallback() {
    let out = Command::new(env!("CARGO_BIN_EXE_nil"))
        .args(["run", EXAMPLE])
        .env("NIL_CLANG", "nil-clang-does-not-exist")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8(out.stderr).unwrap().starts_with("E011"));
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn llvm_build_produces_an_independent_executable_and_preserves_files_on_failure() {
    let directory = std::env::temp_dir().join(format!("nil-cli-native-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("source with spaces.nil");
    let binary = directory.join("compiled program");
    std::fs::write(&source, "f17(a,b)=a+b\n").unwrap();
    let out = cli(&[
        "build",
        source.to_str().unwrap(),
        "-o",
        binary.to_str().unwrap(),
        "--entry",
        "17",
        "-O0",
    ]);
    assert!(out.status.success(), "{out:?}");
    std::fs::remove_file(&source).unwrap();
    let out = Command::new(&binary).args(["20", "22"]).output().unwrap();
    assert!(out.status.success());
    assert_eq!(out.stdout, b"42\n");
    std::fs::write(&source, "f0()=42\n").unwrap();
    let original = std::fs::read(&source).unwrap();
    let out = cli(&[
        "build",
        source.to_str().unwrap(),
        "-o",
        source.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(std::fs::read(&source).unwrap(), original);
    let existing = std::fs::read(&binary).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_nil"))
        .args([
            "build",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .env("NIL_CLANG", "nil-clang-does-not-exist")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(std::fs::read(&binary).unwrap(), existing);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn llvm_module_and_native_usage_are_exposed() {
    let out = cli(&["llvm", EXAMPLE]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("define i64 @nil_fn0")
    );
    for args in [
        vec!["build", EXAMPLE],
        vec!["build", EXAMPLE, "-o", "x", "-O3"],
        vec!["build", EXAMPLE, "--entry", "bad", "-o", "x"],
    ] {
        assert_eq!(cli(&args).status.code(), Some(2));
    }
}

#[test]
fn expr_v3_is_explicit_wrapping_native_and_can_be_instrumented() {
    let source = "../../examples/expr-v3/wrapping.nil";
    for flags in [
        vec!["--profile", "expr-v3"],
        vec!["--profile", "expr-v3", "--bounded"],
    ] {
        let mut args = flags;
        args.extend(["run", source]);
        let out = cli(&args);
        assert!(out.status.success(), "{out:?}");
        assert_eq!(out.stdout, b"-9223372036854775808\n");
    }
    let legacy = cli(&["--profile", "expr-v2", "run", source]);
    assert_eq!(legacy.status.code(), Some(1));
    assert!(String::from_utf8(legacy.stderr).unwrap().contains("E009"));
    let ir = cli(&["--profile", "expr-v3", "llvm", source]);
    assert!(ir.status.success());
    assert!(
        !String::from_utf8(ir.stdout)
            .unwrap()
            .contains("call void @nil_tick")
    );
    let ir = cli(&["--profile", "expr-v3", "--bounded", "llvm", source]);
    assert!(ir.status.success());
    assert!(
        String::from_utf8(ir.stdout)
            .unwrap()
            .contains("call void @nil_tick")
    );
    assert_eq!(
        cli(&["--profile", "expr-v3", "--bounded", "check", source])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        cli(&[
            "--profile",
            "expr-v3",
            "--bounded",
            "--unbounded",
            "run",
            source
        ])
        .status
        .code(),
        Some(2)
    );
    let out = cli(&[
        "--profile",
        "expr-v3",
        "run",
        "../../examples/expr-v3/weighted.nil",
    ]);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"33\n");
    let out = cli(&[
        "--profile",
        "expr-v3",
        "run",
        "../../examples/expr-v3/sum.nil",
    ]);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"500500\n");
}

#[test]
fn typed_profile_flattens_array_arguments_and_formats_typed_results() {
    let result = cli(&[
        "--profile",
        "expr-v4",
        "run",
        "../../examples/expr-v4/reverse.nil",
        "0",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
    ]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"[8,7,6,5,4,3,2,1]\n");
    let result = cli(&[
        "--profile",
        "expr-v4",
        "run",
        "../../examples/expr-v4/predicate.nil",
        "0",
        "1",
    ]);
    assert!(result.status.success(), "{result:?}");
    assert_eq!(result.stdout, b"true\n");
    let result = cli(&[
        "--profile",
        "expr-v4",
        "run",
        "../../examples/expr-v4/reverse.nil",
        "0",
        "1",
    ]);
    assert_eq!(result.status.code(), Some(1));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .starts_with("E006")
    );
}
