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
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("nil run FILE")
    );
    let version = cli(&["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap(),
        format!(
            "nil {} (lines-v0, interpreter)\n",
            env!("CARGO_PKG_VERSION")
        )
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
        "check",
        "../../crates/nil-compiler/tests/fixtures/invalid-value.txt",
    ]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8(out.stderr).unwrap().starts_with("E001 @"));
}
