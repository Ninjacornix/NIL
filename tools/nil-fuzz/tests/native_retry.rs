//! A native case whose first wall deadline expires must be retried, not failed.
//!
//! The deadline is wall clock, so under load it used to fail cases that were
//! merely slow: a concurrent gate run reported "case 147: native timeout" for a
//! program that is not slow at all. A first expiry is now treated as suspect —
//! the harness is quiesced and the same child gets one more window — and only a
//! second expiry is a real timeout.
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("nil-fuzz-retry-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    path
}

/// One test, not two: the window is process-global, so separate tests would
/// race when the harness runs a file's tests in parallel.
#[test]
fn a_first_window_expiry_is_retried_and_an_unrushed_campaign_never_retries() {
    // One millisecond guarantees the first window expires for every case, which
    // is the situation load used to create. The campaign must still pass.
    nil_fuzz::application::set_first_window_ms(1);
    let before = nil_fuzz::application::native_retries();
    let rushed = scratch("expiry");
    let result = nil_fuzz::application::io_fault_campaign(61001, &rushed, 2);
    nil_fuzz::application::set_first_window_ms(10_000);
    let retries = nil_fuzz::application::native_retries() - before;
    assert!(
        result.is_ok(),
        "a rushed first window must not fail the campaign: {result:?}"
    );
    assert!(
        retries > 0,
        "the retry path was never taken, so this test proves nothing"
    );
    let _ = std::fs::remove_dir_all(&rushed);

    // With the production window, a retry means a real environment problem, so
    // the count must stay at zero on an idle machine.
    let before = nil_fuzz::application::native_retries();
    let quiet = scratch("quiet");
    nil_fuzz::application::io_fault_campaign(83003, &quiet, 2).expect("campaign");
    assert_eq!(
        nil_fuzz::application::native_retries() - before,
        0,
        "an unrushed campaign retried, which means the deadline is too tight"
    );
    let _ = std::fs::remove_dir_all(&quiet);
}
