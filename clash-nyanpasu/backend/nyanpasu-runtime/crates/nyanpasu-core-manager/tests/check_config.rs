mod common;

use nyanpasu_core_manager::{Error, kind::check_config};

#[tokio::test]
async fn check_config_passes_and_fails() {
    let (_guard, dir) = common::utf8_tempdir();
    let ok_config = common::write_config(&dir, "mixed-port: 7890\n");
    let spec = common::mihomo_spec(&dir, ok_config);
    check_config(&spec).await.expect("valid config passes");

    let bad_config = dir.join("bad.yaml");
    std::fs::write(
        &bad_config,
        "x-fake-core:\n  check-fail: port already in use\n",
    )
    .unwrap();
    let mut bad_spec = common::mihomo_spec(&dir, bad_config);
    bad_spec.config_path = dir.join("bad.yaml");
    let err = check_config(&bad_spec).await.expect_err("must fail");
    match err {
        Error::ConfigCheckFailed(msg) => assert_eq!(msg, "port already in use"),
        other => panic!("unexpected error: {other}"),
    }
}

/// A core that never answers `-t` must not hold the caller — and must not
/// survive being given up on. `check-delay-ms` is two orders of magnitude above
/// the bound, so the elapsed time is the proof that the wait was cut short, and
/// the marker file is the proof that a process really was spawned to cut short.
///
/// Gap, stated: this asserts the call returns, not that the child was reaped.
/// The tree kill belongs to `nyanpasu_utils::process::Command::timeout`
/// (`command.rs:105`) in a read-only crate, and there is no portable handle to
/// assert on from here.
#[tokio::test]
async fn a_hung_check_is_bounded_by_its_timeout() {
    use nyanpasu_core_manager::kind::check_config_within;
    use std::time::{Duration, Instant};

    let (_guard, dir) = common::utf8_tempdir();
    let started = dir.join("check-started");
    let config = dir.join("hang.yaml");
    std::fs::write(
        &config,
        format!("x-fake-core:\n  check-delay-ms: 30000\n  check-started-file: '{started}'\n"),
    )
    .unwrap();
    let spec = common::mihomo_spec(&dir, config);

    let began = Instant::now();
    let err = check_config_within(&spec, Duration::from_millis(300))
        .await
        .expect_err("a hung check must fail");
    let elapsed = began.elapsed();

    assert!(
        elapsed < Duration::from_secs(10),
        "the check must not wait for the core: {elapsed:?}"
    );
    assert!(started.exists(), "the fake core must actually have started");
    match err {
        Error::ConfigCheckFailed(msg) => assert!(msg.contains("timed out"), "{msg}"),
        other => panic!("unexpected error: {other}"),
    }
}
