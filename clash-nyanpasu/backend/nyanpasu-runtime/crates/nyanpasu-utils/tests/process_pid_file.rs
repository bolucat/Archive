#![cfg(feature = "process")]

use nyanpasu_utils::process::{
    Command, EpochPidFile, EpochPidFileSpec, OrphanReapOutcome, ProcessEvent, read_epoch_pid_file,
    reap_epoch_pid_file,
};

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
async fn pid_file_written_and_cleaned_up() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core.pid");
    let (handle, rx) = Command::new(child())
        .args(["exit-with", "0"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    // pid file exists right after spawn and contains the pid
    let content: u32 = std::fs::read_to_string(&pid_path)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(content, handle.pid());
    // drain to termination -> file removed
    let mut rx = rx;
    while rx.recv().await.is_some() {}
    assert!(!pid_path.exists());
}

#[tokio::test]
async fn epoch_pid_file_written_and_cleaned_up() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-6.pid");
    let runtime_path = dir.path().join("config-6.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let (handle, mut events) = Command::new(child())
        .args(["sleep-then-exit", "500", "0"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 6,
        }))
        .spawn()
        .await
        .unwrap();
    let record = read_epoch_pid_file(&pid_path).await.unwrap().unwrap();
    assert_eq!(record.pid, handle.pid());
    assert_eq!(record.epoch, 6);
    while events.recv().await.is_some() {}
    assert!(!pid_path.exists());
}

#[tokio::test]
async fn residual_process_is_killed_before_spawn() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-1.pid");
    let runtime_path = dir.path().join("config-1.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let epoch_pid = || {
        EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 1,
        })
    };

    let (h1, mut rx1) = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(epoch_pid())
        .spawn()
        .await
        .unwrap();
    loop {
        match rx1.recv().await.unwrap() {
            ProcessEvent::Stdout(l) if l.contains("ready") => break,
            ProcessEvent::Terminated(_) => panic!("exited early"),
            _ => {}
        }
    }
    let old_pid = h1.pid();

    // A matching same-epoch record must kill the residual first.
    let (h2, _rx2) = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(epoch_pid())
        .spawn()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert!(!pid_alive(old_pid), "residual process not killed");
    let record = read_epoch_pid_file(&pid_path)
        .await
        .unwrap()
        .expect("second epoch record");
    assert_eq!(record.pid, h2.pid());
    assert_eq!(record.epoch, 1);
    assert_eq!(record.runtime_config, runtime_path.canonicalize().unwrap());
    h2.kill().await.unwrap();
}

#[tokio::test]
async fn pid_path_reuse_without_epoch_identity_does_not_kill_live_process() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core.pid");

    let (first, _first_events) = Command::new(child())
        .args(["sleep-forever"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    let first_pid = first.pid();

    let (second, _second_events) = Command::new(child())
        .args(["sleep-forever"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let first_survived = pid_alive(first_pid);

    let _ = first.kill().await;
    let _ = second.kill().await;
    assert!(
        first_survived,
        "a numeric pid file without epoch/start identity must not authorize killing a live process"
    );
}

#[tokio::test]
async fn different_epoch_pid_files_do_not_kill_overlapping_process() {
    let dir = tempfile::tempdir().unwrap();
    let runtime_1 = dir.path().join("config-1.yaml");
    let runtime_2 = dir.path().join("config-2.yaml");
    std::fs::write(&runtime_1, "mixed-port: 0\n").unwrap();
    std::fs::write(&runtime_2, "mixed-port: 0\n").unwrap();

    let (first, _first_events) = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &dir.path().join("core-1.pid"),
            runtime_config: &runtime_1,
            epoch: 1,
        }))
        .spawn()
        .await
        .unwrap();
    let first_pid = first.pid();
    let (second, _second_events) = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &dir.path().join("core-2.pid"),
            runtime_config: &runtime_2,
            epoch: 2,
        }))
        .spawn()
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    assert!(
        pid_alive(first_pid),
        "epoch 2 killed the overlapping epoch 1"
    );
    assert_eq!(
        read_epoch_pid_file(dir.path().join("core-1.pid"))
            .await
            .unwrap()
            .unwrap()
            .pid,
        first_pid
    );
    assert_eq!(
        read_epoch_pid_file(dir.path().join("core-2.pid"))
            .await
            .unwrap()
            .unwrap()
            .pid,
        second.pid()
    );

    let _ = first.kill().await;
    let _ = second.kill().await;
}

#[tokio::test]
async fn validated_orphan_reap_kills_matching_epoch_record() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-9.pid");
    let runtime_path = dir.path().join("config-9.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let (handle, _events) = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 9,
        }))
        .spawn()
        .await
        .unwrap();
    let pid = handle.pid();

    assert_eq!(
        reap_epoch_pid_file(&pid_path, dir.path()).await.unwrap(),
        OrphanReapOutcome::Killed
    );
    assert!(!pid_alive(pid));
    assert!(!pid_path.exists());
}

#[tokio::test]
async fn validated_orphan_reap_kills_captured_descendant_tree() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-10.pid");
    let runtime_path = dir.path().join("config-10.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let (handle, mut events) = Command::new(child())
        .args(["spawn-grandchild"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 10,
        }))
        .spawn()
        .await
        .unwrap();
    let root_pid = handle.pid();
    let grandchild_pid = loop {
        match events.recv().await.expect("parent event") {
            ProcessEvent::Stdout(line) => {
                if let Some(pid) = line.strip_prefix("grandchild-pid:") {
                    break pid.parse::<u32>().unwrap();
                }
            }
            ProcessEvent::Terminated(status) => panic!("parent exited early: {status:?}"),
            _ => {}
        }
    };
    assert!(pid_alive(root_pid));
    assert!(pid_alive(grandchild_pid));

    assert_eq!(
        reap_epoch_pid_file(&pid_path, dir.path()).await.unwrap(),
        OrphanReapOutcome::Killed
    );
    assert!(!pid_alive(root_pid), "recorded parent survived reaping");
    assert!(
        !pid_alive(grandchild_pid),
        "captured descendant survived reaping"
    );
}

/// Rewrites one field of a serialized epoch record.
fn replace_record_field(path: &std::path::Path, field: &str, value: &str) {
    let raw = std::fs::read_to_string(path).unwrap();
    let rewritten = raw
        .lines()
        .map(|line| match line.split_once('=') {
            Some((key, _)) if key == field => format!("{key}={value}"),
            _ => line.to_owned(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{rewritten}\n")).unwrap();
}

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

#[tokio::test]
async fn orphan_reap_retires_a_reused_pid_without_killing() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-4.pid");
    let runtime_path = dir.path().join("config-4.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let (handle, mut events) = Command::new(child())
        .args(["echo-stdin"])
        .pipe_stdin(true)
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 4,
        }))
        .spawn()
        .await
        .unwrap();
    // A record whose start token differs describes a process that has exited;
    // the live process now holding the pid is someone else's.
    let record = read_epoch_pid_file(&pid_path).await.unwrap().unwrap();
    replace_record_field(
        &pid_path,
        "start-token",
        &record.start_token.wrapping_add(1).to_string(),
    );

    assert_eq!(
        reap_epoch_pid_file(&pid_path, dir.path()).await.unwrap(),
        OrphanReapOutcome::AlreadyExited
    );
    assert!(!pid_path.exists(), "the stale record was kept");

    // The process still answers, so it was never signaled.
    handle.write_stdin(b"nonce\n").await.unwrap();
    loop {
        match events.recv().await.expect("child event") {
            ProcessEvent::Stdout(line) if line.trim_end() == "echo:nonce" => break,
            ProcessEvent::Terminated(status) => panic!("reused pid was killed: {status:?}"),
            _ => {}
        }
    }
}

#[tokio::test]
async fn orphan_reap_spares_the_tree_under_a_reused_pid() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-5.pid");
    let runtime_path = dir.path().join("config-5.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let (handle, mut events) = Command::new(child())
        .args(["spawn-grandchild"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 5,
        }))
        .spawn()
        .await
        .unwrap();
    let grandchild_pid = loop {
        match events.recv().await.expect("parent event") {
            ProcessEvent::Stdout(line) => {
                if let Some(pid) = line.strip_prefix("grandchild-pid:") {
                    break pid.trim().parse::<u32>().unwrap();
                }
            }
            ProcessEvent::Terminated(status) => panic!("parent exited early: {status:?}"),
            _ => {}
        }
    };
    let record = read_epoch_pid_file(&pid_path).await.unwrap().unwrap();
    replace_record_field(
        &pid_path,
        "start-token",
        &record.start_token.wrapping_add(1).to_string(),
    );

    assert_eq!(
        reap_epoch_pid_file(&pid_path, dir.path()).await.unwrap(),
        OrphanReapOutcome::AlreadyExited
    );
    assert!(pid_alive(handle.pid()), "reused root was killed");
    assert!(
        pid_alive(grandchild_pid),
        "descendant of a reused pid was killed"
    );

    let _ = handle.kill().await;
}

#[tokio::test]
async fn orphan_reap_keeps_a_record_whose_executable_differs() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core-6.pid");
    let runtime_path = dir.path().join("config-6.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();
    let (handle, _events) = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 6,
        }))
        .spawn()
        .await
        .unwrap();
    let pid = handle.pid();
    // Same start token, another name: neither ownership nor exit is proven.
    replace_record_field(&pid_path, "executable", &hex("another-core"));

    let error = reap_epoch_pid_file(&pid_path, dir.path())
        .await
        .expect_err("an executable mismatch must stay uncertain");
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(pid_path.exists(), "an uncertain record was removed");
    assert!(pid_alive(pid), "uncertain identity was killed");

    let _ = handle.kill().await;
}

#[tokio::test]
async fn epoch_pid_file_rejects_runtime_path_escape() {
    let pid_dir = tempfile::tempdir().unwrap();
    let config_dir = tempfile::tempdir().unwrap();
    let pid_path = pid_dir.path().join("core-3.pid");
    let runtime_path = config_dir.path().join("config-3.yaml");
    std::fs::write(&runtime_path, "mixed-port: 0\n").unwrap();

    let error = Command::new(child())
        .args(["sleep-forever"])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: &pid_path,
            runtime_config: &runtime_path,
            epoch: 3,
        }))
        .spawn()
        .await
        .err()
        .expect("path escape must fail before spawn");
    assert!(matches!(
        error,
        nyanpasu_utils::process::ProcessError::Io(error)
            if error.kind() == std::io::ErrorKind::InvalidInput
    ));
}

#[tokio::test]
async fn abandonment_cleans_up_pid_file() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core.pid");
    let (handle, rx) = Command::new(child())
        .args(["sleep-forever"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    let pid: u32 = std::fs::read_to_string(&pid_path)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    drop(handle);
    drop(rx);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while (pid_alive(pid) || pid_path.exists()) && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(!pid_alive(pid), "abandoned process still alive");
    assert!(
        !pid_path.exists(),
        "pid file still exists after abandonment"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_pid_file_is_rejected() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.pid");
    let pid_path = dir.path().join("core.pid");
    std::fs::write(&target, std::process::id().to_string()).unwrap();
    symlink(&target, &pid_path).unwrap();

    let error = Command::new(child())
        .args(["exit-with", "0"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .err()
        .expect("symlink pid file must fail spawn");
    assert!(matches!(
        error,
        nyanpasu_utils::process::ProcessError::Io(error)
            if error.kind() == std::io::ErrorKind::InvalidInput
    ));
}

#[tokio::test]
async fn unrelated_pid_is_not_killed_by_residual_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let pid_path = dir.path().join("core.pid");
    let own_pid = std::process::id();
    std::fs::write(&pid_path, own_pid.to_string()).unwrap();

    let (handle, mut rx) = Command::new(child())
        .args(["exit-with", "0"])
        .pid_file(&pid_path)
        .spawn()
        .await
        .unwrap();
    while rx.recv().await.is_some() {}

    assert_eq!(handle.wait().await.unwrap().code, Some(0));
    assert!(pid_alive(own_pid), "pid validator killed the test process");
}
