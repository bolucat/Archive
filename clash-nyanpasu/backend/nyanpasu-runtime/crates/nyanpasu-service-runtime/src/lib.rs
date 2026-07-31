#![feature(error_generic_member_access)]

mod cmds;
pub mod consts;
mod logging;
mod server;
pub mod utils;

use consts::ExitCode;
use tokio_util::sync::CancellationToken;
use tracing::error;

pub async fn handler() -> ExitCode {
    crate::utils::deadlock_detection();
    let result = cmds::process().await;
    match result {
        Ok(_) => ExitCode::Normal,
        Err(cmds::CommandError::PermissionDenied) => {
            eprintln!("Permission denied, please run as administrator or root");
            ExitCode::PermissionDenied
        }
        Err(cmds::CommandError::ServiceNotInstalled) => {
            eprintln!("Service not installed");
            ExitCode::ServiceNotInstalled
        }
        Err(cmds::CommandError::ServiceAlreadyInstalled) => {
            eprintln!("Service already installed");
            ExitCode::ServiceAlreadyInstalled
        }
        Err(cmds::CommandError::ServiceAlreadyStopped) => {
            eprintln!("Service already stopped");
            ExitCode::ServiceAlreadyStopped
        }
        Err(cmds::CommandError::ServiceAlreadyRunning) => {
            eprintln!("Service already running");
            ExitCode::ServiceAlreadyRunning
        }
        Err(e) => {
            error!("Error: {:#?}", e);
            ExitCode::Other
        }
    }
}

/// The running server's cancellation token, published by the `server` command.
/// The Windows service control handler cancels it when the SCM asks to stop.
pub fn server_shutdown_token() -> Option<CancellationToken> {
    cmds::SERVER_SHUTDOWN_TOKEN.get().cloned()
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_module_paths_keep_the_legacy_log_target_root() {
        assert_eq!(module_path!().split("::").next(), Some("nyanpasu_service"));
    }
}
