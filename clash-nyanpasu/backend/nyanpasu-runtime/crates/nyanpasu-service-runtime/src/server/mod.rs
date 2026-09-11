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
pub use manager_bridge::{CoreManagerService as CoreManager, ServiceDirs};
use nyanpasu_core_manager::{ExecutorExit, LocalIpcPolicy};
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
    let data_dir = camino::Utf8PathBuf::from_path_buf(runtime.nyanpasu_data_dir.clone())
        .map_err(|path| anyhow::anyhow!("nyanpasu data dir is not UTF-8: {}", path.display()))?;
    let (controller_dir, access): (_, Arc<dyn nyanpasu_core_manager::ControllerAccess>) =
        controller_access_for_host();
    let core_manager = CoreManager::with_controller_access(
        ServiceDirs {
            runtime: runtime_dir,
            data: data_dir,
        },
        local_ipc_policy,
        controller_dir,
        access,
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

fn controller_access_for_host() -> (
    Option<camino::Utf8PathBuf>,
    Arc<dyn nyanpasu_core_manager::ControllerAccess>,
) {
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
                        return (path, Arc::new(access));
                    }
                }
                Err(error) => tracing::warn!("service core IPC is unavailable: {error}"),
            }
        }
    }
    // Windows core-owned pipe ACLs have not yet passed the non-elevated GUI gate.
    (
        None,
        Arc::new(controller_access::UnavailableControllerAccess),
    )
}
