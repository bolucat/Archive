use crate::{
    Feature, RuntimeFeature,
    error::Error,
    runtime::RuntimeInstance,
    spec::InstanceSpec,
    state::{
        ConfigRevision, CoreState, CoreStatus, HealthStatus, InstanceState, InstanceStatus,
        SpecSummary, StopReason, now_ms,
    },
};

use super::{Active, CoreManager, Ctrl, Inner};

impl Inner {
    pub(super) fn publish(
        &self,
        state: CoreState,
        spec: Option<SpecSummary>,
        controller: Option<clash_api::Host>,
        revision: Option<ConfigRevision>,
    ) {
        self.status_tx.send_modify(|status| {
            let lifecycle_changed = status.state != state;
            let health = default_health_for_state(status.health.as_ref(), &state);
            status.state = state;
            status.health = health;
            status.spec = spec;
            status.controller = controller;
            status.revision = revision;
            if lifecycle_changed {
                status.changed_at = now_ms();
            }
        });
    }

    pub(super) fn publish_active(&self, active: &Active, state: CoreState) {
        self.publish_instance(
            active.instance.as_ref(),
            state,
            &active.source_spec,
            &active.revision,
            active.capabilities,
            active.runtime_features,
        );
    }

    pub(super) fn publish_instance(
        &self,
        instance: &dyn RuntimeInstance,
        state: CoreState,
        source_spec: &InstanceSpec,
        revision: &ConfigRevision,
        capabilities: enumset::EnumSet<Feature>,
        runtime_features: enumset::EnumSet<RuntimeFeature>,
    ) {
        let health = instance.state().borrow().health.clone();
        self.status_tx.send_modify(|status| {
            let lifecycle_changed = status.state != state;
            status.state = state;
            status.health = health;
            status.spec = Some(spec_summary(source_spec, capabilities, runtime_features));
            status.controller = Some(instance.controller().host.clone());
            status.revision = Some(revision.clone());
            if lifecycle_changed {
                status.changed_at = now_ms();
            }
        });
    }

    pub(super) fn publish_epoch_status(&self, epoch: u64, instance: InstanceStatus) {
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

fn apply_epoch_status(status: &mut CoreStatus, epoch: u64, instance: &InstanceStatus) -> bool {
    if status.revision.as_ref().map(|revision| revision.epoch) != Some(epoch) {
        return false;
    }
    let state = instance_core_state(epoch, &instance.state);
    let lifecycle_changed = status.state != state;
    let health_changed = status.health != instance.health;
    if !lifecycle_changed && !health_changed {
        return false;
    }
    status.state = state;
    status.health = instance.health.clone();
    if lifecycle_changed {
        status.changed_at = now_ms();
    }
    true
}

pub(super) fn spec_summary(
    spec: &InstanceSpec,
    capabilities: enumset::EnumSet<Feature>,
    runtime_features: enumset::EnumSet<RuntimeFeature>,
) -> SpecSummary {
    SpecSummary {
        kind: spec.core.kind,
        config_path: spec.config_path.clone(),
        capabilities: capabilities.iter().collect(),
        runtime_features: runtime_features.iter().collect(),
    }
}

impl CoreManager {
    pub(super) fn publish_terminal_error(&self, error: &Error) {
        self.inner.publish(
            CoreState::Stopped {
                reason: Some(StopReason::Error(error.to_string())),
            },
            None,
            None,
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

pub(super) fn instance_core_state(epoch: u64, state: &InstanceState) -> CoreState {
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
    use tokio::sync::watch;

    use super::*;
    use crate::state::{ConfigRevision, InstanceState, InstanceStatus, StopReason};

    #[test]
    fn old_epoch_events_cannot_overwrite_new_epoch_status() {
        let mut status = CoreStatus::initial();
        status.revision = Some(ConfigRevision {
            epoch: 9,
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-9.yaml".into(),
        });
        status.state = CoreState::Running { epoch: 9, pid: 90 };
        for stale in [
            CoreState::Running { epoch: 8, pid: 80 },
            CoreState::Restarting {
                epoch: 8,
                attempt: 2,
            },
            CoreState::Stopped {
                reason: Some(StopReason::Finished),
            },
        ] {
            let stale_status = InstanceStatus {
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
            assert!(!apply_epoch_status(&mut status, 8, &stale_status));
            assert!(matches!(
                status.state,
                CoreState::Running { epoch: 9, pid: 90 }
            ));
        }
    }

    #[test]
    fn stale_epoch_status_neither_mutates_nor_wakes_watchers() {
        let mut status = CoreStatus::initial();
        status.revision = Some(ConfigRevision {
            epoch: 9,
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-9.yaml".into(),
        });
        status.state = CoreState::Running { epoch: 9, pid: 90 };
        let (tx, rx) = watch::channel(status);
        let stale = InstanceStatus {
            state: InstanceState::Running { pid: 80 },
            health: Some(HealthStatus::starting()),
        };

        let sent = tx.send_if_modified(|status| apply_epoch_status(status, 8, &stale));

        assert!(!sent);
        assert!(!rx.has_changed().unwrap());
        assert!(matches!(
            rx.borrow().state,
            CoreState::Running { epoch: 9, pid: 90 }
        ));
    }

    #[test]
    fn pure_health_transition_preserves_lifecycle_changed_at() {
        let mut status = CoreStatus::initial();
        status.revision = Some(ConfigRevision {
            epoch: 3,
            generation: 1,
            source_hash: "source".into(),
            effective_hash: "effective".into(),
            runtime_path: "config-3.yaml".into(),
        });
        status.state = CoreState::Running { epoch: 3, pid: 30 };
        status.changed_at = 7;
        let mut health = HealthStatus::starting();
        health.state = crate::state::HealthState::Unhealthy;
        let instance = InstanceStatus {
            state: InstanceState::Running { pid: 30 },
            health: Some(health.clone()),
        };

        assert!(apply_epoch_status(&mut status, 3, &instance));
        assert_eq!(status.changed_at, 7);
        assert_eq!(status.health, Some(health));
    }
}
