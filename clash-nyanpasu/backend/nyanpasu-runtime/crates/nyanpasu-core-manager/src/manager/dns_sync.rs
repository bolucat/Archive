//! DNS override convergence at the fixed phases of the v2 transaction entry
//! points (`reconcile` / `stop` / `shutdown` / `recover_quarantine`).
//!
//! The legacy v1 methods (`start` / `apply_config` / `switch` / `restart`)
//! deliberately do *not* converge DNS: under v1 the app process owns DNS
//! behavior, and that stays untouched until the bridge-cleanup stage removes
//! v1 entirely.
//!
//! Failure policy: a DNS failure never fails the core lifecycle transaction.
//! It is logged, the record is kept (so a later restore can still undo it),
//! and the caller's outcome is unchanged. Read-back lives inside the
//! controller; an `Err` here never implies the side effect is absent.
//!
//! One asymmetry, and it is the crash-recovery guarantee rather than a
//! preference: applying an override is refused when the record cannot be
//! written, because an override nothing recorded is an override nothing can
//! undo. Restoring is never blocked by a persistence failure — it is the safe
//! direction.

use std::{io, time::Duration};

use crate::{
    dns::{DnsController, DnsOverrideRecord, DnsOverrideState},
    runtime_store::RuntimeConfigStore,
};

use super::{CoreManager, Ctrl};

const RECORD_FILE: &str = "dns-override.json";
const RECORD_STAGING_FILE: &str = "dns-override.json.tmp";

/// Startup orphan reconcile: a record left behind by a dead process is
/// restored (a no-op when its side effect never landed) and cleared. Runs
/// before the manager exists, so it takes the store directly.
///
/// Bounded by the same `dns_timeout` as the converge tail: this runs inside
/// manager construction, and a platform command that never returns would hang
/// the daemon before it ever serves.
pub(super) async fn reconcile_orphan_record(
    store: &RuntimeConfigStore,
    dns: Option<&dyn DnsController>,
    dns_timeout: Duration,
) {
    let path = store.dir().join(RECORD_FILE);
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return;
    };
    let record = match serde_json::from_slice::<DnsOverrideRecord>(&bytes) {
        Ok(record) => record,
        Err(error) => {
            tracing::warn!("unreadable orphan dns override record; keeping the file: {error}");
            return;
        }
    };
    let Some(dns) = dns else {
        tracing::warn!(
            "an orphan dns override record exists but no dns controller is injected; keeping it"
        );
        return;
    };
    match tokio::time::timeout(dns_timeout, dns.restore(&record)).await {
        Ok(Ok(())) => {
            if let Err(error) = tokio::fs::remove_file(&path).await {
                tracing::warn!("failed to clear the restored dns override record: {error}");
            }
        }
        Ok(Err(error)) => {
            tracing::warn!("orphan dns override restore failed; keeping the record: {error}");
        }
        Err(_) => tracing::warn!(
            "orphan dns override restore timed out after {dns_timeout:?}; keeping the record"
        ),
    }
}

impl CoreManager {
    /// Converge tail: apply the override when a runtime is up and wants one,
    /// restore otherwise. Idempotent; called with the control lock held.
    pub(super) async fn dns_converge(&self, ctrl: &mut Ctrl) {
        let Some(dns) = self.inner.dns.clone() else {
            return;
        };
        let desired = ctrl.current.as_ref().and_then(|active| {
            let running = !active.instance.state().borrow().state.is_terminal();
            running
                .then(|| {
                    dns.desired(&active.effective_document)
                        .map(|intent| (intent, active.instance.epoch()))
                })
                .flatten()
        });
        let Some((intent, epoch)) = desired else {
            self.dns_restore(ctrl).await;
            return;
        };

        // The baseline is captured once, on the converge that first took
        // ownership. Every later converge re-applies over an interface that
        // already carries *our* override, so the controller's read-back would
        // report the override itself as the thing to restore to.
        //
        // A non-empty `interface` is the discriminator, because only a
        // successful read-back supplies one -- the pre-record written before
        // the first apply deliberately leaves it empty. An empty `previous` is
        // a legitimate baseline (an interface with no resolvers), and
        // `RestorePending` still owns the baseline: it says the restore is
        // uncertain, not that what we recorded stopped being true.
        let baseline = ctrl
            .dns_record
            .as_ref()
            .filter(|record| !record.interface.is_empty())
            .map(|record| (record.interface.clone(), record.previous.clone()));

        // Persist a pre-record before the side effect: a crash between the
        // two leaves an orphan whose restore is a read-back no-op.
        let pre_record = DnsOverrideRecord {
            interface: baseline
                .as_ref()
                .map_or_else(String::new, |(interface, _)| interface.clone()),
            previous: baseline
                .as_ref()
                .map_or_else(Vec::new, |(_, previous)| previous.clone()),
            applied: intent.servers.clone(),
            runtime_epoch: epoch,
            owner_generation: None,
            state: DnsOverrideState::Applied,
        };
        if let Err(error) = self.persist_dns_record(ctrl, Some(pre_record)).await {
            tracing::warn!("the dns override record is unwritable; skipping the override: {error}");
            return;
        }

        let dns_timeout = self.inner.options.dns_timeout;
        match tokio::time::timeout(dns_timeout, dns.apply(&intent, epoch)).await {
            Ok(Ok(mut record)) => {
                if let Some((interface, previous)) = baseline {
                    if record.interface != interface {
                        tracing::warn!(
                            "the dns interface changed under the override; keeping the recorded baseline"
                        );
                        return;
                    }
                    record.previous = previous;
                }
                if let Err(error) = self.persist_dns_record(ctrl, Some(record)).await {
                    tracing::warn!(
                        "failed to persist the dns read-back; keeping the pre-record: {error}"
                    );
                }
            }
            // The side effect is uncertain either way, so the pre-record stays:
            // a later restore must still be able to undo it.
            Ok(Err(error)) => {
                tracing::warn!("dns override apply failed; keeping the pre-record: {error}")
            }
            Err(_) => tracing::warn!(
                "dns override apply timed out after {dns_timeout:?}; keeping the pre-record"
            ),
        }
    }

    /// Restore head/tail: undo the recorded override and clear the record.
    /// Called at the head of stop/shutdown (so resolution never points at a
    /// core being torn down) and from the converge tail when nothing runs.
    pub(super) async fn dns_restore(&self, ctrl: &mut Ctrl) {
        let Some(dns) = self.inner.dns.clone() else {
            return;
        };
        let Some(record) = ctrl.dns_record.clone() else {
            return;
        };
        let mut pending = record.clone();
        pending.state = DnsOverrideState::RestorePending;
        if let Err(error) = self.persist_dns_record(ctrl, Some(pending)).await {
            // Undoing an override is the safe direction; a record that cannot
            // be updated must not keep the system pointed at a dead core.
            tracing::warn!("failed to mark the dns override restore-pending: {error}");
        }
        let dns_timeout = self.inner.options.dns_timeout;
        match tokio::time::timeout(dns_timeout, dns.restore(&record)).await {
            Ok(Ok(())) => {
                if let Err(error) = self.persist_dns_record(ctrl, None).await {
                    tracing::warn!("dns was restored but its record could not be cleared: {error}");
                }
            }
            Ok(Err(error)) => {
                tracing::warn!("dns override restore failed; keeping the record: {error}")
            }
            Err(_) => tracing::warn!(
                "dns override restore timed out after {dns_timeout:?}; keeping the record"
            ),
        }
    }

    /// Record-before-side-effect persistence, durably: a torn write would
    /// leave an orphan record that cannot be parsed and therefore cannot be
    /// undone, and an unsynced one could vanish in the crash it exists to
    /// survive. Same protocol as [`RuntimeConfigStore`](crate::runtime_store):
    /// write, flush and `sync_all` a staging file, publish it atomically, then
    /// sync the directory entry.
    ///
    /// In-memory state advances once the record is *visible* -- after the
    /// rename, not after the directory sync. A failed directory sync leaves a
    /// readable record behind, so pretending we do not own it would be the
    /// bigger lie; it is reported and the ownership stands.
    async fn persist_dns_record(
        &self,
        ctrl: &mut Ctrl,
        record: Option<DnsOverrideRecord>,
    ) -> io::Result<()> {
        use nyanpasu_utils::io::atomic_fs;
        use tokio::io::AsyncWriteExt;

        let dir = self.inner.store.dir();
        let path = dir.join(RECORD_FILE);
        match &record {
            Some(entry) => {
                let staging = dir.join(RECORD_STAGING_FILE);
                let result = async {
                    let bytes = serde_json::to_vec_pretty(entry).map_err(io::Error::other)?;
                    let mut file = tokio::fs::File::create(&staging).await?;
                    file.write_all(&bytes).await?;
                    file.flush().await?;
                    file.sync_all().await?;
                    drop(file);
                    // Same split as the runtime config store: `atomic_replace`
                    // requires an existing target on Windows, `atomic_move_new`
                    // requires an absent one. The runtime directory is held
                    // under an ownership lock, so nothing else publishes here.
                    // A `try_exists` failure is propagated rather than read as
                    // "absent": we cannot publish safely into a directory we
                    // cannot stat.
                    if tokio::fs::try_exists(&path).await? {
                        atomic_fs::atomic_replace(&staging, &path).await
                    } else {
                        atomic_fs::atomic_move_new(&staging, &path).await
                    }
                    .map_err(io::Error::other)
                }
                .await;
                if let Err(error) = result {
                    let _ = tokio::fs::remove_file(&staging).await;
                    return Err(error);
                }
                if let Err(error) = atomic_fs::sync_dir(dir).await {
                    tracing::warn!(
                        "the dns override record is published but its directory entry is unsynced: {error}"
                    );
                }
            }
            None => match tokio::fs::remove_file(&path).await {
                Ok(()) => {
                    if let Err(error) = atomic_fs::sync_dir(dir).await {
                        tracing::warn!(
                            "the dns override record is removed but its directory entry is unsynced: {error}"
                        );
                    }
                }
                Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
                Err(_) => {}
            },
        }
        ctrl.dns_record = record;
        Ok(())
    }
}
