use std::{
    fs,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
fn temp() -> std::path::PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::env::temp_dir().join(format!(
        "nil-fuzz-runner-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}
#[test]
fn cli_campaign_and_replay_emit_reproducible_reports() {
    let out = temp();
    let result = Command::new(env!("CARGO_BIN_EXE_nil-fuzz"))
        .args(["--cases", "8", "--native", "0", "--seed", "0", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let report = String::from_utf8(result.stdout).unwrap();
    assert!(report.contains("\"valid_cases\":8"));
    assert!(report.contains("\"mutations\":40"));
    assert!(out.join("current-input.bin").is_file());
    let replay = Command::new(env!("CARGO_BIN_EXE_nil-fuzz"))
        .arg("--replay")
        .arg(out.join("current-input.bin"))
        .args(["--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(replay.status.success(), "{replay:?}");
    fs::remove_dir_all(out).unwrap();
}
#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn native_campaign_compares_oracle_reference_and_o0_o2_instrumentation_modes() {
    let out = temp();
    let result = Command::new(env!("CARGO_BIN_EXE_nil-fuzz"))
        .args(["--cases", "2", "--native", "2", "--seed", "100", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{result:?}; artifacts: {}",
        out.display()
    );
    assert!(
        String::from_utf8(result.stdout)
            .unwrap()
            .contains("\"native_builds\":24")
    );
    assert!(!out.join("native-100").exists());
    fs::remove_dir_all(out).unwrap();
}
#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn native_toolchain_failure_retains_source_mode_and_worker_log() {
    let out = temp();
    let result = Command::new(env!("CARGO_BIN_EXE_nil-fuzz"))
        .env("NIL_CLANG", "/nil-fuzz-deliberately-missing-clang")
        .args(["--cases", "1", "--native", "1", "--seed", "42", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!result.status.success());
    for name in ["source.nil", "mode.txt", "module.ll", "worker.log"] {
        assert!(out.join("native-42").join(name).is_file(), "{name}");
    }
    assert!(
        fs::read_to_string(out.join("native-42/worker.log"))
            .unwrap()
            .contains("E011")
    );
    fs::remove_dir_all(out).unwrap();
}
