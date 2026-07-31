use std::thread;

use service_manager::{ServiceLabel, ServiceManager, ServiceStatus, ServiceUninstallCtx};

use crate::consts::SERVICE_LABEL;

use super::CommandError;

pub fn uninstall() -> Result<(), CommandError> {
    let manager = crate::utils::get_service_manager()?;
    uninstall_with(manager.as_ref())
}

pub fn uninstall_with(manager: &dyn ServiceManager) -> Result<(), CommandError> {
    let label: ServiceLabel = SERVICE_LABEL.parse()?;
    let status = crate::utils::service::status(manager, &label)?;
    match status {
        ServiceStatus::NotInstalled => {
            tracing::info!("service not installed, nothing to do");
            return Err(CommandError::ServiceNotInstalled);
        }
        ServiceStatus::Stopped(_) => {
            tracing::info!("service already stopped, so we can uninstall it directly");
            manager.uninstall(ServiceUninstallCtx {
                label: label.clone(),
            })?;
        }
        ServiceStatus::Running => {
            tracing::info!("Service is running, we need to stop it first");
            crate::utils::service::stop(manager, &label)?;
            thread::sleep(std::time::Duration::from_secs(5)); // wait for the service to stop
            manager.uninstall(ServiceUninstallCtx {
                label: label.clone(),
            })?;
        }
    }
    tracing::info!("confirming service is uninstalled...");
    let status = crate::utils::service::status(manager, &label)?;
    if status != ServiceStatus::NotInstalled {
        return Err(CommandError::Other(anyhow::anyhow!(
            "service uninstall failed, status: {:?}",
            status
        )));
    }
    Ok(())
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use crate::cmds::test_support::{Call, MockServiceManager, label, status_call};
    use service_manager::ServiceStopCtx;

    #[test]
    fn uninstall_reports_a_missing_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::NotInstalled]);

        assert!(matches!(
            uninstall_with(&manager),
            Err(CommandError::ServiceNotInstalled)
        ));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn uninstall_removes_a_stopped_service() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Stopped(None),
            ServiceStatus::NotInstalled,
        ]);

        uninstall_with(&manager).unwrap();
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Uninstall(ServiceUninstallCtx { label: label() }),
                status_call(),
            ]
        );
    }

    #[test]
    fn uninstall_fails_when_the_service_survives() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Stopped(None),
            ServiceStatus::Running,
        ]);

        let error = uninstall_with(&manager).unwrap_err();
        assert!(error.to_string().contains("service uninstall failed"));
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Uninstall(ServiceUninstallCtx { label: label() }),
                status_call(),
            ]
        );
    }

    #[test]
    fn uninstall_stops_then_removes_a_running_service() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Running,
            ServiceStatus::NotInstalled,
        ]);

        uninstall_with(&manager).unwrap();
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Stop(ServiceStopCtx { label: label() }),
                Call::Uninstall(ServiceUninstallCtx { label: label() }),
                status_call(),
            ]
        );
    }
}
