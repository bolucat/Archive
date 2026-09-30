use std::path::PathBuf;

use nyanpasu_ipc::client::shortcuts;
use semver::Version;
use tokio::task::spawn_blocking;

use crate::consts::APP_VERSION;

use super::CommandError;

#[derive(Debug, clap::Args)]
pub struct UpdateCommand {
    /// Report what an update would do and exit, without touching anything
    #[clap(long, default_value = "false")]
    pub(super) check: bool,

    /// Copy this binary over the installed service instead of the running one.
    /// The version comparison still uses the running binary's version.
    #[clap(long, value_name = "PATH")]
    from: Option<PathBuf>,

    /// Bind native core data to this desktop user when upgrading an older installation
    #[cfg(unix)]
    #[clap(long, requires = "nyanpasu_data_dir")]
    user: Option<String>,
    /// The desktop data directory belonging to --user
    #[cfg(unix)]
    #[clap(long, requires = "user")]
    nyanpasu_data_dir: Option<PathBuf>,
}

/// What `update` would do. Split out of [`update`] so the branch table can be
/// tested without a service, a filesystem or an IPC endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdatePlan {
    /// No installed binary: copy the source into place, nothing to stop.
    Seed,
    /// The service is not answering: overwrite the binary where it lies.
    ReplaceOffline,
    /// The installed service is older: stop, overwrite, start again.
    StopReplaceStart,
    /// The binary is current, but the service must reload its owner binding.
    Restart,
    /// The installed service is not older than the source: do nothing.
    UpToDate,
}

impl UpdatePlan {
    /// The line `--check` prints.
    fn summary(self) -> &'static str {
        match self {
            UpdatePlan::Seed => "would install: no service binary is present",
            UpdatePlan::ReplaceOffline => "would replace: the service is not answering",
            UpdatePlan::StopReplaceStart => {
                "would update: stop the service, replace the binary, start it again"
            }
            UpdatePlan::Restart => "would restart: reload the native store owner binding",
            UpdatePlan::UpToDate => {
                "up to date: the installed service is not older than the running binary"
            }
        }
    }
}

fn plan_update(
    binary_exists: bool,
    source: &Version,
    installed: Option<&Version>,
    refresh_owner: bool,
) -> UpdatePlan {
    if !binary_exists {
        return UpdatePlan::Seed;
    }
    match installed {
        None => UpdatePlan::ReplaceOffline,
        Some(installed) if source > installed => UpdatePlan::StopReplaceStart,
        Some(_) if refresh_owner => UpdatePlan::Restart,
        Some(_) => UpdatePlan::UpToDate,
    }
}

/// The file a copy branch reads from: `--from` if given, else the running
/// binary, resolved only at the moment of the copy.
fn source_path(explicit: &Option<PathBuf>) -> Result<PathBuf, CommandError> {
    match explicit {
        Some(path) => Ok(path.clone()),
        None => Ok(std::env::current_exe()?),
    }
}

pub async fn update(ctx: UpdateCommand) -> Result<(), CommandError> {
    tracing::info!("Checking for updates...");
    // `--from` is validated eagerly; the running binary is resolved lazily,
    // immediately before a copy, exactly as the pre-S5 code did — the
    // up-to-date branch must never touch current_exe().
    let explicit_source = match ctx.from {
        Some(from) => {
            if !from.exists() {
                return Err(CommandError::Other(anyhow::anyhow!(
                    "update source does not exist: {}",
                    from.display()
                )));
            }
            Some(from)
        }
        None => None,
    };
    let service_data_dir = crate::utils::dirs::service_data_dir();
    tracing::info!("Service data dir: {:?}", service_data_dir);
    tracing::info!("Client version: {}", APP_VERSION);
    let service_binary = crate::utils::dirs::service_binary_path();
    let binary_exists = service_binary.exists();
    let client_version = Version::parse(APP_VERSION).unwrap();
    // Only ask the running service for its version when there is a binary to
    // compare against — the pre-S5 code did not probe in that case either.
    let installed = if binary_exists {
        tracing::info!("Get server version...");
        let client = shortcuts::Client::service_default();
        client
            .status()
            .await
            .ok()
            .map(|status| Version::parse(&status.version).unwrap())
    } else {
        None
    };
    #[cfg(unix)]
    let refresh_owner = ctx.user.is_some();
    #[cfg(not(unix))]
    let refresh_owner = false;
    let plan = plan_update(
        binary_exists,
        &client_version,
        installed.as_ref(),
        refresh_owner,
    );

    if ctx.check {
        let source = source_path(&explicit_source)?;
        println!("service binary: {}", service_binary.display());
        println!("update source: {}", source.display());
        println!("comparison version (running binary): {APP_VERSION}");
        match installed.as_ref() {
            Some(installed) => println!("installed version: {installed}"),
            None => println!("installed version: unknown (the service did not answer)"),
        }
        #[cfg(unix)]
        if let (Some(user), Some(data)) = (&ctx.user, &ctx.nyanpasu_data_dir) {
            println!("would bind native store owner: {user} ({})", data.display());
        }
        println!("{}", plan.summary());
        return Ok(());
    }

    #[cfg(unix)]
    if let (Some(user), Some(data)) = (&ctx.user, &ctx.nyanpasu_data_dir) {
        crate::utils::native_store_owner::FsOwnerBindingStore::new(
            crate::utils::dirs::service_config_dir(),
            data,
        )
        .save(user)?;
    }
    match plan {
        UpdatePlan::Seed => {
            tracing::info!("Service binary not found, copying from current binary directly...");
            tokio::fs::copy(source_path(&explicit_source)?, &service_binary).await?;
        }
        UpdatePlan::ReplaceOffline => {
            tracing::info!("Server is stopped or not installed, replacing the binary directly...");
            tokio::fs::copy(source_path(&explicit_source)?, &service_binary).await?;
        }
        UpdatePlan::StopReplaceStart => {
            tracing::info!("Client version is newer than server version, prepare updating...");
            tracing::info!("Stopping the service before updating...");
            spawn_blocking(super::stop::stop).await??; // stop the service before updating
            tracing::info!("Copying the binary...");
            tokio::fs::copy(source_path(&explicit_source)?, &service_binary).await?;
            tracing::info!("Service binary updated, starting the service...");
            spawn_blocking(super::start::start).await??; // start the service after updating
        }
        UpdatePlan::Restart => {
            spawn_blocking(super::stop::stop).await??;
            spawn_blocking(super::start::start).await??;
        }
        UpdatePlan::UpToDate => {
            tracing::info!("Client version is the same as server version, no need to update.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(raw: &str) -> Version {
        Version::parse(raw).unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn owner_binding_arguments_must_be_supplied_together() {
        use crate::cmds::Cli;
        use clap::Parser;

        for args in [
            vec!["nyanpasu-service", "update", "--user", "alice"],
            vec!["nyanpasu-service", "update", "--nyanpasu-data-dir", "data"],
        ] {
            let error = Cli::try_parse_from(args).err().unwrap();
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::MissingRequiredArgument
            );
        }
        for check in [false, true] {
            let mut args = vec![
                "nyanpasu-service",
                "update",
                "--user",
                "alice",
                "--nyanpasu-data-dir",
                "data",
            ];
            if check {
                args.push("--check");
            }
            assert!(Cli::try_parse_from(args).is_ok());
        }
    }

    #[test]
    fn the_update_plan_reproduces_the_legacy_branches() {
        let source = version("1.4.5");
        assert_eq!(plan_update(false, &source, None, false), UpdatePlan::Seed);
        assert_eq!(
            plan_update(false, &source, Some(&version("1.4.5")), false),
            UpdatePlan::Seed
        );
        assert_eq!(
            plan_update(true, &source, None, false),
            UpdatePlan::ReplaceOffline
        );
        assert_eq!(
            plan_update(true, &source, Some(&version("1.4.4")), false),
            UpdatePlan::StopReplaceStart
        );
        assert_eq!(
            plan_update(true, &source, Some(&version("1.4.5")), false),
            UpdatePlan::UpToDate
        );
        assert_eq!(
            plan_update(true, &source, Some(&version("1.5.0")), false),
            UpdatePlan::UpToDate
        );
    }

    #[test]
    fn updating_the_owner_restarts_a_current_binary_without_downgrading_it() {
        let source = version("1.4.5");
        assert_eq!(plan_update(false, &source, None, true), UpdatePlan::Seed);
        assert_eq!(
            plan_update(true, &source, None, true),
            UpdatePlan::ReplaceOffline
        );
        assert_eq!(
            plan_update(true, &source, Some(&version("1.4.4")), true),
            UpdatePlan::StopReplaceStart
        );
        for installed in [version("1.4.5"), version("1.5.0")] {
            let plan = plan_update(true, &source, Some(&installed), true);
            assert_eq!(plan, UpdatePlan::Restart);
            assert!(plan.summary().contains("would restart"));
        }
    }
}
