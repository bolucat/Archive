#![cfg(windows)]
mod common;
use futures_util::StreamExt;

use nyanpasu_core_manager::{
    ControllerAccess, ControllerAuthorization, CoreKind, CoreManager, Host, LocalIpcPolicy,
    ManagerOptions,
};
use nyanpasu_windows_security::{acl::get_current_user_sid_string, pipe::PipeSecurity};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

struct Access {
    policy: PipeSecurity,
    creation_override: Option<String>,
    observed: tokio::sync::mpsc::UnboundedSender<(u32, ControllerAuthorization)>,
}
impl ControllerAccess for Access {
    fn supports_local_ipc(&self) -> bool {
        true
    }
    fn pipe_security_descriptor(&self) -> Option<&str> {
        Some(
            self.creation_override
                .as_deref()
                .unwrap_or_else(|| self.policy.sddl()),
        )
    }
    fn authorize(
        &self,
        host: &Host,
        pid: u32,
        mode: ControllerAuthorization,
    ) -> std::io::Result<()> {
        if let Host::NamedPipe(path) = host {
            self.policy
                .authorize(path, pid, mode == ControllerAuthorization::Host)
                .map_err(std::io::Error::other)?;
            let _ = self.observed.send((pid, mode));
        }
        Ok(())
    }
}

#[tokio::test]
async fn native_permissions_survive_process_respawn() {
    exercise(None, ControllerAuthorization::Core).await;
}

#[tokio::test]
async fn host_permissions_survive_process_respawn() {
    exercise(None, ControllerAuthorization::Host).await;
}

#[tokio::test]
#[ignore = "requires MIHOMO_BIN"]
async fn real_mihomo_native_permissions() {
    exercise(Some(&common::real::MIHOMO), ControllerAuthorization::Core).await;
}

#[tokio::test]
#[ignore = "requires CLASH_RS_BIN"]
async fn real_clash_rs_host_permissions() {
    exercise(Some(&common::real::CLASH_RS), ControllerAuthorization::Host).await;
}

// A successful probe does not reserve a pipe instance for the next caller.
// Wait on Windows' availability signal rather than assuming the core has
// already replaced the instance consumed by the preceding request.
async fn authorize_when_available(policy: PipeSecurity, path: &Path, pid: u32, set: bool) {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        use windows::{
            Win32::{Foundation::ERROR_PIPE_BUSY, System::Pipes::WaitNamedPipeW},
            core::HSTRING,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match policy.authorize(&path, pid, set) {
                Ok(()) => return,
                Err(error) => {
                    assert_eq!(
                        error
                            .downcast_ref::<windows::core::Error>()
                            .map(|error| error.code()),
                        Some(ERROR_PIPE_BUSY.to_hresult()),
                        "{error:#}",
                    );
                    let remaining = deadline
                        .saturating_duration_since(Instant::now())
                        .as_millis();
                    assert!(remaining > 0, "controller pipe remained busy: {error:#}");
                    unsafe {
                        WaitNamedPipeW(&HSTRING::from(path.as_os_str()), remaining as u32).ok()
                    }
                    .expect("wait for an available controller pipe instance");
                }
            }
        }
    })
    .await
    .unwrap();
}

async fn exercise(core: Option<&common::real::RealCore>, expected: ControllerAuthorization) {
    let (_guard, dir) = common::utf8_tempdir();
    let user = get_current_user_sid_string().unwrap();
    let (tx, mut observed) = tokio::sync::mpsc::unbounded_channel();
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: Some(format!(
            r"\\.\pipe\nyanpasu-permissions-{}-{{epoch}}",
            uuid::Uuid::new_v4()
        )),
        ..ManagerOptions::default()
    })
    .controller_access(Arc::new(Access {
        policy: PipeSecurity::new(&[&user]).unwrap(),
        creation_override: None,
        observed: tx,
    }))
    .build()
    .await
    .unwrap();
    let path = common::write_config(&dir, "rules:\n  - MATCH,DIRECT\n");
    let spec = match core {
        Some(core) => common::real::real_spec(core, &dir, path),
        None => {
            let mut spec = common::mihomo_spec(&dir, path);
            if expected == ControllerAuthorization::Host {
                spec.core.kind = CoreKind::ClashRust;
                spec.core.version = Some("0.9.7".into());
            }
            spec
        }
    };
    manager.start(spec).await.unwrap();
    let (first_pid, mode) = observed.recv().await.unwrap();
    assert_eq!(mode, expected);
    let binding = manager.api_connection().await.unwrap();
    let client = clash_api::Client::new(binding.controller.host.clone()).unwrap();
    client.version().await.unwrap();
    if core.is_some() {
        let mut stream = client.traffic_ws().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
    if expected == ControllerAuthorization::Host {
        let Host::NamedPipe(path) = &binding.controller.host else {
            panic!("expected pipe")
        };
        // Simulate a listener resetting its policy without replacing the core.
        let permissive = PipeSecurity::new(&[&user, "S-1-1-0"]).unwrap();
        authorize_when_available(permissive, path, first_pid, true).await;
        while observed.try_recv().is_ok() {}
        let (pid, mode) = tokio::time::timeout(Duration::from_secs(5), observed.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!((pid, mode), (first_pid, expected));
        // The liveness acknowledgement is only emitted after the strict policy
        // was restored, including when no custom liveness probe is configured.
        authorize_when_available(PipeSecurity::new(&[&user]).unwrap(), path, first_pid, false)
            .await;
    }
    // Explicit termination drives the actual supervisor's respawn path.
    let result = tokio::process::Command::new("taskkill")
        .args(["/PID", &first_pid.to_string(), "/F"])
        .output()
        .await
        .unwrap();
    assert!(result.status.success());
    let (new_pid, mode) = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let value = observed.recv().await.unwrap();
            if value.0 != first_pid {
                break value;
            }
        }
    })
    .await
    .unwrap();
    assert_ne!(new_pid, first_pid);
    assert_eq!(mode, expected);
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn incorrect_native_descriptor_never_becomes_ready() {
    let (_guard, dir) = common::utf8_tempdir();
    let user = get_current_user_sid_string().unwrap();
    let (tx, mut observed) = tokio::sync::mpsc::unbounded_channel();
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        local_ipc_policy: LocalIpcPolicy::Force,
        controller_template: Some(format!(
            r"\\.\pipe\nyanpasu-reject-{}-{{epoch}}",
            uuid::Uuid::new_v4()
        )),
        ..ManagerOptions::default()
    })
    .controller_access(Arc::new(Access {
        policy: PipeSecurity::new(&[&user]).unwrap(),
        creation_override: Some(
            PipeSecurity::new(&[&user, "S-1-1-0"])
                .unwrap()
                .sddl()
                .into(),
        ),
        observed: tx,
    }))
    .build()
    .await
    .unwrap();
    let config = common::write_config(&dir, "rules: []\n");
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_millis(750);
    let result = manager.start(spec).await;
    assert!(
        matches!(
            result,
            Err(nyanpasu_core_manager::Error::StartupTimeout { .. })
        ),
        "{result:?}"
    );
    assert!(observed.try_recv().is_err());
    manager.shutdown().await.unwrap();
}
