//! The unified mutating convergence entry (amendment A2): one command that
//! makes the runtime match the desired spec. Start, patch, reload, restart,
//! and switch are classifications this transaction derives — never caller
//! choices — and the config check runs inside it, before any commit point.

use crate::{error::Error, spec::InstanceSpec, state::RevisionId};

use super::{ApplyOutcome, CoreManager, abort_and_await, quarantine::reject_quarantine};

impl CoreManager {
    /// Converges the runtime toward `spec`.
    ///
    /// Transaction envelope, mapped to the ten phases of the 2026-08-12
    /// amendment A3 (admission by queue and idempotency live one layer up, in
    /// the control executor):
    ///
    /// 1. admission — the quarantine gate rejects everything until recovery;
    /// 2. CAS — `expected_applied` must name the actually applied revision
    ///    (`None` claims nothing is applied; a mismatch changes nothing);
    /// 3. internal check — parse ([`Error::InvalidConfig`]) and core dry run
    ///    ([`Error::ConfigCheckFailed`]) inside the prepare step, before any
    ///    commit point: a rejection is a clean abort with the previous runtime
    ///    untouched;
    /// 4. stage / classify / execute / verify / fallback / publish — the
    ///    existing apply, switch, and start transactions, every stop proven
    ///    by [`stop_and_confirm_dead`](crate::runtime::RuntimeInstance) or the
    ///    manager latches quarantine.
    ///
    /// [`ApplyOutcome::RolledBack`] is a *successfully completed* transaction
    /// whose desired config did not take effect; the actual revision rides in
    /// the outcome.
    pub async fn reconcile(
        &self,
        spec: InstanceSpec,
        expected_applied: Option<RevisionId>,
    ) -> Result<ApplyOutcome, Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        reject_quarantine(&ctrl)?;
        let result = self
            .reconcile_locked(&mut ctrl, spec, expected_applied)
            .await;
        // DNS rides the same transaction (fixed converge tail): applied while
        // the desired runtime is up, restored when nothing survived.
        //
        // A rejected transaction is the exception, and only when it changed
        // nothing: a CAS conflict or an invalid config leaves the previous
        // runtime alive and untouched, so touching DNS would be a side effect
        // of a refusal. A transaction that failed *and* left nothing running
        // still converges, because that is the only place a restore can undo
        // an override still pointing at a dead core.
        let runtime_alive = ctrl
            .current
            .as_ref()
            .is_some_and(|active| !active.instance.state().borrow().state.is_terminal());
        if result.is_ok() || !runtime_alive {
            self.dns_converge(&mut ctrl).await;
        }
        result
    }

    async fn reconcile_locked(
        &self,
        ctrl: &mut super::Ctrl,
        spec: InstanceSpec,
        expected_applied: Option<RevisionId>,
    ) -> Result<ApplyOutcome, Error> {
        let running = ctrl
            .current
            .as_ref()
            .is_some_and(|active| !active.instance.state().borrow().state.is_terminal());
        if running {
            return self.apply_locked(ctrl, spec, expected_applied).await;
        }

        // Nothing is effectively applied — a crashed epoch's revision is not
        // an applied revision. A caller that believes one is applied must
        // learn the truth before anything happens.
        if let Some(expected) = expected_applied {
            return Err(Error::RevisionConflict {
                expected,
                actual: None,
            });
        }

        // Same stale-epoch hygiene as `start`: prove death before reuse.
        if let Some(stale) = ctrl.current.take() {
            abort_and_await(stale.forwarder).await;
            let epoch = stale.instance.epoch();
            if let Err(error) = stale
                .instance
                .stop_and_confirm_dead(self.inner.options.stop_timeout)
                .await
            {
                if matches!(error, Error::StopUnconfirmed(_)) {
                    return Err(self.latch_quarantine(ctrl, epoch, error));
                }
                return Err(error);
            }
            self.inner.store.cleanup_epoch(epoch).await?;
        }
        self.start_locked(ctrl, spec).await?;
        let revision = ctrl
            .current
            .as_ref()
            .expect("start_locked installed the active runtime")
            .revision
            .clone();
        Ok(ApplyOutcome::Started { revision })
    }
}
