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
use nyanpasu_core_manager::LocalIpcPolicy;
use nyanpasu_ipc::{
    SERVICE_PLACEHOLDER,
    api::ws::events::{Event as WsEvent, TraceLog},
    server::create_server,
};
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
    let core_manager = CoreManager::new(runtime_dir, local_ipc_policy).await?;
    let hub = EventHub::new();
    core_manager.spawn_bridges(hub.clone());

    // The tracing writer was bound to the global logger before `run`; share that
    // instance so the `/logs` routes read the buffer that is actually being fed.
    let logger = Logger::global().clone();
    let log_hub = hub.clone();
    logger.set_subscriber(Box::new(move |logging| {
        // Runs inside the tracing writer: stays synchronous, never logs.
        let target = logging
            .fields
            .get("target")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .unwrap_or("".to_string());
        if !events::should_forward_to_hub(&target) {
            return;
        }
        log_hub.send(WsEvent::new_log(TraceLog {
            timestamp: logging.timestamp,
            level: logging.level,
            message: logging
                .fields
                .get("message")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or("".to_string()),
            target,
            fields: logging.fields,
        }));
    }));

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
            match tokio::time::timeout(SERVER_DRAIN_TIMEOUT, &mut server).await {
                Ok(result) => result?,
                Err(_) => tracing::warn!(
                    "pipe server did not drain within {SERVER_DRAIN_TIMEOUT:?}; abandoning open connections"
                ),
            }
        }
    }
    Ok(())
}
