use service_manager::{ServiceLabel, ServiceManager, ServiceStatus, ServiceStatusCtx};

#[cfg(not(target_os = "macos"))]
use service_manager::{ServiceStartCtx, ServiceStopCtx};

pub fn status(
    manager: &dyn ServiceManager,
    label: &ServiceLabel,
) -> std::io::Result<ServiceStatus> {
    let status = manager.status(ServiceStatusCtx {
        label: label.clone(),
    })?;

    #[cfg(target_os = "macos")]
    {
        Ok(macos::normalize_status(
            status,
            macos::plist_path(label).exists(),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(status)
    }
}

pub fn start(manager: &dyn ServiceManager, label: &ServiceLabel) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = manager;
        macos::bootstrap(label)
    }

    #[cfg(not(target_os = "macos"))]
    {
        manager.start(ServiceStartCtx {
            label: label.clone(),
        })
    }
}

pub fn stop(manager: &dyn ServiceManager, label: &ServiceLabel) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = manager;
        macos::bootout(label)
    }

    #[cfg(not(target_os = "macos"))]
    {
        manager.stop(ServiceStopCtx {
            label: label.clone(),
        })
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{ffi::OsString, path::PathBuf, process::Command};

    use service_manager::{ServiceLabel, ServiceStatus};

    const LAUNCHCTL: &str = "launchctl";
    const LAUNCHD_SYSTEM_DOMAIN: &str = "system";

    pub(super) fn normalize_status(status: ServiceStatus, plist_exists: bool) -> ServiceStatus {
        if status == ServiceStatus::NotInstalled && plist_exists {
            ServiceStatus::Stopped(None)
        } else {
            status
        }
    }

    pub(super) fn plist_path(label: &ServiceLabel) -> PathBuf {
        PathBuf::from("/Library/LaunchDaemons").join(format!("{}.plist", label.to_qualified_name()))
    }

    fn bootout_args(label: &ServiceLabel) -> Vec<OsString> {
        vec![
            "bootout".into(),
            format!("{LAUNCHD_SYSTEM_DOMAIN}/{}", label.to_qualified_name()).into(),
        ]
    }

    fn bootstrap_args(label: &ServiceLabel) -> Vec<OsString> {
        vec![
            "bootstrap".into(),
            LAUNCHD_SYSTEM_DOMAIN.into(),
            plist_path(label).into_os_string(),
        ]
    }

    fn run(args: &[OsString]) -> std::io::Result<()> {
        let output = Command::new(LAUNCHCTL).args(args).output()?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        Err(std::io::Error::other(format!(
            "{LAUNCHCTL} {} failed with status {}: {}{}",
            args.iter()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            output.status,
            stderr.trim(),
            stdout.trim()
        )))
    }

    pub(super) fn bootout(label: &ServiceLabel) -> std::io::Result<()> {
        run(&bootout_args(label))
    }

    pub(super) fn bootstrap(label: &ServiceLabel) -> std::io::Result<()> {
        run(&bootstrap_args(label))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn label() -> ServiceLabel {
            "moe.elaina.nyanpasu-service".parse().unwrap()
        }

        #[test]
        fn builds_system_bootout_command() {
            assert_eq!(
                bootout_args(&label()),
                ["bootout", "system/moe.elaina.nyanpasu-service"]
                    .map(OsString::from)
                    .to_vec()
            );
        }

        #[test]
        fn builds_system_bootstrap_command() {
            assert_eq!(
                bootstrap_args(&label()),
                [
                    "bootstrap",
                    "system",
                    "/Library/LaunchDaemons/moe.elaina.nyanpasu-service.plist",
                ]
                .map(OsString::from)
                .to_vec()
            );
        }

        #[test]
        fn treats_an_unloaded_plist_as_stopped() {
            assert_eq!(
                normalize_status(ServiceStatus::NotInstalled, true),
                ServiceStatus::Stopped(None)
            );
            assert_eq!(
                normalize_status(ServiceStatus::NotInstalled, false),
                ServiceStatus::NotInstalled
            );
        }
    }
}
