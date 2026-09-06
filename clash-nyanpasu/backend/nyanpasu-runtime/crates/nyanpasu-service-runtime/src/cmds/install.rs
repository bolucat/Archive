use std::{env::current_exe, ffi::OsString, path::PathBuf};

use service_manager::{
    RestartPolicy, ServiceInstallCtx, ServiceLabel, ServiceManager, ServiceStatus,
};

use crate::consts::{APP_NAME, SERVICE_LABEL};

use super::{CommandError, LocalIpcPolicyArg};

/// Every argument may come from its `NYANPASU_*` variable instead; an explicit
/// flag always wins. Note for callers: `sudo` resets the environment by
/// default, so the fallbacks need `sudo -E` (or `sudo NYANPASU_…=…`) to reach
/// an elevated install.
#[derive(Debug, clap::Args)]
pub struct InstallCommand {
    /// The user who will run the service
    #[clap(long, env = "NYANPASU_USER")]
    user: String, // Should manual specify because the runner should be administrator/root
    /// The nyanpasu data directory
    #[clap(long, env = "NYANPASU_DATA_DIR")]
    nyanpasu_data_dir: PathBuf,
    /// The nyanpasu config directory
    #[clap(long, env = "NYANPASU_CONFIG_DIR")]
    nyanpasu_config_dir: PathBuf,
    /// The nyanpasu install directory, allowing to search the sidecar binary
    #[clap(long, env = "NYANPASU_APP_DIR")]
    nyanpasu_app_dir: PathBuf,
    /// How the installed service reaches the core's control plane
    #[clap(
        long,
        value_enum,
        default_value = "disable",
        env = "NYANPASU_LOCAL_IPC_POLICY"
    )]
    local_ipc_policy: LocalIpcPolicyArg,
}

pub fn install(ctx: InstallCommand) -> Result<(), CommandError> {
    let manager = crate::utils::get_service_manager()?;
    install_with(manager.as_ref(), ctx)
}

pub fn install_with(manager: &dyn ServiceManager, ctx: InstallCommand) -> Result<(), CommandError> {
    tracing::info!("nyanpasu data dir: {:?}", ctx.nyanpasu_data_dir);
    tracing::info!("nyanpasu config dir: {:?}", ctx.nyanpasu_config_dir);
    let label: ServiceLabel = SERVICE_LABEL.parse()?;
    // before we install the service, we need to check if the service is already installed
    if !matches!(
        crate::utils::service::status(manager, &label)?,
        ServiceStatus::NotInstalled
    ) {
        return Err(CommandError::ServiceAlreadyInstalled);
    }

    let service_data_dir = crate::utils::dirs::service_data_dir();
    let service_config_dir = crate::utils::dirs::service_config_dir();
    tracing::info!("suggested service data dir: {:?}", service_data_dir);
    tracing::info!("suggested service config dir: {:?}", service_config_dir);
    // copy nyanpasu service binary to the service data dir
    if !service_data_dir.exists() {
        std::fs::create_dir_all(&service_data_dir)?;
    }
    if !service_config_dir.exists() {
        std::fs::create_dir_all(&service_config_dir)?;
    }
    let binary_name = format!("{}{}", APP_NAME, std::env::consts::EXE_SUFFIX);
    #[cfg(not(target_os = "linux"))]
    let service_binary = service_data_dir.join(binary_name);
    #[cfg(target_os = "linux")]
    let service_binary = PathBuf::from("/usr/bin").join(binary_name);
    let current_binary = current_exe()?;
    // Prevent both src and target binary are the same
    // It possible happens when a app was installed by a linux package manager
    if current_binary != service_binary {
        tracing::info!("Copying service binary to: {:?}", service_binary);
        std::fs::copy(current_binary, &service_binary)?;
    }

    // create nyanpasu group to ensure share unix socket access
    #[cfg(not(windows))]
    {
        tracing::info!("checking nyanpasu group exists...");
        if !crate::utils::os::user::is_nyanpasu_group_exists() {
            tracing::info!("nyanpasu group not exists, creating...");
            crate::utils::os::user::create_nyanpasu_group()?;
        }
        tracing::info!("checking whether user is in nyanpasu group...");
        if !crate::utils::os::user::is_user_in_nyanpasu_group(&ctx.user) {
            tracing::info!("adding user to nyanpasu group...");
            crate::utils::os::user::add_user_to_nyanpasu_group(&ctx.user)?;
        }
    }
    tracing::info!("Working dir: {:?}", service_data_dir);
    let mut envs = Vec::new();
    #[cfg(windows)]
    {
        let rt = tokio::runtime::Runtime::new().unwrap();
        tracing::info!("Creating acl file...");
        rt.block_on(crate::utils::acl::create_acl_file())?;
        tracing::info!("Reading acl file...");
        let mut list =
            std::collections::BTreeSet::from_iter(rt.block_on(crate::utils::acl::read_acl_file())?);
        list.insert(ctx.user.clone());
        let list = list.into_iter().collect::<Vec<_>>();
        tracing::info!(list = ?list, "Writing acl file...");
        rt.block_on(crate::utils::acl::write_acl_file(list.as_slice()))?;
    }
    if let Ok(home) = std::env::var("HOME") {
        envs.push(("HOME".to_string(), home));
    }
    tracing::info!("Installing service...");
    manager.install(build_install_ctx(
        label.clone(),
        InstallPaths {
            program: service_binary,
            working_directory: service_data_dir,
        },
        &ctx,
        envs,
    ))?;
    // Confirm the service is installed
    if matches!(
        crate::utils::service::status(manager, &label)?,
        ServiceStatus::NotInstalled
    ) {
        tracing::error!("Service install failed");
        return Err(CommandError::Other(anyhow::anyhow!(
            "Service install failed"
        )));
    }
    tracing::info!("Service installed");
    Ok(())
}

/// The kebab-case CLI value the server side parses back.
///
/// Spelled out rather than read off `ValueEnum`: this string is written into the
/// persisted service definition, so it must not change as a side effect of a
/// derive-macro upgrade. `cmds::tests` pins the round trip.
pub(super) fn policy_value(policy: LocalIpcPolicyArg) -> &'static str {
    match policy {
        LocalIpcPolicyArg::Force => "force",
        LocalIpcPolicyArg::Prefer => "prefer",
        LocalIpcPolicyArg::Disable => "disable",
    }
}

/// The two paths the service definition is written with. Both are `PathBuf`
/// and were adjacent parameters, so a transposition compiles and is persisted
/// into the OS service manager: the daemon is registered to launch a directory
/// and to run out of its own binary.
struct InstallPaths {
    program: PathBuf,
    working_directory: PathBuf,
}

fn build_install_ctx(
    label: ServiceLabel,
    paths: InstallPaths,
    ctx: &InstallCommand,
    environment: Vec<(String, String)>,
) -> ServiceInstallCtx {
    ServiceInstallCtx {
        label,
        program: paths.program,
        args: vec![
            OsString::from("server"),
            OsString::from("--nyanpasu-data-dir"),
            ctx.nyanpasu_data_dir.clone().into(),
            OsString::from("--nyanpasu-config-dir"),
            ctx.nyanpasu_config_dir.clone().into(),
            OsString::from("--nyanpasu-app-dir"),
            ctx.nyanpasu_app_dir.clone().into(),
            OsString::from("--local-ipc-policy"),
            OsString::from(policy_value(ctx.local_ipc_policy)),
            OsString::from("--service"),
        ],
        contents: None,
        username: None, // because we just need to run the service as root
        working_directory: Some(paths.working_directory),
        environment: Some(environment),
        autostart: true,
        restart_policy: RestartPolicy::default(),
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;
    use crate::cmds::test_support::{MockServiceManager, label, status_call};

    #[test]
    fn install_rejects_a_running_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::Running]);
        let result = install_with(
            &manager,
            InstallCommand {
                user: "user".into(),
                nyanpasu_data_dir: "data".into(),
                nyanpasu_config_dir: "config".into(),
                nyanpasu_app_dir: "app".into(),
                local_ipc_policy: LocalIpcPolicyArg::Disable,
            },
        );

        assert!(matches!(result, Err(CommandError::ServiceAlreadyInstalled)));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn install_rejects_an_installed_but_stopped_service() {
        let manager = MockServiceManager::with_statuses([ServiceStatus::Stopped(None)]);
        let result = install_with(
            &manager,
            InstallCommand {
                user: "user".into(),
                nyanpasu_data_dir: "data".into(),
                nyanpasu_config_dir: "config".into(),
                nyanpasu_app_dir: "app".into(),
                local_ipc_policy: LocalIpcPolicyArg::Disable,
            },
        );

        assert!(matches!(result, Err(CommandError::ServiceAlreadyInstalled)));
        assert_eq!(manager.calls(), [status_call()]);
    }

    #[test]
    fn install_ctx_carries_the_label_program_and_server_args() {
        let ctx = InstallCommand {
            user: "user".into(),
            nyanpasu_data_dir: "data".into(),
            nyanpasu_config_dir: "config".into(),
            nyanpasu_app_dir: "app".into(),
            local_ipc_policy: LocalIpcPolicyArg::Disable,
        };
        let environment = vec![("HOME".into(), "home".into())];
        let install_ctx = build_install_ctx(
            label(),
            InstallPaths {
                program: PathBuf::from("program"),
                working_directory: PathBuf::from("working-directory"),
            },
            &ctx,
            environment.clone(),
        );

        assert_eq!(install_ctx.label, label());
        assert_eq!(install_ctx.program, PathBuf::from("program"));
        assert_eq!(
            install_ctx.args,
            [
                "server",
                "--nyanpasu-data-dir",
                "data",
                "--nyanpasu-config-dir",
                "config",
                "--nyanpasu-app-dir",
                "app",
                "--local-ipc-policy",
                "disable",
                "--service",
            ]
            .map(OsString::from)
            .to_vec()
        );
        assert_eq!(install_ctx.contents, None);
        assert_eq!(install_ctx.username, None);
        assert_eq!(
            install_ctx.working_directory,
            Some(PathBuf::from("working-directory"))
        );
        assert_eq!(install_ctx.environment, Some(environment));
        assert!(install_ctx.autostart);
        assert_eq!(install_ctx.restart_policy, RestartPolicy::default());
    }
}
