pub mod consts;
mod controller_access;
mod events;
mod logger;
mod manager_bridge;
mod routing;
mod transparent_proxy;

use std::sync::Arc;

use consts::RuntimeInfos;
pub use events::EventHub;
pub use logger::Logger;
pub use manager_bridge::CoreManagerService as CoreManager;
use nyanpasu_core_manager::{
    ExecutorExit, LocalIpcPolicy,
    native_store::{FsNativeStore, StoreOwner},
};
use nyanpasu_ipc::{SERVICE_PLACEHOLDER, server::create_server};
use routing::{AppState, create_router};
use tokio_util::sync::CancellationToken;
use tracing_attributes::instrument;

const SERVER_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[instrument(skip(runtime))]
pub async fn run(
    runtime: RuntimeInfos,
    local_ipc_policy: LocalIpcPolicy,
    token: CancellationToken,
    #[cfg(windows)] sids: &[&str],
    #[cfg(not(windows))] sids: (),
) -> Result<(), anyhow::Error> {
    let runtime_dir =
        camino::Utf8PathBuf::from_path_buf(crate::utils::dirs::service_core_runtime_dir())
            .map_err(|path| anyhow::anyhow!("core runtime dir is not UTF-8: {}", path.display()))?;
    #[cfg(unix)]
    let socket_owner_uid = crate::utils::native_store_owner::FsOwnerBindingStore::new(
        &runtime.service_config_dir,
        &runtime.nyanpasu_data_dir,
    )
    .load()?
    .map(|owner| owner.uid);
    let native_store = Arc::new(native_store_for_host(&runtime)?);
    let (controller_dir, access) = controller_access_for_host(
        sids,
        #[cfg(target_os = "linux")]
        socket_owner_uid,
    )?;
    let core_manager = CoreManager::with_controller_access(
        runtime_dir,
        local_ipc_policy,
        controller_dir,
        access,
        native_store,
    )
    .await?;
    let hub = EventHub::new();
    core_manager.spawn_bridges(hub.clone());
    let transparent_proxy = transparent_proxy::TransparentProxy::new(core_manager.clone());
    if let Err(error) = transparent_proxy.recover().await {
        tracing::error!("failed to recover transparent proxy state: {error}");
    }
    let mut core_status = core_manager.subscribe_runtime_status();
    let proxy_lifecycle = transparent_proxy.clone();
    let proxy_shutdown = transparent_proxy.clone();
    let lifecycle_token = token.clone();
    let lifecycle_shutdown = token.clone();
    let lifecycle_task = tokio::spawn(async move {
        let mut retry = tokio::time::interval(std::time::Duration::from_secs(5));
        retry.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = lifecycle_token.cancelled() => break,
                _ = retry.tick() => {
                    if let Err(error) = proxy_lifecycle.cleanup_if_stale().await {
                        tracing::error!("failed to retry transparent proxy cleanup: {error}");
                    }
                }
                changed = core_status.changed() => {
                    if changed.is_err() { break; }
                    core_status.borrow_and_update();
                    if let Err(error) = proxy_lifecycle.cleanup_if_stale().await {
                        tracing::error!("failed to remove stale transparent proxy rules: {error}");
                    }
                }
            }
        }
        proxy_lifecycle.begin_shutdown();
        if let Err(error) = proxy_lifecycle.cleanup().await {
            tracing::error!("failed to clean transparent proxy rules on shutdown: {error}");
        }
    });

    // The tracing writer was bound to the global logger before `run`; share that
    // instance so the `/logs` routes read the buffer that is actually being fed.
    // Nothing forwards it anywhere else: the service's own logs are files, and
    // `/status` reports the directory.
    let logger = Logger::global().clone();
    let logs = nyanpasu_logging::LogsClient::start(
        Arc::new(nyanpasu_logging::FsLogFiles::new(
            crate::utils::dirs::service_logs_dir(),
            "nyanpasu-service".into(),
        )),
        Arc::new(nyanpasu_logging::MonotonicClock::default()),
    )
    .await?;

    let state = AppState {
        core_manager: core_manager.clone(),
        hub,
        runtime: Arc::new(runtime),
        logger,
        logs: logs.clone(),
        transparent_proxy,
    };
    let app = create_router(state);
    tracing::info!("Starting server...");
    let shutdown_token = token.clone();
    let server = create_server(
        SERVICE_PLACEHOLDER,
        app,
        Some(async move {
            shutdown_token.cancelled().await;
        }),
        #[cfg(windows)]
        sids,
        #[cfg(not(windows))]
        socket_owner_uid,
    );
    tokio::pin!(server);
    enum ServerExit<E> {
        Server(Result<(), E>),
        Cancelled,
        Control(ExecutorExit),
    }
    let exit = tokio::select! {
        result = &mut server => ServerExit::Server(result),
        _ = token.cancelled() => ServerExit::Cancelled,
        exit = core_manager.until_control_closed() => ServerExit::Control(exit),
    };
    proxy_shutdown.begin_shutdown();
    lifecycle_shutdown.cancel();
    if let Err(error) = lifecycle_task.await {
        tracing::error!("transparent proxy lifecycle task failed: {error}");
    }
    match exit {
        ServerExit::Server(result) => {
            let _ = logs.shutdown().await;
            core_manager.shutdown().await;
            result?;
        }
        ServerExit::Cancelled => {
            let _ = logs.shutdown().await;
            core_manager.shutdown().await;
            drain(&mut server).await?;
        }
        // The control plane owns every core transaction. If its executor is
        // gone the daemon cannot serve `/v2/core/*` truthfully, so it stops
        // rather than answering with a control plane that is not there.
        ServerExit::Control(exit) => {
            let _ = logs.shutdown().await;
            if exit == ExecutorExit::Died {
                tracing::error!("the core control executor died; shutting the service down");
                core_manager.shutdown().await;
                drain(&mut server).await?;
                anyhow::bail!("the core control executor died");
            }
            // Clean: a local shutdown already ran. Nothing maps `Shutdown` onto
            // the wire, so this is the service's own teardown finishing.
            core_manager.shutdown().await;
            drain(&mut server).await?;
        }
    }
    Ok(())
}

fn native_store_for_host(runtime: &RuntimeInfos) -> anyhow::Result<FsNativeStore> {
    let data_dir = camino::Utf8PathBuf::from_path_buf(runtime.nyanpasu_data_dir.clone())
        .map_err(|path| anyhow::anyhow!("nyanpasu data dir is not UTF-8: {}", path.display()))?;
    #[cfg(unix)]
    let owner = crate::utils::native_store_owner::FsOwnerBindingStore::new(
        &runtime.service_config_dir,
        &runtime.nyanpasu_data_dir,
    )
    .load()?
    .map_or(StoreOwner::Missing, |owner| StoreOwner::Unix {
        uid: owner.uid,
        gid: owner.gid,
    });
    #[cfg(windows)]
    let owner = StoreOwner::current();
    Ok(FsNativeStore::new(data_dir, owner))
}

async fn drain<E: std::error::Error + Send + Sync + 'static>(
    server: impl std::future::Future<Output = Result<(), E>> + Unpin,
) -> Result<(), anyhow::Error> {
    match tokio::time::timeout(SERVER_DRAIN_TIMEOUT, server).await {
        Ok(result) => result?,
        Err(_) => tracing::warn!(
            "pipe server did not drain within {SERVER_DRAIN_TIMEOUT:?}; abandoning open connections"
        ),
    }
    Ok(())
}

fn controller_access_for_host(
    #[cfg(windows)] sids: &[&str],
    #[cfg(not(windows))] _sids: (),
    #[cfg(target_os = "linux")] desktop_uid: Option<u32>,
) -> anyhow::Result<(
    Option<camino::Utf8PathBuf>,
    Arc<dyn nyanpasu_core_manager::ControllerAccess>,
)> {
    #[cfg(windows)]
    {
        Ok((
            None,
            Arc::new(controller_access::WindowsControllerAccess::new(sids)?),
        ))
    }
    #[cfg(unix)]
    {
        // The installation establishes this authorization group for GUI users.
        let mut group = std::mem::MaybeUninit::<libc::group>::uninit();
        let mut result = std::ptr::null_mut();
        let mut buffer = vec![0u8; 16 * 1024];
        let error = unsafe {
            libc::getgrnam_r(
                c"nyanpasu".as_ptr(),
                group.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if error == 0 && !result.is_null() {
            let gid = unsafe { group.assume_init().gr_gid };
            let root = std::path::Path::new("/var/run/nyanpasu-core");
            match controller_access::UnixControllerAccess::prepare(
                root,
                gid,
                #[cfg(target_os = "linux")]
                desktop_uid,
            ) {
                Ok(access) => {
                    let path = root
                        .canonicalize()
                        .ok()
                        .and_then(|path| camino::Utf8PathBuf::from_path_buf(path).ok());
                    if path.is_some() {
                        return Ok((path, Arc::new(access)));
                    }
                }
                Err(error) => tracing::warn!("service core IPC is unavailable: {error}"),
            }
        }
    }
    #[cfg(not(windows))]
    Ok((
        None,
        Arc::new(controller_access::UnavailableControllerAccess),
    ))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use nyanpasu_core_manager::{
        CoreKind,
        native_store::{NativeStore, StoreError},
    };

    fn runtime(root: &std::path::Path) -> RuntimeInfos {
        RuntimeInfos {
            service_data_dir: root.join("service-data"),
            service_config_dir: root.join("service-config"),
            nyanpasu_config_dir: root.join("config"),
            nyanpasu_data_dir: root.join("data"),
            nyanpasu_app_dir: root.join("app"),
        }
    }

    #[test]
    fn an_unbound_service_can_check_but_cannot_start_a_native_writer() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = runtime(dir.path());
        std::fs::create_dir(&runtime.nyanpasu_data_dir).unwrap();
        let store = native_store_for_host(&runtime).unwrap();
        assert!(store.check_home().is_ok());
        assert!(matches!(
            store.acquire(CoreKind::Mihomo),
            Err(StoreError::OwnerMissing)
        ));
        assert!(!runtime.nyanpasu_data_dir.join("native-store").exists());
    }

    #[test]
    #[ignore = "requires UID 0; can run inside a private user namespace"]
    fn privileged_service_loads_its_binding_and_preserves_legacy_data() {
        assert_eq!(unsafe { libc::geteuid() }, 0);
        let dir = tempfile::tempdir().unwrap();
        let runtime = runtime(dir.path());
        std::fs::create_dir(&runtime.nyanpasu_data_dir).unwrap();
        let legacy = runtime.nyanpasu_data_dir.join("cache.db");
        std::fs::write(&legacy, "selected: {proxy: DIRECT}\n").unwrap();
        crate::utils::native_store_owner::FsOwnerBindingStore::new(
            &runtime.service_config_dir,
            &runtime.nyanpasu_data_dir,
        )
        .save("root")
        .unwrap();
        let store = native_store_for_host(&runtime).unwrap();
        let lease = store.acquire(CoreKind::ClashRust).unwrap();
        assert_eq!(
            std::fs::read(store.home(CoreKind::ClashRust).join("cache.db")).unwrap(),
            std::fs::read(&legacy).unwrap()
        );
        lease.confirm_stopped().unwrap();
    }
}
