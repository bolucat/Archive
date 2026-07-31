#![cfg(feature = "process")]

use std::time::Duration;

use nyanpasu_utils::process::{Command, ProcessError, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[tokio::test]
async fn write_stdin_roundtrip() {
    let (handle, mut rx) = Command::new(child())
        .args(["echo-stdin"])
        .pipe_stdin(true)
        .spawn()
        .await
        .unwrap();
    handle.write_stdin(b"ping\n").await.unwrap();
    let mut echoed = None;
    while let Some(e) = rx.recv().await {
        if let ProcessEvent::Stdout(l) = e {
            echoed = Some(l);
            break;
        }
    }
    assert_eq!(echoed.unwrap().trim(), "echo:ping");
}

#[tokio::test]
async fn write_stdin_without_pipe_is_error() {
    let (handle, _rx) = Command::new(child())
        .args(["sleep-forever"])
        .spawn()
        .await
        .unwrap();
    let err = handle.write_stdin(b"x").await.err().unwrap();
    assert!(matches!(err, ProcessError::StdinUnavailable));
    handle.kill().await.unwrap();
}

#[tokio::test]
async fn write_stdin_does_not_stall_output_pump() {
    let (handle, mut rx) = Command::new(child())
        .args(["spam-stdout", "10000"])
        .pipe_stdin(true)
        .event_channel_capacity(8)
        .spawn()
        .await
        .unwrap();

    let big = vec![b'x'; 1 << 20];
    let h2 = handle.clone();
    // Pre-fix, the pump awaited this 1 MiB write inline, so stdout draining stopped and
    // the child blocked on its full stdout pipe without exiting; post-fix, the pump
    // keeps draining while the write is parked on the dedicated writer task.
    let write_task = tokio::spawn(async move { h2.write_stdin(&big).await });

    let events = tokio::time::timeout(Duration::from_secs(60), async {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        events
    })
    .await
    .expect("pump stalled: regression of full-duplex deadlock");
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ProcessEvent::Stdout(_)))
            .count(),
        10000
    );
    assert!(matches!(events.last(), Some(ProcessEvent::Terminated(_))));

    // Child death may resolve the write as Ok or Err(StdinUnavailable), depending on how much the OS buffered.
    let _write_result = tokio::time::timeout(Duration::from_secs(10), write_task)
        .await
        .expect("stdin write never resolved")
        .unwrap();
}

#[tokio::test]
async fn write_stdin_after_termination_is_unavailable() {
    let (handle, _rx) = Command::new(child())
        .args(["exit-with", "0"])
        .pipe_stdin(true)
        .spawn()
        .await
        .unwrap();

    handle.wait().await.unwrap();
    assert!(matches!(
        handle.write_stdin(b"too late").await,
        Err(ProcessError::StdinUnavailable)
    ));
}
