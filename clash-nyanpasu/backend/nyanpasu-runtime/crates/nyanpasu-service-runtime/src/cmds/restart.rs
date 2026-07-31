use service_manager::{ServiceLabel, ServiceManager, ServiceStatus};

use crate::consts::SERVICE_LABEL;

use super::CommandError;

pub fn restart() -> Result<(), CommandError> {
    let manager = crate::utils::get_service_manager()?;
    restart_with(manager.as_ref())
}

pub fn restart_with(manager: &dyn ServiceManager) -> Result<(), CommandError> {
    let label: ServiceLabel = SERVICE_LABEL.parse()?;
    let status = crate::utils::service::status(manager, &label)?;
    match status {
        ServiceStatus::NotInstalled => {
            return Err(CommandError::ServiceNotInstalled);
        }
        ServiceStatus::Stopped(_) => {
            tracing::info!("service already stopped, starting it...");
            crate::utils::service::start(manager, &label)?;
        }
        ServiceStatus::Running => {
            tracing::info!("service is running, stopping it...");
            crate::utils::service::stop(manager, &label)?;
            std::thread::sleep(std::time::Duration::from_secs(3)); // wait for the service to stop
            crate::utils::service::start(manager, &label)?;
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(3));
    // check if the service is running
    let status = crate::utils::service::status(manager, &label)?;
    if status != ServiceStatus::Running {
        return Err(CommandError::Other(anyhow::anyhow!(
            "service restart failed, status: {:?}",
            status
        )));
    }
    Ok(())
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use crate::cmds::test_support::{Call, MockServiceManager, label, status_call};
    use service_manager::{ServiceStartCtx, ServiceStopCtx};

    #[test]
    fn restart_reports_a_missing_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::NotInstalled]);

        assert!(matches!(
            restart_with(&manager),
            Err(CommandError::ServiceNotInstalled)
        ));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn restart_starts_a_stopped_service() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Stopped(None),
            ServiceStatus::Running,
        ]);

        restart_with(&manager).unwrap();
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Start(ServiceStartCtx { label: label() }),
                status_call(),
            ]
        );
    }

    #[test]
    fn restart_cycles_a_running_service() {
        let manager =
            MockServiceManager::with_statuses([ServiceStatus::Running, ServiceStatus::Running]);

        restart_with(&manager).unwrap();
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Stop(ServiceStopCtx { label: label() }),
                Call::Start(ServiceStartCtx { label: label() }),
                status_call(),
            ]
        );
    }

    #[test]
    fn restart_fails_when_the_service_does_not_come_back() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Stopped(None),
            ServiceStatus::Stopped(None),
        ]);

        let error = restart_with(&manager).unwrap_err();
        assert!(error.to_string().contains("service restart failed"));
        assert_eq!(
            manager.calls(),
            [
                status_call(),
                Call::Start(ServiceStartCtx { label: label() }),
                status_call(),
            ]
        );
    }
}
