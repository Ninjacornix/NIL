use std::process::Command;

#[test]
fn help_reports_actual_capabilities() {
    let output = Command::new(env!("CARGO_BIN_EXE_nil"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Parsing and execution are not implemented"));
    assert!(stdout.contains("docs/ROADMAP.md"));
}

#[test]
fn version_matches_package() {
    let output = Command::new(env!("CARGO_BIN_EXE_nil"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("nil {} (scaffold)\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn unimplemented_run_command_fails_without_claiming_execution() {
    let output = Command::new(env!("CARGO_BIN_EXE_nil"))
        .args(["run", "example.nil"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("unsupported arguments")
    );
}
