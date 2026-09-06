use crate::{
    config::{
        ConfigSnapshot,
        mihomo::{self, ConfigChange},
    },
    error::Error,
    probe::ProbePhase,
    runtime::RuntimeInstance,
    spec::InstanceSpec,
    state::{ConfigRevision, CoreState, RevisionId},
};

use super::{
    Active, ApplyOutcome, CoreManager, Ctrl, EpochPlan, PreparedApply, RetireFailure,
    quarantine::reject_quarantine,
};

impl CoreManager {
    pub async fn apply_config(
        &self,
        input: InstanceSpec,
        expected_revision: Option<RevisionId>,
    ) -> Result<ApplyOutcome, Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        reject_quarantine(&ctrl)?;
        self.apply_locked(&mut ctrl, input, expected_revision).await
    }

    /// The running-core apply transaction, entered with the control lock held.
    /// Shared by [`Self::apply_config`] and [`CoreManager::reconcile`].
    pub(super) async fn apply_locked(
        &self,
        ctrl: &mut Ctrl,
        input: InstanceSpec,
        expected_revision: Option<RevisionId>,
    ) -> Result<ApplyOutcome, Error> {
        let current = ctrl.current.as_ref().ok_or(Error::NotStarted)?;
        if current.instance.state().borrow().state.is_terminal() {
            return Err(Error::NotStarted);
        }
        let actual_revision = current.plan.revision.id();
        if let Some(expected) = expected_revision
            && expected != actual_revision
        {
            return Err(Error::RevisionConflict {
                expected,
                actual: Some(actual_revision),
            });
        }

        let snapshot = ConfigSnapshot::load(&input.config_path).await?;
        let prepared = self
            .prepare_apply(current, input.clone(), &snapshot)
            .await?;
        let change = mihomo::classify(
            current.plan.classification_view(),
            prepared.plan.classification_view(),
        )?;
        if matches!(change, ConfigChange::Noop) {
            return Ok(ApplyOutcome::Noop {
                revision: current.plan.revision.clone(),
            });
        }
        if matches!(change, ConfigChange::Switch) {
            drop(prepared);
            return self.switch_with_compensation(ctrl, input, snapshot).await;
        }

        let backup = self
            .inner
            .store
            .backup(
                current.plan.revision.epoch,
                prepared.plan.revision.generation,
            )
            .await?;
        let PreparedApply {
            plan: desired,
            staged,
        } = prepared;
        let commit = match self
            .inner
            .store
            .commit_replace(staged, desired.revision.epoch)
            .await
        {
            Ok(commit) => commit,
            Err(error) => {
                let _ = self.inner.store.remove_backup(backup).await;
                return Err(error);
            }
        };
        let durability_warning = commit.durability_warning().map(str::to_owned);

        let reconciled = tokio::time::timeout(
            self.inner.options.reconcile_timeout,
            self.reconcile_in_place(current, &change, &desired),
        )
        .await
        .unwrap_or(false);
        if reconciled {
            let revision = desired.revision.clone();
            let outcome = match change {
                ConfigChange::Patch { .. } => ApplyOutcome::Patched {
                    revision: revision.clone(),
                },
                ConfigChange::Reload => ApplyOutcome::Reloaded {
                    revision: revision.clone(),
                },
                ConfigChange::Noop | ConfigChange::Switch => unreachable!(),
            };
            let active = ctrl.current.as_mut().expect("current held by control lock");
            active.plan = desired;
            self.inner.publish_active(
                active,
                CoreState::Running {
                    epoch: revision.epoch,
                    pid: active.instance.pid().unwrap_or_default(),
                },
            );
            ctrl.last_spec = Some(active.plan.source_spec.clone());
            if let Err(error) = self.inner.store.remove_backup(backup).await {
                tracing::warn!("failed to remove successful apply backup: {error}");
            }
            return Ok(with_durability_warning(outcome, durability_warning));
        }

        let result = self.restart_with_compensation(ctrl, desired, backup).await;
        with_durability_result(result, durability_warning)
    }

    async fn prepare_apply(
        &self,
        current: &Active,
        input: InstanceSpec,
        snapshot: &ConfigSnapshot,
    ) -> Result<PreparedApply, Error> {
        self.validate_launchable(&input).await?;
        let resolved = self.resolve_features(&input.core).await?;
        let epoch = current.plan.revision.epoch;
        let prepared = snapshot.prepare_full(
            self.inner.options.controller_template.as_deref(),
            self.inner.store.dir(),
            epoch,
            resolved.runtime,
        )?;
        self.warn_http_fallback(
            &input.core,
            resolved.version.as_deref(),
            prepared.rewrote_controller,
        );
        let staged = self.inner.store.stage(epoch, &prepared.bytes).await?;
        let mut check_spec = input.clone();
        check_spec.config_path = staged.path().to_owned();
        self.inner.backend.check_config(&check_spec).await?;

        let runtime_path = current.plan.revision.runtime_path.clone();
        let mut effective_spec = input.clone();
        effective_spec.config_path = runtime_path.clone();
        effective_spec.pid_file = Some(self.inner.store.pid_path(epoch));
        Ok(PreparedApply {
            plan: EpochPlan {
                source_spec: input,
                effective_spec,
                controller: prepared.controller,
                revision: ConfigRevision {
                    epoch,
                    generation: current.plan.revision.generation + 1,
                    source_hash: prepared.source_hash,
                    effective_hash: prepared.effective_hash,
                    runtime_path,
                },
                capabilities: resolved.capabilities,
                runtime_features: resolved.runtime,
                source_document: snapshot.document().clone(),
                effective_document: prepared.document,
            },
            staged,
        })
    }

    async fn reconcile_in_place(
        &self,
        current: &Active,
        change: &ConfigChange,
        desired: &EpochPlan,
    ) -> bool {
        if let ConfigChange::Patch { patch, projection } = change {
            return self
                .patch_and_verify(current.instance.as_ref(), patch, projection)
                .await;
        }
        if matches!(change, ConfigChange::Switch) {
            return false;
        }
        if matches!(change, ConfigChange::Noop) {
            return true;
        }
        let client = match crate::health::build_control_client(
            current.instance.controller(),
            self.inner.options.control_timeout,
        ) {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!("failed to build config control client: {error}");
                return false;
            }
        };
        match change {
            ConfigChange::Reload => {
                let request = clash_api::UpdateConfigRequest::from_path(
                    desired.revision.runtime_path.to_string(),
                );
                if let Err(error) = client
                    .update_config(&request, clash_api::UpdateConfigOptions { force: true })
                    .await
                {
                    tracing::warn!("config PUT failed: {error}");
                    return false;
                }
            }
            ConfigChange::Patch { .. } | ConfigChange::Switch | ConfigChange::Noop => {
                unreachable!()
            }
        }
        current
            .instance
            .probe_now(ProbePhase::Reconcile)
            .await
            .is_healthy()
    }

    pub(super) async fn patch_and_verify(
        &self,
        instance: &dyn RuntimeInstance,
        patch: &clash_api::ConfigPatch,
        projection: &mihomo::RuntimeProjection,
    ) -> bool {
        let client = match crate::health::build_control_client(
            instance.controller(),
            self.inner.options.control_timeout,
        ) {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!("failed to build config control client: {error}");
                return false;
            }
        };
        if let Err(error) = client.patch_config(patch).await {
            tracing::warn!("config PATCH returned an uncertain result: {error}");
        }
        match client.configs().await {
            Ok(runtime) => match projection.verify(&runtime) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(error) => {
                    tracing::warn!("failed to verify config projection: {error}");
                    return false;
                }
            },
            Err(error) => {
                tracing::warn!("GET /configs verification failed: {error}");
                return false;
            }
        }
        instance.probe_now(ProbePhase::Reconcile).await.is_healthy()
    }

    async fn restart_with_compensation(
        &self,
        ctrl: &mut Ctrl,
        desired: EpochPlan,
        backup: crate::RuntimeConfigBackup,
    ) -> Result<ApplyOutcome, Error> {
        let old_plan = match self.retire_current(ctrl).await {
            Ok(retired) => retired.expect("current held by control lock"),
            Err(RetireFailure {
                epoch: retired_epoch,
                error,
            }) => {
                if matches!(error, Error::StopUnconfirmed(_)) {
                    return Err(self.latch_quarantine(ctrl, retired_epoch, error));
                }
                let message = format!("failed to stop current epoch for reconcile: {error}");
                self.publish_terminal_error(&Error::ApplyFailed(message.clone()));
                return Err(Error::ApplyFailed(message));
            }
        };

        self.inner.publish(
            CoreState::Restarting {
                epoch: desired.revision.epoch,
                attempt: 0,
            },
            Some(&desired),
        );

        match self.spawn_replacement(&desired).await {
            Ok(instance) => {
                let revision = desired.revision.clone();
                let pid = instance.pid().unwrap_or_default();
                let active = self.install(ctrl, instance, desired);
                self.inner.publish_active(
                    active,
                    CoreState::Running {
                        epoch: revision.epoch,
                        pid,
                    },
                );
                if let Err(error) = self.inner.store.remove_backup(backup).await {
                    tracing::warn!("failed to remove successful restart backup: {error}");
                }
                Ok(ApplyOutcome::Restarted { revision })
            }
            Err(error @ Error::StopUnconfirmed(_)) => {
                Err(self.latch_quarantine(ctrl, desired.revision.epoch, error))
            }
            Err(apply_error) => {
                let apply_text = apply_error.to_string();
                let restore = match self.inner.store.restore(&backup).await {
                    Ok(restore) => restore,
                    Err(restore_error) => {
                        let error = Error::ApplyRollbackFailed {
                            apply: apply_text,
                            rollback: format!("runtime restore failed: {restore_error}"),
                        };
                        self.publish_terminal_error(&error);
                        return Err(error);
                    }
                };
                let restore_warning = restore.durability_warning().map(str::to_owned);
                self.inner.publish(
                    CoreState::Restarting {
                        epoch: old_plan.revision.epoch,
                        attempt: 0,
                    },
                    Some(&old_plan),
                );
                let rollback = match self.spawn_replacement(&old_plan).await {
                    Ok(instance) => {
                        let revision = old_plan.revision.clone();
                        let pid = instance.pid().unwrap_or_default();
                        let active = self.install(ctrl, instance, old_plan);
                        self.inner.publish_active(
                            active,
                            CoreState::Running {
                                epoch: revision.epoch,
                                pid,
                            },
                        );
                        if let Err(error) = self.inner.store.remove_backup(backup).await {
                            tracing::warn!("failed to remove rollback backup: {error}");
                        }
                        Ok(ApplyOutcome::RolledBack {
                            revision,
                            failed_apply: apply_text,
                        })
                    }
                    Err(rollback_error @ Error::StopUnconfirmed(_)) => {
                        let error = Error::StopUnconfirmed(format!(
                            "desired apply failed ({apply_text}); rollback replacement {rollback_error}"
                        ));
                        Err(self.latch_quarantine(ctrl, old_plan.revision.epoch, error))
                    }
                    Err(rollback_error) => {
                        let error = Error::ApplyRollbackFailed {
                            apply: apply_text,
                            rollback: rollback_error.to_string(),
                        };
                        self.publish_terminal_error(&error);
                        Err(error)
                    }
                };
                with_durability_result(rollback, restore_warning)
            }
        }
    }

    async fn switch_with_compensation(
        &self,
        ctrl: &mut Ctrl,
        input: InstanceSpec,
        snapshot: ConfigSnapshot,
    ) -> Result<ApplyOutcome, Error> {
        let epoch = ctrl.epochs.next();
        let desired = self.prepare_launch(&input, epoch, &snapshot).await?;
        let old_plan = match self.retire_current(ctrl).await {
            Ok(retired) => retired.expect("current held by control lock"),
            Err(RetireFailure {
                epoch: retired_epoch,
                error,
            }) => {
                let _ = self.inner.store.cleanup_epoch(epoch).await;
                if matches!(error, Error::StopUnconfirmed(_)) {
                    return Err(self.latch_quarantine(ctrl, retired_epoch, error));
                }
                let message = format!("failed to stop current epoch for switch: {error}");
                self.publish_terminal_error(&Error::ApplyFailed(message.clone()));
                return Err(Error::ApplyFailed(message));
            }
        };

        self.inner.publish(
            CoreState::Switching {
                from: Some(old_plan.revision.epoch),
                to: desired.revision.epoch,
            },
            Some(&desired),
        );

        match self.spawn_replacement(&desired).await {
            Ok(instance) => {
                let revision = desired.revision.clone();
                let pid = instance.pid().unwrap_or_default();
                let active = self.install(ctrl, instance, desired);
                self.inner.publish_active(
                    active,
                    CoreState::Running {
                        epoch: revision.epoch,
                        pid,
                    },
                );
                if let Err(error) = self
                    .inner
                    .store
                    .cleanup_epoch(old_plan.revision.epoch)
                    .await
                {
                    tracing::warn!("failed to clean switched-out epoch: {error}");
                }
                Ok(ApplyOutcome::Switched { revision })
            }
            Err(error @ Error::StopUnconfirmed(_)) => {
                Err(self.latch_quarantine(ctrl, desired.revision.epoch, error))
            }
            Err(apply_error) => {
                let apply_text = apply_error.to_string();
                if let Err(error) = self.inner.store.cleanup_epoch(epoch).await {
                    tracing::warn!("failed to clean rejected desired epoch: {error}");
                }
                self.inner.publish(
                    CoreState::Restarting {
                        epoch: old_plan.revision.epoch,
                        attempt: 0,
                    },
                    Some(&old_plan),
                );
                match self.spawn_replacement(&old_plan).await {
                    Ok(instance) => {
                        let revision = old_plan.revision.clone();
                        let pid = instance.pid().unwrap_or_default();
                        let active = self.install(ctrl, instance, old_plan);
                        self.inner.publish_active(
                            active,
                            CoreState::Running {
                                epoch: revision.epoch,
                                pid,
                            },
                        );
                        Ok(ApplyOutcome::RolledBack {
                            revision,
                            failed_apply: apply_text,
                        })
                    }
                    Err(rollback_error @ Error::StopUnconfirmed(_)) => {
                        let error = Error::StopUnconfirmed(format!(
                            "desired switch failed ({apply_text}); rollback replacement {rollback_error}"
                        ));
                        Err(self.latch_quarantine(ctrl, old_plan.revision.epoch, error))
                    }
                    Err(rollback_error) => {
                        let error = Error::ApplyRollbackFailed {
                            apply: apply_text,
                            rollback: rollback_error.to_string(),
                        };
                        self.publish_terminal_error(&error);
                        Err(error)
                    }
                }
            }
        }
    }
}

fn with_durability_warning(outcome: ApplyOutcome, warning: Option<String>) -> ApplyOutcome {
    match warning {
        Some(warning) => ApplyOutcome::DurabilityUncertain {
            outcome: Box::new(outcome),
            warning,
        },
        None => outcome,
    }
}

fn with_durability_result(
    result: Result<ApplyOutcome, Error>,
    warning: Option<String>,
) -> Result<ApplyOutcome, Error> {
    match (result, warning) {
        (Ok(outcome), warning) => Ok(with_durability_warning(outcome, warning)),
        (Err(error), Some(warning)) => Err(Error::DurabilityUncertain {
            source: Box::new(error),
            warning,
        }),
        (Err(error), None) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{super::switching::with_switch_durability_result, *};

    #[test]
    fn durability_warning_preserves_structured_apply_error() {
        let result = with_durability_result(
            Err(Error::ApplyRollbackFailed {
                apply: "desired failed".into(),
                rollback: "rollback failed".into(),
            }),
            Some("directory sync failed".into()),
        );
        let Err(Error::DurabilityUncertain { source, warning }) = result else {
            panic!("structured error was flattened")
        };
        assert!(matches!(*source, Error::ApplyRollbackFailed { .. }));
        assert_eq!(warning, "directory sync failed");
    }

    #[test]
    fn durability_warning_wraps_stop_unconfirmed_without_flattening() {
        let apply = with_durability_result(
            Err(Error::StopUnconfirmed("apply stop uncertain".into())),
            Some("apply sync warning".into()),
        );
        let Err(Error::DurabilityUncertain { source, warning }) = apply else {
            panic!("apply stop uncertainty was not structurally wrapped")
        };
        assert!(matches!(*source, Error::StopUnconfirmed(_)));
        assert_eq!(warning, "apply sync warning");

        let switch = with_switch_durability_result(
            Err(Error::StopUnconfirmed("switch stop uncertain".into())),
            Some("switch sync warning".into()),
        );
        let Err(Error::DurabilityUncertain { source, warning }) = switch else {
            panic!("switch stop uncertainty was not structurally wrapped")
        };
        assert!(matches!(*source, Error::StopUnconfirmed(_)));
        assert_eq!(warning, "switch sync warning");
    }
}
