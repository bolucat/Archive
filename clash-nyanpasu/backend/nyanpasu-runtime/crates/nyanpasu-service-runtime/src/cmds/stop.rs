use std::{thread, time::Duration};

use anyhow::Context;
use service_manager::{ServiceLabel, ServiceManager, ServiceStatus};

use crate::consts::SERVICE_LABEL;

use super::CommandError;

pub fn stop() -> Result<(), CommandError> {
    let manager = crate::utils::get_service_manager()?;
    stop_with(manager.as_ref())
}

pub fn stop_with(manager: &dyn ServiceManager) -> Result<(), CommandError> {
    stop_impl(manager, stop_running_service)
}

/// The stop state machine. Status dispatch and the post-stop confirmation use
/// the injected `manager`; `stop_running` performs the actual stop of a
/// running service — production injects [`stop_running_service`], tests
/// inject a closure that drives the mock.
fn stop_impl(
    manager: &dyn ServiceManager,
    stop_running: impl FnOnce(&ServiceLabel) -> Result<(), CommandError>,
) -> Result<(), CommandError> {
    let label: ServiceLabel = SERVICE_LABEL.parse()?;
    let status = crate::utils::service::status(manager, &label)?;
    match status {
        ServiceStatus::NotInstalled => {
            tracing::info!("service not installed, nothing to do");
            return Err(CommandError::ServiceNotInstalled);
        }
        ServiceStatus::Stopped(_) => {
            tracing::info!("service already stopped");
            return Err(CommandError::ServiceAlreadyStopped);
        }
        ServiceStatus::Running => {
            tracing::info!("service is running, stopping it...");
            stop_running(&label)?;
            tracing::info!("service stopped");
        }
    }
    thread::sleep(std::time::Duration::from_secs(3));
    // check if the service is stopped — via the injected manager
    let status = crate::utils::service::status(manager, &label)?;
    if !matches!(status, ServiceStatus::Stopped(_)) {
        return Err(CommandError::Other(anyhow::anyhow!(
            "service stop failed, status: {:?}",
            status
        )));
    }
    Ok(())
}

/// Stop a running service with an 8s timeout and a process-kill fallback.
/// System-bound by design: the timeout must be able to abandon a hung stop,
/// so the worker thread needs an owned (`'static`) manager and re-fetches the
/// platform one instead of borrowing the caller's.
fn stop_running_service(label: &ServiceLabel) -> Result<(), CommandError> {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let label_ = label.clone();
        let handle = tokio::task::spawn_blocking(move || {
            let manager = crate::utils::get_service_manager()?;
            crate::utils::service::stop(manager.as_ref(), &label_)?;
            anyhow::Ok(())
        });

        match tokio::time::timeout(Duration::from_secs(8), handle).await {
            Ok(res) => res.context("failed to join service stop task").flatten(),
            Err(e) => {
                tracing::error!("service stop timed out: {:?}, trying to kill it", e);
                let mut sys = sysinfo::System::new_all();
                sys.refresh_all();
                let pkg_name = crate::consts::APP_NAME;
                let current_pid = std::process::id();
                tracing::info!("Try to find `{pkg_name}`...");
                for (pid, process) in sys.processes() {
                    if let Some(path) = process.cwd()
                        && path.to_string_lossy().contains(pkg_name)
                        && pid.as_u32() != current_pid
                    {
                        tracing::info!("killing process: {:?}", pid);
                        process.kill();
                    }
                }
                Ok(())
            }
        }
    })?;
    Ok(())
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use crate::cmds::test_support::{Call, MockServiceManager, label, status_call};
    use service_manager::ServiceStopCtx;

    #[test]
    fn stop_reports_a_missing_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::NotInstalled]);

        assert!(matches!(
            stop_with(&manager),
            Err(CommandError::ServiceNotInstalled)
        ));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn stop_reports_an_already_stopped_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::Stopped(None)]);

        assert!(matches!(
            stop_with(&manager),
            Err(CommandError::ServiceAlreadyStopped)
        ));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn stop_stops_a_running_service() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Running,
            ServiceStatus::Stopped(None),
        ]);

        stop_impl(&manager, |label| {
            crate::utils::service::stop(&manager, label).map_err(CommandError::Io)
        })
        .unwrap();
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Stop(ServiceStopCtx { label: label() }),
                status_call(),
            ]
        );
    }

    #[test]
    fn stop_fails_when_the_service_survives() {
        let manager =
            MockServiceManager::with_statuses([ServiceStatus::Running, ServiceStatus::Running]);

        let error = stop_impl(&manager, |_| Ok(())).unwrap_err();
        assert!(error.to_string().contains("service stop failed"));
        assert_eq!(manager.calls(), [status_call(), status_call()]);
    }
}
