#![cfg(feature = "process")]

use std::time::Duration;

use nyanpasu_utils::process::{Command, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

fn pid_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
    let kind = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
    let mut s = System::new_with_specifics(kind);
    s.refresh_specifics(kind);
    s.process(Pid::from_u32(pid)).is_some()
}

#[tokio::test]
async fn kill_terminates_and_wait_returns() {
    let (handle, mut rx) = Command::new(child())
        .args(["sleep-forever"])
        .spawn()
        .await
        .unwrap();
    // wait for "ready" so we know it's running
    loop {
        match rx.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("ready") => break,
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    }
    handle.kill().await.unwrap();
    let payload = handle.wait().await.unwrap();
    assert_ne!(payload.code, Some(0)); // hard kill is never a clean exit
    // killing again is idempotent-Ok
    handle.kill().await.unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn graceful_kill_delivers_sigterm_first() {
    let (handle, mut rx) = Command::new(child())
        .args(["trap-term"])
        .spawn()
        .await
        .unwrap();
    loop {
        match rx.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("ready") => break,
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    }
    handle.graceful_kill().await.unwrap();
    // trap-term exits 0 on SIGTERM -> proves the graceful tier was delivered
    assert_eq!(handle.wait().await.unwrap().code, Some(0));
}

#[cfg(windows)]
#[tokio::test]
async fn graceful_kill_equals_kill_on_windows() {
    let (handle, _rx) = Command::new(child())
        .args(["sleep-forever"])
        .spawn()
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.graceful_kill().await.unwrap();
    assert_ne!(handle.wait().await.unwrap().code, Some(0));
}

#[tokio::test]
async fn whole_tree_is_reaped() {
    let (handle, mut rx) = Command::new(child())
        .args(["spawn-grandchild"])
        .spawn()
        .await
        .unwrap();
    let grandchild_pid: u32 = loop {
        match rx.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("grandchild-pid:") => {
                break l
                    .trim()
                    .trim_start_matches("grandchild-pid:")
                    .parse()
                    .unwrap();
            }
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    };
    assert!(pid_alive(grandchild_pid));
    handle.kill().await.unwrap();
    handle.wait().await.unwrap();
    // the kernel object must have reaped the grandchild too
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!pid_alive(grandchild_pid), "grandchild survived tree kill");
}
