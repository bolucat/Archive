mod common;

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    CoreManager, CoreState, Error, ManagerOptions, RuntimeConfigStore, StopReason,
};
use nyanpasu_utils::process::{
    Command, EpochPidFile, EpochPidFileSpec, ProcessEvent, ProcessHandle, recorded_process_is_alive,
};

async fn orphan(dir: &Utf8Path, epoch: u64) -> (ProcessHandle, String) {
    std::fs::create_dir_all(dir).unwrap();
    let config = dir.join(format!("config-{epoch}.yaml"));
    let pid = dir.join(format!("core-{epoch}.pid"));
    std::fs::write(
        &config,
        "mixed-port: 0\nx-fake-core:\n  stdout-lines:\n    - orphan-ready\n",
    )
    .unwrap();
    let (handle, mut events) = Command::new(common::fake_core_bin())
        .args(["-f", config.as_str()])
        .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
            pid_path: pid.as_std_path(),
            runtime_config: config.as_std_path(),
            epoch,
        }))
        .spawn()
        .await
        .unwrap();
    // The sweep may delete this config. Observe the child after it has read it.
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.recv().await.expect("orphan readiness event") {
                ProcessEvent::Stdout(line) if line == "orphan-ready" => break,
                ProcessEvent::Terminated(status) => {
                    panic!("orphan exited before ready: {status:?}")
                }
                _ => {}
            }
        }
    })
    .await
    .expect("orphan read its config");
    let raw = std::fs::read_to_string(pid).unwrap();
    (handle, raw)
}

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn replace_field(raw: &str, field: &str, value: &str) -> String {
    raw.lines()
        .map(|line| {
            if line.split_once('=').is_some_and(|(key, _)| key == field) {
                format!("{field}={value}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}

fn epoch_extras(dir: &Utf8Path, epoch: u64) -> Vec<Utf8PathBuf> {
    let mut paths = [
        format!("config-{epoch}.yaml.backup-1"),
        format!(".config-{epoch}.yaml.tmp-1"),
        format!("core-{epoch}.pid.tmp-1"),
    ]
    .map(|name| dir.join(name))
    .to_vec();
    for path in &paths {
        std::fs::write(path, "retained artifact").unwrap();
    }
    let socket = dir.join(format!("core-{epoch}.sock"));
    #[cfg(unix)]
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    #[cfg(windows)]
    std::fs::write(&socket, "socket placeholder").unwrap();
    paths.push(socket);
    paths
}

fn corrupt_records(dir: &Utf8Path) -> Vec<Utf8PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| Utf8PathBuf::from_path_buf(entry.unwrap().path()).unwrap())
        .filter(|path| path.file_name().unwrap().contains(".pid.corrupt-"))
        .collect()
}

#[tokio::test]
async fn unusable_startup_records_are_preserved_without_killing_and_never_rediscovered() {
    for case in ["garbage", "version", "missing-parent", "zero"] {
        let (_guard, dir) = common::utf8_tempdir();
        let runtime = dir.join("runtime");
        let epoch = if case == "zero" { 0 } else { 7 };
        let (child, original) = orphan(&runtime, epoch).await;
        let pid_path = runtime.join(format!("core-{epoch}.pid"));
        let bad = match case {
            "garbage" => "garbage".to_owned(),
            "version" => replace_field(&original, "version", "3"),
            "missing-parent" => replace_field(
                &original,
                "runtime-config",
                &hex(runtime.join("missing/config-7.yaml").as_str()),
            ),
            "zero" => original.clone(),
            _ => unreachable!(),
        };
        std::fs::write(&pid_path, &bad).unwrap();
        let extras = epoch_extras(&runtime, epoch);
        let options = ManagerOptions {
            runtime_dir: Some(runtime.clone()),
            ..Default::default()
        };
        let manager = CoreManager::new(options.clone()).await.unwrap();
        assert!(recorded_process_is_alive(&original).unwrap(), "{case}");
        assert!(!pid_path.exists());
        assert!(!runtime.join(format!("config-{epoch}.yaml")).exists());
        assert!(extras.iter().all(|path| !path.exists()));
        let renamed = corrupt_records(&runtime);
        assert_eq!(renamed.len(), 1);
        assert_eq!(std::fs::read_to_string(&renamed[0]).unwrap(), bad);
        let suffix = renamed[0]
            .file_name()
            .unwrap()
            .split_once(".corrupt-")
            .unwrap()
            .1;
        chrono::NaiveDateTime::parse_from_str(suffix, "%Y%m%dT%H%M%SZ").unwrap();
        let store = RuntimeConfigStore::new(runtime.clone()).await.unwrap();
        assert!(store.artifact_epochs().await.unwrap().is_empty());
        store
            .cleanup_epoch(common::epoch(epoch.max(1)))
            .await
            .unwrap();
        assert!(renamed[0].exists());
        manager.shutdown().await.unwrap();
        drop(manager);
        let second = CoreManager::new(options).await.unwrap();
        assert_eq!(corrupt_records(&runtime), renamed);
        assert!(recorded_process_is_alive(&original).unwrap());
        second.shutdown().await.unwrap();
        child.kill().await.unwrap();
        child.wait().await.unwrap();
    }
}

#[tokio::test]
async fn startup_quarantine_is_first_status_blocks_reconcile_and_can_recover() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    let (child, original) = orphan(&runtime, 7).await;
    let pid_path = runtime.join("core-7.pid");
    let uncertain = replace_field(&original, "executable", &hex("unproven-core"));
    std::fs::write(&pid_path, &uncertain).unwrap();
    let extras = epoch_extras(&runtime, 7);
    std::fs::write(runtime.join("config-8.yaml"), "stale").unwrap();
    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime.clone()),
        ..Default::default()
    })
    .await
    .unwrap();
    let first = manager.subscribe().borrow().clone();
    let CoreState::Stopped {
        reason: Some(StopReason::Error(reason)),
    } = first.state
    else {
        panic!("first observable status lacks the quarantine reason")
    };
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{}\n", common::free_port()),
    );
    let spec = common::mihomo_spec(&dir, config);
    let error = manager.reconcile(spec.clone(), None).await.unwrap_err();
    assert!(matches!(&error, Error::ManagerQuarantined { epoch, .. } if epoch.get() == 7));
    assert_eq!(reason, error.to_string());
    assert_eq!(std::fs::read_to_string(&pid_path).unwrap(), uncertain);
    assert!(runtime.join("config-7.yaml").exists());
    assert!(extras.iter().all(|path| path.exists()));
    assert!(
        !runtime.join("config-8.yaml").exists(),
        "sweep must continue past quarantine"
    );
    assert!(recorded_process_is_alive(&original).unwrap());
    assert!(matches!(
        manager.recover_quarantine().await,
        Err(Error::ManagerQuarantined { .. })
    ));
    assert!(pid_path.exists());
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    manager.recover_quarantine().await.unwrap();
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped { reason: None }
    ));
    assert!(!pid_path.exists());
    assert!(extras.iter().all(|path| !path.exists()));
    manager.reconcile(spec, None).await.unwrap();
    assert!(matches!(manager.status().state, CoreState::Running { epoch, .. } if epoch.get() > 8));
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_log_directory_disables_disk_but_delivers_core_logs() {
    for symlink in [false, true] {
        // Unix supports unprivileged symlink creation. Windows symlinks require
        // a privilege or Developer Mode, so the portable Windows case uses a file.
        if symlink && cfg!(windows) {
            continue;
        }
        let (_guard, dir) = common::utf8_tempdir();
        let runtime = dir.join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        let logs_path = runtime.join("logs");
        if symlink {
            #[cfg(unix)]
            std::os::unix::fs::symlink(&dir, &logs_path).unwrap();
        } else {
            std::fs::write(&logs_path, "keep this evidence").unwrap();
        }
        let manager = CoreManager::new(ManagerOptions {
            runtime_dir: Some(runtime.clone()),
            ..Default::default()
        })
        .await
        .unwrap();
        assert!(manager.log_dir().is_none());
        let mut logs = manager.subscribe_logs();
        let config = common::write_config(
            &dir,
            &format!(
                "external-controller: 127.0.0.1:{}\nx-fake-core:\n  stdout-lines:\n    - 'time=\"2026-10-29T00:00:00Z\" level=info msg=\"memory-only frame\"'\n",
                common::free_port()
            ),
        );
        manager
            .start(common::mihomo_spec(&dir, config))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let frame = logs.recv().await.unwrap();
                if frame.message == "memory-only frame" {
                    break;
                }
            }
        })
        .await
        .expect("memory log stream remains available");
        manager.shutdown().await.unwrap();
        if symlink {
            assert!(logs_path.as_std_path().is_symlink());
        } else {
            assert_eq!(
                std::fs::read_to_string(&logs_path).unwrap(),
                "keep this evidence"
            );
        }
    }
}

#[tokio::test]
async fn unreadable_startup_pid_record_is_fatal_and_preserved() {
    #[cfg(unix)]
    // SAFETY: geteuid takes no pointers and has no preconditions.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipped: root bypasses Unix file permissions");
        return;
    }
    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    let pid = runtime.join("core-7.pid");
    std::fs::write(&pid, "unreadable data").unwrap();
    #[cfg(windows)]
    let account = {
        let output = std::process::Command::new("whoami").output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    #[cfg(windows)]
    {
        let deny = std::process::Command::new("icacls")
            .arg(&pid)
            .args(["/deny", &format!("{account}:(R)")])
            .output()
            .unwrap();
        assert!(deny.status.success(), "{deny:?}");
    }
    #[cfg(unix)]
    let original = {
        use std::os::unix::fs::PermissionsExt;
        let original = std::fs::metadata(&pid).unwrap().permissions();
        std::fs::set_permissions(&pid, std::fs::Permissions::from_mode(0o000)).unwrap();
        original
    };
    let result = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime.clone()),
        ..Default::default()
    })
    .await;
    #[cfg(windows)]
    {
        let restore = std::process::Command::new("icacls")
            .arg(&pid)
            .args(["/remove:d", &account])
            .output()
            .unwrap();
        assert!(restore.status.success(), "{restore:?}");
    }
    #[cfg(unix)]
    std::fs::set_permissions(&pid, original).unwrap();
    assert!(
        matches!(result, Err(Error::OrphanReap(nyanpasu_utils::process::OrphanReapError::Io { source, .. }))
        if source.kind() == std::io::ErrorKind::PermissionDenied)
    );
    assert!(pid.is_file());
    assert!(corrupt_records(&runtime).is_empty());
}

#[cfg(windows)]
#[tokio::test]
async fn corrupt_record_rename_failure_aborts_before_artifact_cleanup() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    let pid = runtime.join("core-7.pid");
    let config = runtime.join("config-7.yaml");
    std::fs::write(&pid, "garbage").unwrap();
    std::fs::write(&config, "keep until evidence is preserved").unwrap();
    // Reading is allowed; renaming is denied while this handle is open.
    let held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0)
        .open(&pid)
        .unwrap();
    let result = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime.clone()),
        ..Default::default()
    })
    .await;
    drop(held);
    assert!(matches!(result, Err(Error::Io(source)) if source.raw_os_error() == Some(32)));
    assert_eq!(std::fs::read_to_string(&pid).unwrap(), "garbage");
    assert!(config.exists());
    assert!(corrupt_records(&runtime).is_empty());
}
