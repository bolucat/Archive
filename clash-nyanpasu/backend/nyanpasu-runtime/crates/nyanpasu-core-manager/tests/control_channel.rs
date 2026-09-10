mod common;

use nyanpasu_core_manager::{
    ConfigInput, ControlOptions, CoreCommand, CoreCommandEnvelope, CoreControl, CoreErrorKind,
    CoreManager, Host, LocalIpcPolicy, LocalIpcSettings, ManagerOptions, OperationId,
    ReconcileRequest,
};
use std::sync::Arc;

#[tokio::test]
async fn channel_changes_are_atomic_and_report_the_effective_configuration() {
    channel_matrix(None).await;
}

#[tokio::test]
#[ignore = "requires a real mihomo binary"]
async fn real_mihomo_channel_changes() {
    channel_matrix(Some(&common::real::MIHOMO)).await;
}

#[tokio::test]
#[ignore = "requires a real clash-rs binary"]
async fn real_clash_rs_channel_changes() {
    channel_matrix(Some(&common::real::CLASH_RS)).await;
}

async fn channel_matrix(core: Option<&common::real::RealCore>) {
    let (_guard, dir) = common::utf8_tempdir();
    let controller_dir = dir.join("controllers");
    std::fs::create_dir_all(&controller_dir).unwrap();
    let port = common::free_port();
    let bytes = format!("external-controller: 127.0.0.1:{port}\nsecret: private\n").into_bytes();
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        controller_dir: Some(controller_dir.clone()),
        ..ManagerOptions::default()
    })
    .build()
    .await
    .unwrap();
    let mut rx = manager.subscribe_config_commits();
    let control = CoreControl::spawn(
        manager,
        ControlOptions::new(dir.join("sources"), dir.clone()),
    );
    let spec = if let Some(core) = core {
        common::real::real_spec(core, &dir, dir.join("unused"))
    } else {
        let mut spec = common::mihomo_spec(&dir, dir.join("unused"));
        spec.core.version = Some("1.19.0".into());
        spec
    };
    let envelope = |policy, keep_http| {
        let mut options = spec.options.clone();
        options.local_ipc = Some(LocalIpcSettings {
            policy,
            keep_http_controller: keep_http,
        });
        CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Reconcile(Box::new(ReconcileRequest {
                core: spec.core.clone(),
                config: ConfigInput::inline(bytes.clone()),
                options,
                expected_applied: None,
            })),
        }
    };
    let first = envelope(LocalIpcPolicy::Prefer, true);
    control.submit(first.clone()).unwrap().wait().await.unwrap();
    let applied = control.effective_config().await.unwrap();
    assert!(matches!(
        control.status().controller,
        Some(Host::UnixSocket(_) | Host::NamedPipe(_))
    ));
    assert!(applied.config.contains_key("external-controller"));
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
    );
    #[cfg(unix)]
    let first_socket = match control.status().controller.unwrap() {
        Host::UnixSocket(path) => path,
        _ => panic!("expected local socket"),
    };
    let retained = rx.latest().unwrap();
    let crashed = tokio::spawn(async move {
        let _owned = retained;
        panic!("observer failed");
    });
    assert!(crashed.await.unwrap_err().is_panic());
    let reported = rx.latest().unwrap();
    assert_eq!(reported.instance_id, applied.instance_id);
    assert_eq!(reported.revision, applied.revision);
    assert_eq!(reported.config, applied.config);
    assert!(!format!("{applied:?}").contains("private"));
    // A replay attaches to the original operation; a different policy is not that operation.
    control.submit(first.clone()).unwrap().wait().await.unwrap();
    let mut conflict = first;
    if let CoreCommand::Reconcile(request) = &mut conflict.command {
        request
            .options
            .local_ipc
            .as_mut()
            .unwrap()
            .keep_http_controller = false;
    }
    assert_eq!(
        control.submit(conflict).unwrap_err().kind,
        Some(CoreErrorKind::OperationConflict)
    );
    control
        .submit(envelope(LocalIpcPolicy::Prefer, false))
        .unwrap()
        .wait()
        .await
        .unwrap();
    let ipc_only = control.effective_config().await.unwrap();
    assert!(!ipc_only.config.contains_key("external-controller"));
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err()
    );
    assert_eq!(applied.revision.source_hash, ipc_only.revision.source_hash);
    assert_ne!(
        applied.revision.effective_hash,
        ipc_only.revision.effective_hash
    );
    control
        .submit(envelope(LocalIpcPolicy::Disable, false))
        .unwrap()
        .wait()
        .await
        .unwrap();
    let http = control.effective_config().await.unwrap();
    assert_eq!(rx.latest().unwrap().revision, http.revision);
    assert_eq!(rx.changed().await.unwrap().revision, http.revision);
    assert!(http.config.contains_key("external-controller"));
    assert!(!http.config.contains_key("external-controller-unix"));
    assert!(!http.config.contains_key("external-controller-pipe"));
    control.shutdown().await.unwrap();
    assert!(control.effective_config().await.is_none());
    #[cfg(unix)]
    assert!(!std::path::Path::new(&first_socket).exists());
}

struct HttpOnlyHost(std::sync::atomic::AtomicBool);
impl nyanpasu_core_manager::ControllerAccess for HttpOnlyHost {
    fn supports_local_ipc(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn authorize(&self, host: &Host) -> std::io::Result<()> {
        assert!(matches!(host, Host::Http(_)));
        Ok(())
    }
}

#[tokio::test]
async fn unavailable_host_falls_back_without_disabling_http() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.core.version = Some("1.19.0".into());
    spec.options.local_ipc = Some(LocalIpcSettings {
        policy: LocalIpcPolicy::Prefer,
        keep_http_controller: false,
    });
    let host = Arc::new(HttpOnlyHost(std::sync::atomic::AtomicBool::new(false)));
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        ..Default::default()
    })
    .controller_access(host.clone())
    .build()
    .await
    .unwrap();
    manager.start(spec.clone()).await.unwrap();
    assert!(matches!(manager.status().controller, Some(Host::Http(_))));
    assert!(
        manager
            .effective_config()
            .await
            .unwrap()
            .config
            .contains_key("external-controller")
    );
    let before = manager.effective_config().await.unwrap();
    spec.options.local_ipc.as_mut().unwrap().policy = LocalIpcPolicy::Disable;
    assert!(matches!(
        manager.apply_config(spec, None).await.unwrap(),
        nyanpasu_core_manager::ApplyOutcome::Noop { .. }
    ));
    let after = manager.effective_config().await.unwrap();
    assert_eq!(before.instance_id, after.instance_id);
    assert_eq!(before.revision, after.revision);
    // Restart must remember Disable even when the host becomes IPC-capable.
    host.0.store(true, std::sync::atomic::Ordering::SeqCst);
    manager.restart().await.unwrap();
    assert!(matches!(manager.status().controller, Some(Host::Http(_))));
    manager.shutdown().await.unwrap();
}
