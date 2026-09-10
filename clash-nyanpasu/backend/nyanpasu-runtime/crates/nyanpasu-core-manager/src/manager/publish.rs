use crate::{
    epoch::Epoch,
    error::Error,
    runtime::RuntimeInstance,
    state::{
        CoreState, CoreStatus, HealthStatus, InstanceState, InstanceStatus, SpecSummary,
        StopReason, now_ms,
    },
};

use super::{Active, CoreManager, Ctrl, EpochPlan, Inner};

impl Inner {
    /// `None` publishes a state with no epoch behind it (stopped, quarantined).
    pub(super) fn publish(&self, state: CoreState, plan: Option<&EpochPlan>) {
        self.status_tx.send_modify(|status| {
            let lifecycle_changed = status.state != state;
            let health = default_health_for_state(status.health.as_ref(), &state);
            status.state = state;
            status.instance_id = None;
            status.health = health;
            status.spec = plan.map(spec_summary);
            status.controller = plan.map(|plan| plan.controller.host.clone());
            status.revision = plan.map(|plan| plan.revision.clone());
            if lifecycle_changed {
                status.changed_at = now_ms();
            }
        });
    }

    pub(super) fn publish_active(&self, active: &Active, state: CoreState) {
        self.publish_instance(active.instance.as_ref(), state, &active.plan);
    }

    /// Health and controller still come from the live instance; the epoch's own
    /// description comes from its plan.
    pub(super) fn publish_instance(
        &self,
        instance: &dyn RuntimeInstance,
        state: CoreState,
        plan: &EpochPlan,
    ) {
        let snapshot = instance.state().borrow().clone();
        if matches!(state, CoreState::Running { .. })
            && let Some(instance_id) = snapshot.instance_id
        {
            self.config_commits
                .send_replace(Some(crate::EffectiveConfigSnapshot {
                    instance_id,
                    revision: plan.revision.clone(),
                    config: std::sync::Arc::new(plan.effective_document.clone()),
                }));
        }
        let health = snapshot.health;
        self.status_tx.send_modify(|status| {
            let lifecycle_changed = status.state != state;
            status.state = state;
            status.instance_id = snapshot.instance_id;
            status.health = health;
            status.spec = Some(spec_summary(plan));
            status.controller = Some(instance.controller().host.clone());
            status.revision = Some(plan.revision.clone());
            if lifecycle_changed {
                status.changed_at = now_ms();
            }
        });
    }

    pub(super) fn publish_epoch_status(&self, epoch: Epoch, instance: InstanceStatus) {
        self.status_tx
            .send_if_modified(|status| apply_epoch_status(status, epoch, &instance));
    }
}

fn default_health_for_state(
    previous: Option<&HealthStatus>,
    state: &CoreState,
) -> Option<HealthStatus> {
    let target = match state {
        CoreState::Starting { .. } | CoreState::Restarting { .. } | CoreState::Switching { .. } => {
            crate::state::HealthState::Starting
        }
        CoreState::Running { .. } => crate::state::HealthState::Healthy,
        CoreState::Stopping { .. } | CoreState::Stopped { .. } => return None,
    };
    let mut health = HealthStatus::starting();
    health.state = target;
    if let Some(previous) = previous.filter(|status| status.state == target) {
        health.changed_at = previous.changed_at;
        health.consecutive_failures = previous.consecutive_failures;
        health.last_error.clone_from(&previous.last_error);
        health.last_success_at = previous.last_success_at;
    }
    Some(health)
}

fn apply_epoch_status(status: &mut CoreStatus, epoch: Epoch, instance: &InstanceStatus) -> bool {
    if status.revision.as_ref().map(|revision| revision.epoch) != Some(epoch) {
        return false;
    }
    let state = instance_core_state(epoch, &instance.state);
    let lifecycle_changed = status.state != state;
    let health_changed = status.health != instance.health;
    if !lifecycle_changed && !health_changed && status.instance_id == instance.instance_id {
        return false;
    }
    status.state = state;
    status.health = instance.health.clone();
    status.instance_id = instance.instance_id;
    if lifecycle_changed {
        status.changed_at = now_ms();
    }
    true
}

fn spec_summary(plan: &EpochPlan) -> SpecSummary {
    SpecSummary {
        kind: plan.source_spec.core.kind,
        config_path: plan.source_spec.config_path.clone(),
        capabilities: plan.capabilities.iter().collect(),
        runtime_features: plan.runtime_features.iter().collect(),
    }
}

impl CoreManager {
    pub(super) fn publish_terminal_error(&self, error: &Error) {
        self.inner.publish(
            CoreState::Stopped {
                reason: Some(StopReason::Error(error.to_string())),
            },
            None,
        );
    }

    pub(super) fn republish_retained(&self, ctrl: &Ctrl) {
        let Some(active) = ctrl.current.as_ref() else {
            return;
        };
        let state = instance_core_state(
            active.instance.epoch(),
            &active.instance.state().borrow().state,
        );
        self.inner.publish_active(active, state);
    }
}

pub(super) fn instance_core_state(epoch: Epoch, state: &InstanceState) -> CoreState {
    match state {
        InstanceState::Starting => CoreState::Starting { epoch },
        InstanceState::Running { pid } => CoreState::Running { epoch, pid: *pid },
        InstanceState::Restarting { attempt } => CoreState::Restarting {
            epoch,
            attempt: *attempt,
        },
        InstanceState::Stopping => CoreState::Stopping { epoch },
        InstanceState::Stopped(reason) => CoreState::Stopped {
            reason: Some(reason.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::epoch::epoch;
    use tokio::sync::watch;

    use super::*;
    use crate::state::{ConfigRevision, InstanceState, InstanceStatus, StopReason};

    #[test]
    fn old_epoch_events_cannot_overwrite_new_epoch_status() {
        let mut status = CoreStatus::initial();
        status.revision = Some(ConfigRevision {
            epoch: epoch(9),
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-9.yaml".into(),
        });
        status.state = CoreState::Running {
            epoch: epoch(9),
            pid: 90,
        };
        for stale in [
            CoreState::Running {
                epoch: epoch(8),
                pid: 80,
            },
            CoreState::Restarting {
                epoch: epoch(8),
                attempt: 2,
            },
            CoreState::Stopped {
                reason: Some(StopReason::Finished),
            },
        ] {
            let stale_status = InstanceStatus {
                instance_id: None,
                state: match stale {
                    CoreState::Running { pid, .. } => InstanceState::Running { pid },
                    CoreState::Restarting { attempt, .. } => InstanceState::Restarting { attempt },
                    CoreState::Stopped { reason } => {
                        InstanceState::Stopped(reason.unwrap_or(StopReason::Finished))
                    }
                    _ => unreachable!(),
                },
                health: None,
            };
            assert!(!apply_epoch_status(&mut status, epoch(8), &stale_status));
            assert!(matches!(
                status.state,
                CoreState::Running { epoch: observed, pid: 90 } if observed == epoch(9)
            ));
        }
    }

    #[test]
    fn stale_epoch_status_neither_mutates_nor_wakes_watchers() {
        let mut status = CoreStatus::initial();
        status.revision = Some(ConfigRevision {
            epoch: epoch(9),
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-9.yaml".into(),
        });
        status.state = CoreState::Running {
            epoch: epoch(9),
            pid: 90,
        };
        let (tx, rx) = watch::channel(status);
        let stale = InstanceStatus {
            instance_id: None,
            state: InstanceState::Running { pid: 80 },
            health: Some(HealthStatus::starting()),
        };

        let sent = tx.send_if_modified(|status| apply_epoch_status(status, epoch(8), &stale));

        assert!(!sent);
        assert!(!rx.has_changed().unwrap());
        assert!(matches!(
            rx.borrow().state,
            CoreState::Running { epoch: observed, pid: 90 } if observed == epoch(9)
        ));
    }

    #[test]
    fn pure_health_transition_preserves_lifecycle_changed_at() {
        let mut status = CoreStatus::initial();
        status.revision = Some(ConfigRevision {
            epoch: epoch(3),
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-3.yaml".into(),
        });
        status.state = CoreState::Running {
            epoch: epoch(3),
            pid: 30,
        };
        status.changed_at = 7;
        let mut health = HealthStatus::starting();
        health.state = crate::state::HealthState::Unhealthy;
        let instance = InstanceStatus {
            instance_id: None,
            state: InstanceState::Running { pid: 30 },
            health: Some(health.clone()),
        };

        assert!(apply_epoch_status(&mut status, epoch(3), &instance));
        assert_eq!(status.changed_at, 7);
        assert_eq!(status.health, Some(health));
    }
    #[test]
    fn process_identity_change_wakes_watchers_even_if_pid_and_epoch_are_reused() {
        let mut status = CoreStatus::initial();
        status.state = CoreState::Running {
            epoch: epoch(3),
            pid: 30,
        };
        status.instance_id = Some(uuid::Uuid::new_v4());
        status.revision = Some(ConfigRevision {
            epoch: epoch(3),
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-3.yaml".into(),
        });
        let replacement = InstanceStatus {
            instance_id: Some(uuid::Uuid::new_v4()),
            state: InstanceState::Running { pid: 30 },
            health: None,
        };
        let (tx, rx) = watch::channel(status);
        assert!(tx.send_if_modified(|status| apply_epoch_status(status, epoch(3), &replacement)));
        assert!(rx.has_changed().unwrap());
        assert_eq!(rx.borrow().instance_id, replacement.instance_id);
    }
}
