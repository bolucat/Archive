use nyanpasu_utils::process::{OrphanReapOutcome, reap_epoch_pid_file};

use crate::{Epoch, error::Error, runtime_store::RuntimeConfigStore, state::CoreState};

use super::{CoreManager, Ctrl, QuarantinedEpoch};

impl CoreManager {
    pub(super) fn latch_quarantine(&self, ctrl: &mut Ctrl, epoch: Epoch, error: Error) -> Error {
        record_quarantine(ctrl, epoch, error.to_string());
        let quarantine = quarantine_error(ctrl).expect("quarantine was just inserted");
        self.publish_terminal_error(&quarantine);
        error
    }

    /// Attempts identity-verified recovery of every uncertain epoch. Manager
    /// operations remain rejected until every quarantined process is proven
    /// dead and its artifacts are cleaned.
    pub async fn recover_quarantine(&self) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        if ctrl.quarantine.is_empty() {
            return Ok(());
        }
        let quarantined = ctrl.quarantine.clone();
        let mut failures = Vec::new();
        for entry in quarantined {
            if !entry.death_proven {
                let pid_path = self.inner.store.pid_path(entry.epoch);
                match reap_epoch_pid_file(
                    pid_path.as_std_path(),
                    self.inner.store.dir().as_std_path(),
                )
                .await
                {
                    Ok(OrphanReapOutcome::AlreadyExited | OrphanReapOutcome::Killed) => {
                        if let Some(quarantine) = ctrl
                            .quarantine
                            .iter_mut()
                            .find(|quarantine| quarantine.epoch == entry.epoch)
                        {
                            quarantine.death_proven = true;
                        }
                    }
                    Ok(OrphanReapOutcome::NotFound) => {
                        failures.push(format!(
                            "epoch {}: {}; authoritative epoch pid record is unavailable",
                            entry.epoch, entry.reason
                        ));
                        continue;
                    }
                    Err(error) => {
                        failures.push(format!(
                            "epoch {}: {}; recovery failed: {error}",
                            entry.epoch, entry.reason
                        ));
                        continue;
                    }
                }
            }

            match self.cleanup_epoch(entry.epoch).await {
                Ok(()) => ctrl
                    .quarantine
                    .retain(|quarantine| quarantine.epoch != entry.epoch),
                Err(error) => failures.push(format!(
                    "epoch {}: {}; artifact cleanup failed: {error}",
                    entry.epoch, entry.reason
                )),
            }
        }
        if !failures.is_empty() {
            // Every failure arm above leaves its entry in place: the retain
            // that removes an entry runs only after its cleanup succeeded.
            let first_epoch = ctrl
                .quarantine
                .first()
                .map(|entry| entry.epoch)
                .expect("a failed recovery leaves its epoch quarantined");
            let error = Error::ManagerQuarantined {
                epoch: first_epoch,
                reason: failures.join(" | "),
            };
            return Err(error);
        }
        self.inner
            .publish(CoreState::Stopped { reason: None }, None);
        // Every uncertain epoch is now proven dead; nothing may keep holding
        // the DNS override.
        self.dns_restore(&mut ctrl).await;
        Ok(())
    }
}

fn quarantine_error(ctrl: &Ctrl) -> Option<Error> {
    let first = ctrl.quarantine.first()?;
    let reason = if ctrl.quarantine.len() == 1 {
        first.reason.clone()
    } else {
        let epochs = ctrl
            .quarantine
            .iter()
            .map(|entry| entry.epoch.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("{}; additional uncertain epochs: {epochs}", first.reason)
    };
    Some(Error::ManagerQuarantined {
        epoch: first.epoch,
        reason,
    })
}

pub(super) fn record_quarantine(ctrl: &mut Ctrl, epoch: Epoch, reason: String) {
    if let Some(existing) = ctrl
        .quarantine
        .iter_mut()
        .find(|quarantine| quarantine.epoch == epoch)
    {
        existing.reason = reason;
    } else {
        ctrl.quarantine.push(QuarantinedEpoch {
            epoch,
            reason,
            death_proven: false,
        });
    }
}

pub(super) fn reject_quarantine(ctrl: &Ctrl) -> Result<(), Error> {
    match quarantine_error(ctrl) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

pub(super) async fn sweep_orphans(
    store: &RuntimeConfigStore,
    options: &crate::ManagerOptions,
) -> Result<u64, Error> {
    // Artifact numbers are read back from filenames, so they are not epochs:
    // a directory can carry `config-0.yaml`, and a zero that cannot be named
    // is a zero that leaks forever. Discovery and cleanup stay raw; only the
    // allocator seed crosses into the domain, one increment later.
    let discovered = store.artifact_epochs().await?;
    let max_seen = discovered.iter().copied().max().unwrap_or(0);
    for artifact in discovered {
        let pid_path = store.artifact_pid_path(artifact);
        if tokio::fs::try_exists(&pid_path).await? {
            reap_epoch_pid_file(pid_path.as_std_path(), store.dir().as_std_path()).await?;
        }
        if let Some(epoch) = Epoch::new(artifact) {
            cleanup_controller(options, epoch).await?;
        }
        store.cleanup_artifacts(artifact).await?;
    }
    Ok(max_seen)
}

pub(super) async fn cleanup_controller(
    options: &crate::ManagerOptions,
    epoch: Epoch,
) -> Result<(), Error> {
    if options.controller_dir.is_some() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            let endpoint = crate::config::managed_endpoint_path(
                options.controller_dir.as_deref().expect("checked above"),
                options.controller_template.as_deref(),
                epoch,
            )?;
            match tokio::fs::symlink_metadata(&endpoint).await {
                Ok(metadata) if metadata.file_type().is_socket() => {
                    tokio::fs::remove_file(endpoint).await?
                }
                Ok(_) => {
                    return Err(Error::InvalidManagerOptions(
                        "refusing to remove a non-socket controller".into(),
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod controller_tests {
    use super::*;

    #[tokio::test]
    async fn orphan_sweep_cleans_only_recorded_controller_sockets() {
        let temp = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).unwrap();
        let store = RuntimeConfigStore::new(root.join("runtime")).await.unwrap();
        let dir = root.join("controllers");
        std::fs::create_dir(&dir).unwrap();
        let options = crate::ManagerOptions {
            controller_dir: Some(dir.clone()),
            ..Default::default()
        };
        let endpoint =
            crate::config::managed_endpoint_path(&dir, None, Epoch::new(7).unwrap()).unwrap();
        let listener = std::os::unix::net::UnixListener::bind(&endpoint).unwrap();
        drop(listener);
        std::fs::write(store.dir().join("config-7.yaml"), "secret: private").unwrap();
        let foreign = dir.join("unrelated.sock");
        let _foreign = std::os::unix::net::UnixListener::bind(&foreign).unwrap();
        assert_eq!(sweep_orphans(&store, &options).await.unwrap(), 7);
        assert!(!std::path::Path::new(&endpoint).exists());
        assert!(foreign.exists());
        std::fs::write(&endpoint, "not a socket").unwrap();
        assert!(
            cleanup_controller(&options, Epoch::new(7).unwrap())
                .await
                .is_err()
        );
        assert!(std::path::Path::new(&endpoint).exists());
    }
}
