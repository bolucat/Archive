use std::path::PathBuf;

use nyanpasu_ipc::client::shortcuts;
use semver::Version;
use tokio::task::spawn_blocking;

use crate::consts::{APP_NAME, APP_VERSION};

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
            UpdatePlan::UpToDate => {
                "up to date: the installed service is not older than the running binary"
            }
        }
    }
}

fn plan_update(binary_exists: bool, source: &Version, installed: Option<&Version>) -> UpdatePlan {
    if !binary_exists {
        return UpdatePlan::Seed;
    }
    match installed {
        None => UpdatePlan::ReplaceOffline,
        Some(installed) if source > installed => UpdatePlan::StopReplaceStart,
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
    let service_binary =
        service_data_dir.join(format!("{}{}", APP_NAME, std::env::consts::EXE_SUFFIX));
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
    let plan = plan_update(binary_exists, &client_version, installed.as_ref());

    if ctx.check {
        let source = source_path(&explicit_source)?;
        println!("service binary: {}", service_binary.display());
        println!("update source: {}", source.display());
        println!("comparison version (running binary): {APP_VERSION}");
        match installed.as_ref() {
            Some(installed) => println!("installed version: {installed}"),
            None => println!("installed version: unknown (the service did not answer)"),
        }
        println!("{}", plan.summary());
        return Ok(());
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

    #[test]
    fn the_update_plan_reproduces_the_legacy_branches() {
        let source = version("1.4.5");
        assert_eq!(plan_update(false, &source, None), UpdatePlan::Seed);
        assert_eq!(
            plan_update(false, &source, Some(&version("1.4.5"))),
            UpdatePlan::Seed
        );
        assert_eq!(plan_update(true, &source, None), UpdatePlan::ReplaceOffline);
        assert_eq!(
            plan_update(true, &source, Some(&version("1.4.4"))),
            UpdatePlan::StopReplaceStart
        );
        assert_eq!(
            plan_update(true, &source, Some(&version("1.4.5"))),
            UpdatePlan::UpToDate
        );
        assert_eq!(
            plan_update(true, &source, Some(&version("1.5.0"))),
            UpdatePlan::UpToDate
        );
    }
}
