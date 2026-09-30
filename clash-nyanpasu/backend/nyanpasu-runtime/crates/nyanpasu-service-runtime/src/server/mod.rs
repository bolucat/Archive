pub mod consts;
mod controller_access;
mod events;
mod logger;
mod manager_bridge;
mod routing;

use std::sync::Arc;

use consts::RuntimeInfos;
pub use events::EventHub;
pub use logger::Logger;
pub use manager_bridge::CoreManagerService as CoreManager;
use nyanpasu_core_manager::{
    ExecutorExit, LocalIpcPolicy,
    native_store::{FsNativeStore, StoreOwner, legacy_kind},
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
    let native_store = Arc::new(native_store_for_host(&runtime)?);
    let (controller_dir, access): (_, Arc<dyn nyanpasu_core_manager::ControllerAccess>) =
        controller_access_for_host(sids)?;
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
        sids,
    );
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => {
            let _ = logs.shutdown().await;
            core_manager.shutdown().await;
            result?;
        }
        _ = token.cancelled() => {
            let _ = logs.shutdown().await;
            core_manager.shutdown().await;
            drain(&mut server).await?;
        }
        // The control plane owns every core transaction. If its executor is
        // gone the daemon cannot serve `/v2/core/*` truthfully, so it stops
        // rather than answering with a control plane that is not there.
        exit = core_manager.until_control_closed() => {
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
    let config_path = runtime.nyanpasu_config_dir.join("application.yaml");
    let config_path = if config_path.try_exists()? {
        config_path
    } else {
        runtime.nyanpasu_config_dir.join("nyanpasu-config.yaml")
    };
    Ok(FsNativeStore::new(
        data_dir,
        owner,
        legacy_kind(&config_path)?,
    ))
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
            match controller_access::UnixControllerAccess::prepare(root, gid) {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
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
    fn prefers_the_typed_config_over_the_legacy_file() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = runtime(dir.path());
        std::fs::create_dir(&runtime.nyanpasu_config_dir).unwrap();
        let typed = runtime.nyanpasu_config_dir.join("application.yaml");
        std::fs::write(&typed, "core: mihomo\n").unwrap();
        std::fs::write(
            runtime.nyanpasu_config_dir.join("nyanpasu-config.yaml"),
            "invalid: [",
        )
        .unwrap();
        assert!(native_store_for_host(&runtime).is_ok());
        std::fs::remove_file(typed).unwrap();
        assert!(native_store_for_host(&runtime).is_err());
    }

    #[cfg(unix)]
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

    #[cfg(unix)]
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
