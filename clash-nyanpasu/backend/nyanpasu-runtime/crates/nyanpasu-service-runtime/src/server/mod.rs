pub mod consts;
mod events;
mod logger;
mod manager_bridge;
mod routing;

use std::sync::Arc;

use consts::RuntimeInfos;
pub use events::EventHub;
pub use logger::Logger;
pub use manager_bridge::CoreManagerService as CoreManager;
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
    let core_manager = CoreManager::new(runtime_dir, local_ipc_policy, data_dir).await?;
    let hub = EventHub::new();
    core_manager.spawn_bridges(hub.clone());

    // The tracing writer was bound to the global logger before `run`; share that
    // instance so the `/logs` routes read the buffer that is actually being fed.
    // Nothing forwards it anywhere else: the service's own logs are files, and
    // `/status` reports the directory.
    let logger = Logger::global().clone();

    let state = AppState {
        core_manager: core_manager.clone(),
        hub,
        runtime: Arc::new(runtime),
        logger,
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
            core_manager.shutdown().await;
            result?;
        }
        _ = token.cancelled() => {
            core_manager.shutdown().await;
            drain(&mut server).await?;
        }
        // The control plane owns every core transaction. If its executor is
        // gone the daemon cannot serve `/v2/core/*` truthfully, so it stops
        // rather than answering with a control plane that is not there.
        exit = core_manager.until_control_closed() => {
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
