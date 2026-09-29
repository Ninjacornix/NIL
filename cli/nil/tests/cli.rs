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
            "nil {} (expr-v0 default, lines-v0 optional, interpreter)\n",
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
