#![cfg(feature = "process")]
#![allow(clippy::err_expect)] // Keep the task brief's timeout assertion verbatim.

use std::time::Duration;

use nyanpasu_utils::process::{Command, ProcessError};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[tokio::test]
async fn output_captures_streams_and_code() {
    let out = Command::new(child())
        .args(["echo-lines", "x"])
        .output()
        .await
        .unwrap();
    assert!(out.success());
    assert_eq!(out.stdout.trim(), "x");
    assert!(out.stderr.contains("stderr-marker"));
}

#[tokio::test]
async fn output_nonzero_is_data_not_error() {
    let out = Command::new(child())
        .args(["exit-with", "5"])
        .output()
        .await
        .unwrap();
    assert!(!out.success());
    assert_eq!(out.code, Some(5));
}

#[tokio::test]
async fn output_timeout_is_error() {
    let err = Command::new(child())
        .args(["sleep-forever"])
        .timeout(Duration::from_millis(300))
        .output()
        .await
        .err()
        .expect("must time out");
    assert!(matches!(err, ProcessError::Timeout { .. }));
}
