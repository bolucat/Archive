use nyanpasu_utils::process::{OrphanReapOutcome, reap_epoch_pid_file};

use crate::{error::Error, runtime_store::RuntimeConfigStore, state::CoreState};

use super::{CoreManager, Ctrl, QuarantinedEpoch};

impl CoreManager {
    pub(super) fn latch_quarantine(&self, ctrl: &mut Ctrl, epoch: u64, error: Error) -> Error {
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

            match self.inner.store.cleanup_epoch(entry.epoch).await {
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
            let first_epoch = ctrl
                .quarantine
                .first()
                .map(|entry| entry.epoch)
                .unwrap_or_default();
            let error = Error::ManagerQuarantined {
                epoch: first_epoch,
                reason: failures.join(" | "),
            };
            return Err(error);
        }
        self.inner
            .publish(CoreState::Stopped { reason: None }, None, None, None);
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

pub(super) fn record_quarantine(ctrl: &mut Ctrl, epoch: u64, reason: String) {
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

pub(super) async fn sweep_orphans(store: &RuntimeConfigStore) -> Result<u64, Error> {
    let epochs = store.artifact_epochs().await?;
    let max_epoch = epochs.iter().copied().max().unwrap_or(0);
    for epoch in epochs {
        let pid_path = store.pid_path(epoch);
        if tokio::fs::try_exists(&pid_path).await? {
            reap_epoch_pid_file(pid_path.as_std_path(), store.dir().as_std_path()).await?;
        }
        store.cleanup_epoch(epoch).await?;
    }
    Ok(max_epoch)
}
