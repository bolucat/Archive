use std::thread;

use service_manager::{ServiceLabel, ServiceManager, ServiceStatus};

use crate::consts::SERVICE_LABEL;

use super::CommandError;

pub fn start() -> Result<(), CommandError> {
    let manager = crate::utils::get_service_manager()?;
    start_with(manager.as_ref())
}

pub fn start_with(manager: &dyn ServiceManager) -> Result<(), CommandError> {
    let label: ServiceLabel = SERVICE_LABEL.parse()?;
    let status = crate::utils::service::status(manager, &label)?;
    match status {
        ServiceStatus::NotInstalled => {
            return Err(CommandError::ServiceNotInstalled);
        }
        ServiceStatus::Stopped(_) => {
            crate::utils::service::start(manager, &label)?;
        }
        ServiceStatus::Running => {
            tracing::info!("service already running, nothing to do");
            return Err(CommandError::ServiceAlreadyRunning);
        }
    }
    thread::sleep(std::time::Duration::from_secs(3));
    // check if the service is running
    let status = crate::utils::service::status(manager, &label)?;
    if status != ServiceStatus::Running {
        return Err(CommandError::Other(anyhow::anyhow!(
            "service start failed, status: {:?}",
            status
        )));
    }

    Ok(())
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use crate::cmds::test_support::{Call, MockServiceManager, label, status_call};
    use service_manager::ServiceStartCtx;

    #[test]
    fn start_reports_a_missing_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::NotInstalled]);

        assert!(matches!(
            start_with(&manager),
            Err(CommandError::ServiceNotInstalled)
        ));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn start_rejects_an_already_running_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::Running]);

        assert!(matches!(
            start_with(&manager),
            Err(CommandError::ServiceAlreadyRunning)
        ));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn start_starts_a_stopped_service() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Stopped(None),
            ServiceStatus::Running,
        ]);

        start_with(&manager).unwrap();
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
    fn start_fails_when_the_service_does_not_come_up() {
        let manager = MockServiceManager::with_statuses([
            ServiceStatus::Stopped(None),
            ServiceStatus::Stopped(None),
        ]);

        let error = start_with(&manager).unwrap_err();
        assert!(error.to_string().contains("service start failed"));
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
