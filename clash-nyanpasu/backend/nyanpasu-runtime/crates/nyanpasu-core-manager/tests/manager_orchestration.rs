mod common;

use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use nyanpasu_core_manager::{
    CoreState, Error, HealthPolicy, HealthPolicySpec, HealthState, HealthThresholds,
    LocalIpcPolicy, LogLevel, LogStream, ManagerOptions, ProbeHandle, ProbeResult, StopReason,
    manager::CoreManager,
};

async fn manager(runtime_dir: &camino::Utf8Path) -> CoreManager {
    CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir.join("runtime")),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct manager")
}

#[tokio::test]
async fn manager_builder_forwards_atomic_runtime_health_without_bumping_lifecycle_time() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.health = HealthPolicy::new(HealthPolicySpec {
        interval: Duration::from_millis(20),
        timeout: Duration::from_secs(1),
        thresholds: HealthThresholds {
            failure: NonZeroU32::new(2).unwrap(),
            success: NonZeroU32::MIN,
        },
        start_period: Duration::ZERO,
    })
    .unwrap();
    let readiness_calls = Arc::new(AtomicUsize::new(0));
    let readiness = ProbeHandle::from_fn("manager-readiness", {
        let readiness_calls = readiness_calls.clone();
        move |context| {
            assert_eq!(context.epoch, common::epoch(1));
            readiness_calls.fetch_add(1, Ordering::SeqCst);
            async { ProbeResult::Healthy }
        }
    });
    let failing = Arc::new(AtomicBool::new(false));
    let liveness = ProbeHandle::from_fn("manager-liveness", {
        let failing = failing.clone();
        move |_| {
            let fail = failing.load(Ordering::SeqCst);
            async move {
                if fail {
                    ProbeResult::Unhealthy {
                        detail: Some("runtime API unavailable".into()),
                    }
                } else {
                    ProbeResult::Healthy
                }
            }
        }
    });
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        ..ManagerOptions::default()
    })
    .readiness_probe(readiness)
    .liveness_probe(liveness)
    .build()
    .await
    .unwrap();
    let mut status_rx = manager.subscribe();
    manager.start(spec).await.unwrap();
    assert!(readiness_calls.load(Ordering::SeqCst) >= 1);
    let running = manager.status();
    let running_changed_at = running.changed_at;
    let CoreState::Running { epoch, pid } = running.state else {
        panic!("manager did not publish running")
    };
    assert_eq!(epoch, common::epoch(1));
    assert_eq!(running.health.unwrap().state, HealthState::Healthy);

    failing.store(true, Ordering::SeqCst);
    let unhealthy = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            status_rx.changed().await.unwrap();
            let status = status_rx.borrow_and_update().clone();
            if status
                .health
                .as_ref()
                .is_some_and(|health| health.state == HealthState::Unhealthy)
            {
                break status;
            }
        }
    })
    .await
    .expect("manager did not forward unhealthy state");
    assert!(matches!(
        unhealthy.state,
        CoreState::Running {
            epoch,
            pid: current
        } if epoch == common::epoch(1) && current == pid
    ));
    assert_eq!(unhealthy.changed_at, running_changed_at);
    manager.stop().await.unwrap();
}

#[tokio::test]
async fn a_controller_template_without_epoch_is_rejected_at_construction() {
    let (_guard, dir) = common::utf8_tempdir();
    let options = ManagerOptions {
        runtime_dir: Some(dir),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: Some(r"\\.\pipe\nyanpasu\fixed".to_owned()),
        ..ManagerOptions::default()
    };

    assert!(CoreManager::new(options).await.is_err());
}

/// `runtime_dir` is the manager's only runtime artifact directory.
#[tokio::test]
async fn a_missing_runtime_dir_is_rejected_at_construction() {
    let error = CoreManager::new(ManagerOptions::default())
        .await
        .err()
        .expect("a manager without a runtime dir was accepted");
    assert!(
        matches!(error, Error::InvalidManagerOptions(ref detail) if detail.contains("runtime_dir")),
        "unexpected error: {error}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn managed_unix_controller_template_cannot_escape_runtime_directory() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    let escaped = dir.join("escaped-{epoch}.sock").to_string();
    let result = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: Some(escaped),
        ..ManagerOptions::default()
    })
    .await;
    let error = result.err().expect("escaped template was accepted");
    assert!(error.to_string().contains("escapes runtime directory"));
}

async fn wait_core_state(
    rx: &mut tokio::sync::watch::Receiver<nyanpasu_core_manager::CoreStatus>,
    pred: impl Fn(&CoreState) -> bool,
    timeout: Duration,
) -> CoreState {
    tokio::time::timeout(timeout, async {
        loop {
            let current = rx.borrow_and_update().state.clone();
            if pred(&current) {
                return current;
            }
            rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("timed out waiting for core state")
}

#[tokio::test]
async fn start_publishes_running_and_rejects_double_start() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let manager = manager(&dir).await;
    let mut rx = manager.subscribe();
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped { reason: None }
    ));

    manager.start(spec.clone()).await.expect("start");
    let state = wait_core_state(
        &mut rx,
        |s| matches!(s, CoreState::Running { .. }),
        Duration::from_secs(10),
    )
    .await;
    let CoreState::Running { epoch, pid } = state else {
        unreachable!()
    };
    assert!(epoch >= common::epoch(1) && pid > 0);
    let status = manager.status();
    assert_eq!(
        status.spec.as_ref().map(|s| s.config_path.clone()),
        Some(spec.config_path.clone())
    );
    assert!(status.changed_at > 0);

    let err = manager.start(spec).await.expect_err("double start");
    assert!(matches!(err, Error::AlreadyRunning), "got {err}");

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn log_subscription_observes_startup_frames() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  stdout-lines:\n    - 'time=\"2026-07-29T00:16:22.646059400+08:00\" level=info msg=\"startup frame\"'\n"
        ),
    );
    let manager = manager(&dir).await;
    // Subscribing before the first start must not miss the startup window.
    let mut logs = manager.subscribe_logs();
    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start");

    let frame = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match logs.recv().await {
                Ok(frame) if frame.message == "startup frame" => break frame,
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("log channel closed")
                }
            }
        }
    })
    .await
    .expect("startup frame was never published");

    assert_eq!(frame.epoch, 1);
    assert_eq!(frame.level, LogLevel::Info);
    assert_eq!(frame.stream, LogStream::Stdout);
    manager.shutdown().await.expect("shutdown");
}

/// The other half of the frame contract: what the broadcast delivers also
/// reaches disk. Reads through `log_dir()` rather than rebuilding the path,
/// because the store canonicalizes and Windows hands back a `\\?\` prefix.
#[tokio::test]
async fn core_logs_are_archived_under_the_manager_log_directory() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  stdout-lines:\n    - 'time=\"2026-07-29T00:16:22.646059400+08:00\" level=info msg=\"archived frame\"'\n"
        ),
    );
    let manager = manager(&dir).await;
    let log_dir = manager
        .log_dir()
        .expect("the sink is on by default")
        .to_owned();
    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start");

    // The sink is an independent subscriber, so it lands the frame slightly
    // after `subscribe_logs` would: poll rather than assume.
    let record = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(record) = read_archived(&log_dir, "archived frame") {
                break record;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("the frame was never archived");

    assert_eq!(record["t"], "log");
    assert_eq!(record["epoch"], 1);
    assert_eq!(record["kind"], "mihomo");
    assert_eq!(record["stream"], "stdout");
    assert_eq!(record["level"], "info");
    assert!(record["at"].as_i64().expect("at is always present") > 0);
    manager.shutdown().await.expect("shutdown");
}

fn read_archived(log_dir: &camino::Utf8Path, message: &str) -> Option<serde_json::Value> {
    std::fs::read_dir(log_dir)
        .ok()?
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .flat_map(|body| {
            body.lines()
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                .collect::<Vec<_>>()
        })
        .find(|record| record["message"] == message)
}

#[tokio::test]
async fn stop_requires_a_running_core() {
    let (_guard, dir) = common::utf8_tempdir();
    let manager = manager(&dir).await;
    assert!(matches!(manager.stop().await, Err(Error::NotStarted)));
}

#[tokio::test]
async fn failed_start_reports_error_and_publishes_stopped() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nx-fake-core:\n  never-ready: true\n"),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_secs(1);

    let manager = manager(&dir).await;
    let err = manager.start(spec).await.expect_err("must fail");
    assert!(matches!(err, Error::StartupTimeout { .. }), "got {err}");
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped { reason: Some(_) }
    ));
}

#[tokio::test]
async fn missing_controller_is_rejected_strictly() {
    let (_guard, dir) = common::utf8_tempdir();
    let config = common::write_config(&dir, "mixed-port: 7890\n");
    let spec = common::mihomo_spec(&dir, config);
    let manager = manager(&dir).await;
    assert!(matches!(
        manager.start(spec).await,
        Err(Error::ControllerMissing)
    ));
}

#[tokio::test]
async fn hard_switch_replaces_the_core_and_bumps_the_epoch() {
    let (_guard, dir) = common::utf8_tempdir();
    let port_a = common::free_port();
    let port_b = common::free_port();
    let config_a =
        common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port_a}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        format!("external-controller: 127.0.0.1:{port_b}\n"),
    )
    .unwrap();

    let manager = manager(&dir).await;
    let mut rx = manager.subscribe();
    let mut spec_a = common::mihomo_spec(&dir, config_a);
    spec_a.options.startup_timeout = Duration::from_secs(15);
    manager.start(spec_a).await.expect("start");
    let CoreState::Running { epoch: first, .. } = manager.status().state else {
        panic!("not running")
    };

    let mut spec_b = common::mihomo_spec(&dir, config_b);
    spec_b.config_path = dir.join("config-b.yaml");
    spec_b.options.startup_timeout = Duration::from_secs(15);
    manager.switch(spec_b).await.expect("switch");

    let state = wait_core_state(
        &mut rx,
        |s| matches!(s, CoreState::Running { .. }),
        Duration::from_secs(10),
    )
    .await;
    let CoreState::Running { epoch: second, .. } = state else {
        unreachable!()
    };
    assert!(second > first, "epoch must increase: {first} -> {second}");
    common::wait_port_refused(port_a).await; // old core is dead

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn prepare_failure_never_leaves_switching() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config_a = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(&config_b, "mixed-port: 7890\n").unwrap();

    let manager = manager(&dir).await;
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start");

    let error = manager
        .switch(common::mihomo_spec(&dir, config_b))
        .await
        .expect_err("missing controller must fail");
    assert!(matches!(error, Error::ControllerMissing), "got {error}");
    assert!(
        !matches!(manager.status().state, CoreState::Switching { .. }),
        "prepare failure must publish a terminal or retained state"
    );
}

#[tokio::test]
async fn restart_uses_the_last_spec_and_survives_stop() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));

    let manager = manager(&dir).await;
    assert!(matches!(manager.restart().await, Err(Error::NotStarted)));

    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_secs(15);
    manager.start(spec).await.expect("start");
    manager.stop().await.expect("stop");
    // Legacy parity: restart after stop starts the remembered spec again.
    manager.restart().await.expect("restart after stop");
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn switch_publishes_a_switching_window() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_secs(15);

    let manager = manager(&dir).await;
    manager.start(spec.clone()).await.expect("start");

    let mut rx = manager.subscribe();
    let seen = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    let seen_ = seen.clone();
    let recorder = tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            seen_.lock().push(rx.borrow_and_update().state.clone());
        }
    });

    manager.restart().await.expect("restart");
    recorder.abort();
    let states = seen.lock().clone();
    assert!(
        states
            .iter()
            .any(|s| matches!(s, CoreState::Switching { .. })),
        "sequence was {states:?}"
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn lifecycle_sequence_matches_legacy_contract() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 1500\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );

    let manager = manager(&dir).await;
    let mut rx = manager.subscribe();
    let seen = std::sync::Arc::new(parking_lot::Mutex::new(vec![rx.borrow().state.clone()]));
    let seen_ = seen.clone();
    let recorder = tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            seen_.lock().push(rx.borrow_and_update().state.clone());
        }
    });

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start");
    let first_pid = match manager.status().state {
        CoreState::Running { pid, .. } => pid,
        ref state => panic!("expected initial Running, got {state:?}"),
    };
    let mut recovery_rx = manager.subscribe();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if matches!(
                recovery_rx.borrow_and_update().state,
                CoreState::Running { pid, .. } if pid != first_pid
            ) {
                break;
            }
            recovery_rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("core never recovered from scripted crash");
    manager.stop().await.expect("stop");
    // Let the recorder drain the final Stopped notification before teardown — on the current-thread runtime the woken recorder task only runs once we yield.
    tokio::time::timeout(Duration::from_secs(2), async {
        while !seen.lock().iter().any(|s| {
            matches!(
                s,
                CoreState::Stopped {
                    reason: Some(StopReason::User)
                }
            )
        }) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("terminal user stop never reached the recorder");
    recorder.abort();

    let states = seen.lock().clone();
    let position = |pred: &dyn Fn(&CoreState) -> bool| states.iter().position(pred);
    let starting = position(&|s| matches!(s, CoreState::Starting { .. })).expect("Starting");
    let running = position(&|s| matches!(s, CoreState::Running { .. })).expect("Running");
    let restarting = position(&|s| matches!(s, CoreState::Restarting { .. })).expect("Restarting");
    let stopped = states
        .iter()
        .rposition(|s| {
            matches!(
                s,
                CoreState::Stopped {
                    reason: Some(StopReason::User)
                }
            )
        })
        .expect("terminal user stop");
    assert!(
        starting < running && running < restarting && restarting < stopped,
        "sequence was {states:?}"
    );
    let running_after_restart = states[restarting..]
        .iter()
        .any(|s| matches!(s, CoreState::Running { .. }));
    assert!(
        running_after_restart,
        "recovery must re-confirm Running: {states:?}"
    );
}

#[tokio::test]
async fn source_mutation_after_snapshot_does_not_affect_respawn() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 1500\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );
    let manager = manager(&dir).await;
    manager
        .start(common::mihomo_spec(&dir, config.clone()))
        .await
        .expect("start");

    let revision = manager.status().revision.expect("active revision");
    assert_eq!(revision.runtime_path.file_name(), Some("config-1.yaml"));
    assert_ne!(revision.runtime_path, config);
    assert!(runtime_dir.join("core-1.pid").exists());

    std::fs::write(
        &config,
        "external-controller: 127.0.0.1:1\nx-fake-core:\n  never-ready: true\n",
    )
    .unwrap();
    let mut rx = manager.subscribe();
    wait_core_state(
        &mut rx,
        |state| matches!(state, CoreState::Restarting { .. }),
        Duration::from_secs(5),
    )
    .await;
    wait_core_state(
        &mut rx,
        |state| matches!(state, CoreState::Running { .. }),
        Duration::from_secs(10),
    )
    .await;

    let runtime = std::fs::read_to_string(&revision.runtime_path).unwrap();
    assert!(runtime.contains(&format!("127.0.0.1:{port}")));
    assert!(!runtime.contains("never-ready"));
    manager.shutdown().await.expect("shutdown");
    assert!(!revision.runtime_path.exists());
    assert!(!runtime_dir.join("core-1.pid").exists());
}

#[tokio::test]
async fn manager_sweeps_stale_artifacts_and_advances_epoch() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::fs::write(runtime_dir.join("config-7.yaml"), "stale: true\n").unwrap();
    std::fs::write(runtime_dir.join("config-7.yaml.backup-3"), "stale backup\n").unwrap();
    std::fs::write(
        runtime_dir.join("core-9.pid.tmp-123-0"),
        "stale pid staging\n",
    )
    .unwrap();

    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir.clone()),
        ..ManagerOptions::default()
    })
    .await
    .expect("sweeping manager");
    assert!(!runtime_dir.join("config-7.yaml").exists());
    assert!(!runtime_dir.join("config-7.yaml.backup-3").exists());
    assert!(!runtime_dir.join("core-9.pid.tmp-123-0").exists());

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start after sweep");
    assert!(matches!(
        manager.status().state,
        CoreState::Running { epoch, .. } if epoch == common::epoch(10)
    ));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn dropping_live_manager_releases_runtime_ownership() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let options = || ManagerOptions {
        runtime_dir: Some(runtime_dir.clone()),
        ..ManagerOptions::default()
    };
    let manager = CoreManager::new(options()).await.expect("first manager");
    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start live core");
    drop(manager);

    let replacement = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match CoreManager::new(options()).await {
                Ok(manager) => break manager,
                Err(Error::RuntimeDirectoryOwned(_)) => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Err(error) => panic!("unexpected construction error: {error}"),
            }
        }
    })
    .await
    .expect("dropping the manager retained its runtime lock");
    replacement.shutdown().await.unwrap();
}

#[tokio::test]
async fn initial_start_stop_uncertainty_quarantines_until_recovery() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let port = common::free_port();
    let crash_state = dir.join("initial-crash-state");
    let launch_count = dir.join("initial-launch-count");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  never-ready: true\n  crash-after-ms: 3000\n  crash-times: 1\n  state-file: '{crash_state}'\n  launch-count-file: '{launch_count}'\n  fail-after-launches: 99\n"
        ),
    );
    let manager = std::sync::Arc::new(
        CoreManager::new(ManagerOptions {
            runtime_dir: Some(runtime_dir.clone()),
            stop_timeout: Duration::from_secs(1),
            ..ManagerOptions::default()
        })
        .await
        .unwrap(),
    );
    let start = {
        let manager = manager.clone();
        let mut spec = common::mihomo_spec(&dir, config.clone());
        spec.options.startup_timeout = Duration::from_secs(15);
        spec.options.restart_policy =
            nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 0 };
        tokio::spawn(async move { manager.start(spec).await })
    };
    let pid_path = runtime_dir.join("core-1.pid");
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let launched =
                std::fs::read_to_string(&launch_count).is_ok_and(|value| value.trim() == "1");
            if pid_path.exists() && launched {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("initial pid record never appeared");
    let record = std::fs::read_to_string(&pid_path).unwrap();
    std::fs::write(&pid_path, "corrupt record\n").unwrap();

    assert!(matches!(
        start.await.unwrap(),
        Err(Error::StopUnconfirmed(_))
    ));
    let start_error = manager
        .start(common::mihomo_spec(&dir, config.clone()))
        .await
        .expect_err("quarantine must reject another initial start");
    assert!(matches!(
        start_error,
        Error::ManagerQuarantined { epoch, .. } if epoch == common::epoch(1)
    ));

    std::fs::write(&pid_path, record).unwrap();
    common::wait_port_refused(port).await;
    let blocked_cleanup = runtime_dir.join("config-1.yaml.backup-blocked");
    std::fs::create_dir(&blocked_cleanup).unwrap();
    let quarantine_status = manager.status();
    manager
        .recover_quarantine()
        .await
        .expect_err("unsafe artifact must fail the first cleanup attempt");
    assert_eq!(manager.status().state, quarantine_status.state);
    std::fs::remove_dir(&blocked_cleanup).unwrap();
    manager.recover_quarantine().await.unwrap();
    let healthy = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    manager
        .start(common::mihomo_spec(&dir, healthy))
        .await
        .expect("start after initial quarantine recovery");
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn second_manager_cannot_take_over_an_owned_runtime_directory() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let options = || ManagerOptions {
        runtime_dir: Some(runtime_dir.clone()),
        ..ManagerOptions::default()
    };

    let first = CoreManager::new(options()).await.expect("first manager");
    first
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start first manager's core");
    let CoreState::Running { pid, .. } = first.status().state else {
        panic!("first manager is not running")
    };

    let second = CoreManager::new(options()).await;
    assert!(
        second.is_err(),
        "second manager acquired an owned directory"
    );
    let error = second.err().unwrap().to_string();
    assert!(error.contains("already owned"), "unexpected error: {error}");
    assert!(matches!(
        first.status().state,
        CoreState::Running { pid: live_pid, .. } if live_pid == pid
    ));
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("second construction must not kill the owned core");

    first.shutdown().await.expect("shutdown first manager");
}
