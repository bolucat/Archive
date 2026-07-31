mod common;

use std::time::Duration;

use nyanpasu_core_manager::{
    ControllerVersionProbe, CoreState, DegradeReason, Error, HealthProbe, LocalIpcPolicy,
    ManagerOptions, ProbeHandle, ProbeResult, StopReason, manager::CoreManager,
};

fn unique_template() -> Option<String> {
    #[cfg(windows)]
    {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        Some(format!(
            r"\\.\pipe\nyanpasu-test-{}-{n}-{{epoch}}",
            std::process::id()
        ))
    }
    #[cfg(not(windows))]
    {
        None // unix default derives the socket under the runtime dir (already unique)
    }
}

async fn local_ipc_manager(runtime_dir: camino::Utf8PathBuf) -> CoreManager {
    CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: unique_template(),
        ..Default::default()
    })
    .await
    .expect("construct manager")
}

#[tokio::test]
async fn local_ipc_start_injects_the_epoch_endpoint_and_advertises_it() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    // Stale artifacts from a "previous run" must be swept by CoreManager::new.
    std::fs::create_dir_all(&runtime_dir).unwrap();
    std::fs::write(runtime_dir.join("config-99.yaml"), "stale").unwrap();

    // No external-controller in the user config — Force resolves to local IPC, so the manager
    // injects the epoch-scoped endpoint.
    let config = common::write_config(&dir, "mixed-port: 0\n");
    let manager = local_ipc_manager(runtime_dir.clone()).await;
    assert!(
        !runtime_dir.join("config-99.yaml").exists(),
        "stale runtime config swept on construction"
    );

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("managed start");
    let status = manager.status();
    assert!(matches!(status.state, CoreState::Running { .. }));
    let controller = status.controller.expect("advertised managed endpoint");
    let endpoint = format!("{controller:?}");
    assert!(
        endpoint.contains('1'),
        "endpoint should embed the epoch: {endpoint}"
    );
    assert!(runtime_dir.join("config-100.yaml").exists());

    manager.shutdown().await.expect("shutdown");
    assert!(
        !runtime_dir.join("config-100.yaml").exists(),
        "runtime config removed after shutdown"
    );
    let _ = Duration::ZERO;
}

#[tokio::test]
async fn spawn_error_removes_the_secret_runtime_config() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let config = common::write_config(
        &dir,
        "mixed-port: 0\nsecret: test-secret\nx-fake-core:\n  check-fail: boom\n",
    );
    let manager = local_ipc_manager(runtime_dir.clone()).await;

    let error = manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect_err("config check must fail");
    assert!(matches!(error, Error::ConfigCheckFailed(_)), "got {error}");
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped {
            reason: Some(StopReason::Error(_))
        }
    ));
    assert!(
        !runtime_dir.join("config-1.yaml").exists(),
        "secret-bearing runtime config must be removed"
    );
}

#[tokio::test]
async fn stop_cleans_the_runtime_config_for_terminal_instance() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "mixed-port: 0\nx-fake-core:\n  crash-after-ms: 500\n  crash-times: 99\n  state-file: {state_file}\n"
        ),
    );
    let manager = local_ipc_manager(runtime_dir.clone()).await;
    let mut rx = manager.subscribe();

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = rx.borrow_and_update().state.clone();
            if matches!(
                state,
                CoreState::Stopped {
                    reason: Some(StopReason::Error(ref message))
                } if message.contains("restart budget exhausted")
            ) {
                break;
            }
            rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("core never exhausted its restart budget");

    assert!(
        runtime_dir.join("config-1.yaml").exists(),
        "runtime config must exist before terminal stop cleanup"
    );
    assert!(matches!(manager.stop().await, Err(Error::NotStarted)));
    assert!(
        !runtime_dir.join("config-1.yaml").exists(),
        "runtime config must be removed after terminal stop"
    );
    manager.shutdown().await.expect("shutdown");
}

use nyanpasu_core_manager::SwitchOutcome;
use parking_lot::Mutex;
use std::sync::Arc;

#[tokio::test]
async fn graceful_switch_overlaps_and_restores_listeners() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let patch_log_b = dir.join("patch-b.log");

    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(
        &config_b_path,
        format!("mixed-port: {mixed}\nx-fake-core:\n  patch-log: {patch_log_b}\n"),
    )
    .unwrap();

    let manager = local_ipc_manager(runtime_dir.clone()).await;
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");
    assert!(runtime_dir.join("config-1.yaml").exists());
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("A holds the mixed port");

    let mut rx = manager.subscribe();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_ = seen.clone();
    let recorder = tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            seen_.lock().push(rx.borrow_and_update().state.clone());
        }
    });

    let mut spec_b = common::mihomo_spec(&dir, config_b_path.clone());
    spec_b.config_path = config_b_path;
    let outcome = manager.switch(spec_b.clone()).await.expect("switch");
    assert_eq!(outcome, SwitchOutcome::Graceful);
    recorder.abort();

    // The user-visible overlap guarantee: never Stopped during the switch.
    let states = seen.lock().clone();
    assert!(
        states
            .iter()
            .any(|s| matches!(s, CoreState::Switching { .. })),
        "sequence was {states:?}"
    );
    assert!(
        !states
            .iter()
            .any(|s| matches!(s, CoreState::Stopped { .. })),
        "graceful switch must not publish Stopped: {states:?}"
    );

    // The new core received the original listener values via PATCH.
    let log = std::fs::read_to_string(&patch_log_b).expect("patch log");
    assert!(
        log.contains(&format!("\"mixed-port\":{mixed}")),
        "log: {log}"
    );
    // And rebound the port after the old core released it.
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("B serves the mixed port after the switch");

    let status = manager.status();
    let CoreState::Running { epoch, .. } = status.state else {
        panic!("not running after switch")
    };
    assert_eq!(epoch, 2);
    assert_eq!(
        status
            .spec
            .as_ref()
            .map(|summary| summary.config_path.clone()),
        Some(spec_b.config_path)
    );
    assert_eq!(
        status.revision.as_ref().map(|revision| revision.epoch),
        Some(2)
    );
    assert!(status.controller.is_some());
    assert!(
        !runtime_dir.join("config-1.yaml").exists(),
        "old runtime config must be removed after switch"
    );
    assert!(
        runtime_dir.join("config-2.yaml").exists(),
        "new runtime config must remain active"
    );
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn graceful_switch_surfaces_installed_but_uncertain_durability() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(&config_b, format!("mixed-port: {mixed}\n")).unwrap();
    let manager = local_ipc_manager(runtime_dir).await;
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start old core");
    manager.inject_runtime_parent_sync_failure_once_for_test();

    let outcome = manager
        .switch(common::mihomo_spec(&dir, config_b))
        .await
        .expect("graceful switch");

    let SwitchOutcome::DurabilityUncertain { outcome, warning } = outcome else {
        panic!("expected durability wrapper")
    };
    assert_eq!(*outcome, SwitchOutcome::Graceful);
    assert!(warning.contains("injected"), "{warning}");
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn graceful_respawn_loads_the_full_committed_runtime_config() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let ports = std::array::from_fn::<_, 5, _>(|_| common::free_port());
    let [port, socks, redir, tproxy, mixed] = ports;
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        format!(
            "port: {port}\nsocks-port: {socks}\nredir-port: {redir}\ntproxy-port: {tproxy}\nmixed-port: {mixed}\ntun:\n  enable: true\n"
        ),
    )
    .unwrap();

    let manager = local_ipc_manager(runtime_dir.clone()).await;
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start old core");
    manager
        .switch(common::mihomo_spec(&dir, config_b))
        .await
        .expect("graceful switch");

    let mut status_rx = manager.subscribe();
    let status = manager.status();
    let CoreState::Running {
        epoch,
        pid: first_pid,
    } = status.state
    else {
        panic!("new core is not running")
    };
    assert_eq!(epoch, 2);
    nyanpasu_utils::os::kill_pid::<String>(first_pid, None)
        .await
        .expect("kill new core process");

    let second_pid = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = status_rx.borrow_and_update().state.clone();
            if let CoreState::Running { epoch: 2, pid } = state
                && pid != first_pid
            {
                break pid;
            }
            status_rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("new core did not respawn");
    assert_ne!(second_pid, first_pid);

    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("respawn restored mixed-port");
    let client = clash_api::Client::builder(manager.status().controller.unwrap())
        .build()
        .unwrap();
    let runtime = client.configs().await.expect("GET respawned config");
    assert_eq!(runtime.port, i64::from(port));
    assert_eq!(runtime.socks_port, i64::from(socks));
    assert_eq!(runtime.redir_port, i64::from(redir));
    assert_eq!(runtime.tproxy_port, i64::from(tproxy));
    assert_eq!(runtime.mixed_port, i64::from(mixed));
    assert!(runtime.tun.enable, "respawn must restore TUN enablement");

    let runtime_file = std::fs::read_to_string(runtime_dir.join("config-2.yaml")).unwrap();
    assert!(runtime_file.contains(&format!("port: {port}")));
    assert!(runtime_file.contains(&format!("socks-port: {socks}")));
    assert!(runtime_file.contains(&format!("redir-port: {redir}")));
    assert!(runtime_file.contains(&format!("tproxy-port: {tproxy}")));
    assert!(runtime_file.contains(&format!("mixed-port: {mixed}")));
    assert!(runtime_file.contains("enable: true"));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn graceful_success_publishes_only_the_new_epoch_context() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(&config_b, format!("mixed-port: {mixed}\n")).unwrap();
    let manager = local_ipc_manager(runtime_dir).await;
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start old core");
    let old_controller = manager.status().controller.expect("old controller");
    let mut status_rx = manager.subscribe();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_ = observed.clone();
    let recorder = tokio::spawn(async move {
        while status_rx.changed().await.is_ok() {
            observed_.lock().push(status_rx.borrow_and_update().clone());
        }
    });

    let spec_b = common::mihomo_spec(&dir, config_b.clone());
    manager
        .switch(spec_b.clone())
        .await
        .expect("graceful switch");
    recorder.abort();
    let _ = recorder.await;

    let status = manager.status();
    let new_controller = status.controller.clone().expect("new controller");
    assert_ne!(format!("{new_controller:?}"), format!("{old_controller:?}"));
    assert_eq!(
        status.spec.as_ref().map(|spec| spec.config_path.clone()),
        Some(spec_b.config_path)
    );
    assert_eq!(
        status.revision.as_ref().map(|revision| revision.epoch),
        Some(2)
    );
    assert!(matches!(status.state, CoreState::Running { epoch: 2, .. }));
    for status in observed.lock().iter() {
        let published_epoch = match status.state {
            CoreState::Running { epoch, .. }
            | CoreState::Starting { epoch }
            | CoreState::Restarting { epoch, .. }
            | CoreState::Stopping { epoch } => Some(epoch),
            CoreState::Switching { to, .. } => Some(to),
            CoreState::Stopped { .. } => None,
            _ => None,
        };
        if published_epoch == Some(2) {
            assert_eq!(status.controller.as_ref(), Some(&new_controller));
            assert_eq!(
                status.revision.as_ref().map(|revision| revision.epoch),
                Some(2)
            );
        }
    }
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn graceful_patch_timeout_with_matching_get_is_success() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        format!("mixed-port: {mixed}\nx-fake-core:\n  patch-delay-ms: 250\n"),
    )
    .unwrap();
    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: unique_template(),
        control_timeout: Duration::from_millis(50),
        reconcile_timeout: Duration::from_secs(3),
        ..Default::default()
    })
    .await
    .expect("construct manager");
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start old core");

    let CoreState::Running {
        pid: before_pid, ..
    } = manager.status().state
    else {
        panic!("old core is not running")
    };
    let outcome = manager
        .switch(common::mihomo_spec(&dir, config_b))
        .await
        .expect("switch converges");

    assert_eq!(outcome, SwitchOutcome::Graceful);
    let CoreState::Running { epoch, pid } = manager.status().state else {
        panic!("new core is not running")
    };
    assert_eq!(epoch, 2);
    assert_ne!(pid, before_pid);
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("verified timed-out patch restored listener");
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn graceful_overlap_keeps_both_epoch_pid_records_without_miskilling_old() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        format!("mixed-port: {mixed}\nx-fake-core:\n  ready-delay-ms: 5000\n"),
    )
    .unwrap();
    let manager = Arc::new(local_ipc_manager(runtime_dir.clone()).await);
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start old core");
    let CoreState::Running { pid: old_pid, .. } = manager.status().state else {
        panic!("old core is not running")
    };

    let switching = {
        let manager = manager.clone();
        let mut spec_b = common::mihomo_spec(&dir, config_b);
        spec_b.options.startup_timeout = Duration::from_secs(15);
        tokio::spawn(async move { manager.switch(spec_b).await })
    };
    let (old_record, new_record) = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let old = nyanpasu_utils::process::read_epoch_pid_file(
                runtime_dir.join("core-1.pid").as_std_path(),
            )
            .await
            .ok()
            .flatten();
            let new = nyanpasu_utils::process::read_epoch_pid_file(
                runtime_dir.join("core-2.pid").as_std_path(),
            )
            .await
            .ok()
            .flatten();
            if let (Some(old), Some(new)) = (old, new) {
                break (old, new);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("epoch pid files never overlapped");
    assert_eq!(old_record.pid, old_pid);
    assert_ne!(old_record.pid, new_record.pid);
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("old core still serves during overlap");

    assert_eq!(switching.await.unwrap().unwrap(), SwitchOutcome::Graceful);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn graceful_overlap_removes_shared_http_controller_from_both_epochs() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let controller = common::free_port();
    let shared_controller = format!("127.0.0.1:{controller}");
    let config_a = common::write_config(
        &dir,
        &format!("mixed-port: {mixed}\nexternal-controller: {shared_controller}\nsecret: shared\n"),
    );
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        format!(
            "mixed-port: {mixed}\nexternal-controller: {shared_controller}\nsecret: shared\nx-fake-core:\n  ready-delay-ms: 2000\n"
        ),
    )
    .unwrap();
    let manager = Arc::new(local_ipc_manager(runtime_dir.clone()).await);
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start old core");

    let switching = {
        let manager = manager.clone();
        let mut spec_b = common::mihomo_spec(&dir, config_b);
        spec_b.options.startup_timeout = Duration::from_secs(15);
        tokio::spawn(async move { manager.switch(spec_b).await })
    };
    let (old_record, new_record) = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let old = nyanpasu_utils::process::read_epoch_pid_file(
                runtime_dir.join("core-1.pid").as_std_path(),
            )
            .await
            .ok()
            .flatten();
            let new = nyanpasu_utils::process::read_epoch_pid_file(
                runtime_dir.join("core-2.pid").as_std_path(),
            )
            .await
            .ok()
            .flatten();
            if let (Some(old), Some(new)) = (old, new) {
                break (old, new);
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("epoch pid files never overlapped");
    assert_ne!(old_record.pid, new_record.pid);

    for epoch in [1, 2] {
        let runtime = std::fs::read_to_string(runtime_dir.join(format!("config-{epoch}.yaml")))
            .expect("read overlapping effective config");
        let document: serde_yaml_ng::Mapping =
            serde_yaml_ng::from_str(&runtime).expect("parse effective config");
        assert!(
            !document.contains_key(serde_yaml_ng::Value::String("external-controller".into())),
            "epoch {epoch} retained the shared HTTP controller"
        );
        assert_eq!(
            document
                .get(serde_yaml_ng::Value::String("secret".into()))
                .and_then(serde_yaml_ng::Value::as_str),
            Some("shared"),
            "epoch {epoch} did not retain the source secret"
        );
    }

    assert_eq!(switching.await.unwrap().unwrap(), SwitchOutcome::Graceful);
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn quarantine_recovery_continues_after_an_independent_epoch_failure() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let config_a = common::write_config(&dir, "mixed-port: 0\n");
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        "mixed-port: 0\nx-fake-core:\n  ready-delay-ms: 5000\n",
    )
    .unwrap();
    let manager = Arc::new(
        CoreManager::new(ManagerOptions {
            runtime_dir: Some(runtime_dir.clone()),
            local_ipc_policy: LocalIpcPolicy::Force,
            controller_template: unique_template(),
            stop_timeout: Duration::from_secs(1),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .unwrap();

    let mut spec_b = common::mihomo_spec(&dir, config_b);
    spec_b.options.startup_timeout = Duration::from_secs(15);
    let switching = {
        let manager = manager.clone();
        tokio::spawn(async move { manager.switch(spec_b).await })
    };
    let first_pid = runtime_dir.join("core-1.pid");
    let second_pid = runtime_dir.join("core-2.pid");
    tokio::time::timeout(Duration::from_secs(15), async {
        while !second_pid.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("new epoch record never appeared");
    let first_record = std::fs::read_to_string(&first_pid).unwrap();
    let second_record = std::fs::read_to_string(&second_pid).unwrap();
    std::fs::write(&first_pid, "unverifiable first epoch\n").unwrap();
    std::fs::write(&second_pid, "unverifiable second epoch\n").unwrap();
    assert!(matches!(
        switching.await.unwrap(),
        Err(Error::StopUnconfirmed(_))
    ));

    std::fs::write(&second_pid, second_record).unwrap();
    manager
        .recover_quarantine()
        .await
        .expect_err("first epoch must remain uncertain");
    assert!(
        !runtime_dir.join("config-2.yaml").exists(),
        "a later independent quarantine was not recovered"
    );
    assert!(!second_pid.exists());
    assert!(runtime_dir.join("config-1.yaml").exists());

    std::fs::write(&first_pid, first_record).unwrap();
    manager.recover_quarantine().await.unwrap();
    assert!(!runtime_dir.join("config-1.yaml").exists());
}

#[tokio::test]
async fn hard_switch_removes_the_old_runtime_config() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let config_a = common::write_config(&dir, "mixed-port: 0\n");
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(&config_b_path, "dns:\n  listen: 127.0.0.1:0\n").unwrap();
    let manager = local_ipc_manager(runtime_dir.clone()).await;

    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");
    assert!(runtime_dir.join("config-1.yaml").exists());

    let outcome = manager
        .switch(common::mihomo_spec(&dir, config_b_path))
        .await
        .expect("hard switch");
    assert_eq!(
        outcome,
        SwitchOutcome::Hard {
            reason: DegradeReason::DnsListen
        }
    );
    assert!(
        !runtime_dir.join("config-1.yaml").exists(),
        "old runtime config must be removed after hard switch"
    );
    assert!(runtime_dir.join("config-2.yaml").exists());
    assert!(matches!(
        manager.status().state,
        CoreState::Running { epoch: 2, .. }
    ));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn prefer_http_fallback_degrades_switch_to_hard() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let port = common::free_port();
    let config_a = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(
        &config_b,
        format!("external-controller: 127.0.0.1:{port}\nmode: direct\n"),
    )
    .unwrap();
    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir),
        local_ipc_policy: LocalIpcPolicy::Prefer,
        controller_template: unique_template(),
        ..ManagerOptions::default()
    })
    .await
    .unwrap();
    let mut spec_a = common::mihomo_spec(&dir, config_a);
    let mut spec_b = common::mihomo_spec(&dir, config_b);
    #[cfg(windows)]
    let unsupported = "1.18.8";
    #[cfg(not(windows))]
    let unsupported = "1.18.3";
    spec_a.core.version = Some(unsupported.into());
    spec_b.core.version = Some(unsupported.into());
    manager.start(spec_a).await.unwrap();

    let outcome = manager.switch(spec_b).await.unwrap();

    assert_eq!(
        outcome,
        SwitchOutcome::Hard {
            reason: DegradeReason::HttpController
        }
    );
    assert!(matches!(
        manager.status().state,
        CoreState::Running { epoch: 2, .. }
    ));
    assert!(matches!(
        manager.status().controller,
        Some(nyanpasu_core_manager::Host::Http(_))
    ));
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn derive_failure_republishes_old_running_state() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(&config_b_path, format!("mixed-port: {mixed}\n1: invalid\n")).unwrap();
    let manager = local_ipc_manager(runtime_dir.clone()).await;

    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");
    let CoreState::Running {
        epoch: old_epoch,
        pid: old_pid,
    } = manager.status().state
    else {
        panic!("not running")
    };

    let error = manager
        .switch(common::mihomo_spec(&dir, config_b_path))
        .await
        .expect_err("derive must fail");
    assert!(matches!(error, Error::InvalidConfig(_)), "got {error}");
    assert_eq!(
        manager.status().state,
        CoreState::Running {
            epoch: old_epoch,
            pid: old_pid
        }
    );
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("old core still holds its port");
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn failed_new_core_while_old_restarting_republishes_actual_state() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let state_file = dir.join("crash-state");
    let config_a = common::write_config(
        &dir,
        &format!(
            "mixed-port: 0\nx-fake-core:\n  crash-after-ms: 400\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(&config_b_path, "mixed-port: 0\n").unwrap();
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: unique_template(),
        cancel_token: cancel_token.clone(),
        ..Default::default()
    })
    .await
    .expect("construct manager");
    let mut spec_a = common::mihomo_spec(&dir, config_a);
    spec_a.options.backoff = nyanpasu_utils::process::Backoff::exponential(
        Duration::from_secs(60),
        Duration::from_secs(60),
    );
    let mut rx = manager.subscribe();

    manager.start(spec_a).await.expect("start A");
    let (old_epoch, old_attempt) = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = rx.borrow_and_update().state.clone();
            if let CoreState::Restarting { epoch, attempt } = state {
                break (epoch, attempt);
            }
            rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("old core never entered restarting");

    let missing_binary = dir.join("missing-core");
    let mut spec_b = common::mihomo_spec(&dir, config_b_path);
    spec_b.core.binary_path = missing_binary.clone();
    let error = manager
        .switch(spec_b)
        .await
        .expect_err("new core spawn must fail");
    assert!(
        matches!(&error, Error::BinaryNotFound(path) if path == &missing_binary),
        "got {error}"
    );
    assert_eq!(
        manager.status().state,
        CoreState::Restarting {
            epoch: old_epoch,
            attempt: old_attempt
        }
    );

    cancel_token.cancel();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = rx.borrow_and_update().state.clone();
            if matches!(state, CoreState::Stopped { .. }) {
                break;
            }
            rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("replacement forwarder did not publish terminal state");
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn failed_new_core_rolls_back_without_touching_the_old_one() {
    let (_guard, dir) = common::utf8_tempdir();
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(&config_b_path, "x-fake-core:\n  never-ready: true\n").unwrap();

    let manager = local_ipc_manager(dir.join("runtime")).await;
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");
    let CoreState::Running {
        epoch: old_epoch, ..
    } = manager.status().state
    else {
        panic!("not running")
    };

    let mut spec_b = common::mihomo_spec(&dir, config_b_path.clone());
    spec_b.config_path = config_b_path;
    spec_b.options.startup_timeout = Duration::from_secs(1);
    manager.switch(spec_b).await.expect_err("switch must fail");

    // The old core is untouched and republished as Running.
    let CoreState::Running { epoch, .. } = manager.status().state else {
        panic!(
            "old core must still be running, got {:?}",
            manager.status().state
        )
    };
    assert_eq!(epoch, old_epoch);
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("old core still holds its port");
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn rejected_patch_falls_back_to_a_hard_restart() {
    let (_guard, dir) = common::utf8_tempdir();
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(
        &config_b_path,
        format!("mixed-port: {mixed}\nx-fake-core:\n  reject-patch: true\n"),
    )
    .unwrap();

    let attempts = Arc::new(Mutex::new(Vec::<(u64, u32)>::new()));
    let readiness = ProbeHandle::from_fn("graceful-recorded-readiness", {
        let attempts = attempts.clone();
        move |context| {
            attempts.lock().push((context.epoch, context.pid));
            async move {
                match ControllerVersionProbe::new(context.controller.as_ref()) {
                    Ok(probe) => probe.check(context).await,
                    Err(error) => ProbeResult::Unhealthy {
                        detail: Some(error.to_string()),
                    },
                }
            }
        }
    });
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: unique_template(),
        ..ManagerOptions::default()
    })
    .readiness_probe(readiness)
    .build()
    .await
    .unwrap();
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");

    let mut spec_b = common::mihomo_spec(&dir, config_b_path.clone());
    spec_b.config_path = config_b_path;
    let outcome = manager.switch(spec_b).await.expect("switch converges");
    assert_eq!(
        outcome,
        SwitchOutcome::Hard {
            reason: nyanpasu_core_manager::DegradeReason::PatchFailed
        }
    );
    // The fallback instance boots on the FULL config, so it binds the port itself.
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("fallback core serves the mixed port");
    let (initial_pids, candidate_pids): (
        std::collections::HashSet<_>,
        std::collections::HashSet<_>,
    ) = {
        let attempts = attempts.lock();
        (
            attempts
                .iter()
                .filter_map(|(epoch, pid)| (*epoch == 1).then_some(*pid))
                .collect(),
            attempts
                .iter()
                .filter_map(|(epoch, pid)| (*epoch == 2).then_some(*pid))
                .collect(),
        )
    };
    assert!(!initial_pids.is_empty(), "initial start missed probe plan");
    assert!(
        candidate_pids.len() >= 2,
        "graceful bootstrap and fallback did not both inherit probe plan"
    );
    manager.shutdown().await.expect("shutdown");
}
