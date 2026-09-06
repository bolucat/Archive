//! Cross-epoch orchestration: manager-owned artifacts, start/stop/switch, and
//! atomic status publication.

mod apply;
mod dns_sync;
mod publish;
mod quarantine;
mod reconcile;
mod switching;

use std::sync::Arc;

use enumset::EnumSet;
use serde_yaml_ng::Mapping;
use tokio::sync::{broadcast, watch};

use crate::{
    Epoch, Feature, RuntimeFeature,
    capability::{ResolvedFeatures, VersionCache},
    config::{self, ConfigSnapshot, mihomo},
    dns::{DnsController, DnsOverrideRecord},
    error::Error,
    log::{LOG_CHANNEL_CAPACITY, LogFrame},
    log_sink::{self, SinkOptions},
    probe::ProbeHandle,
    runtime::{
        RuntimeBackend, RuntimeInstance, RuntimeLaunchRequest,
        process::{ProbePlan, ProcessRuntimeBackend},
    },
    runtime_store::{RuntimeConfigStore, RuntimeDirectoryLock, StagedRuntimeConfig},
    spec::{CoreSpec, InstanceSpec, LocalIpcPolicy, ManagerOptions, ResolvedController},
    state::{ConfigRevision, CoreState, CoreStatus, InstanceStatus, StopReason},
};

use publish::instance_core_state;
use quarantine::{reject_quarantine, sweep_orphans};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradeReason {
    NotRunning,
    UnsupportedKind,
    DnsListen,
    InboundConflict,
    PatchFailed,
    HttpController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchOutcome {
    Graceful,
    Hard {
        reason: DegradeReason,
    },
    DurabilityUncertain {
        outcome: Box<SwitchOutcome>,
        warning: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    /// Nothing was running; the desired runtime was started cold. Only
    /// [`CoreManager::reconcile`] produces this — the legacy `apply_config`
    /// path requires a running core.
    Started {
        revision: ConfigRevision,
    },
    Noop {
        revision: ConfigRevision,
    },
    Patched {
        revision: ConfigRevision,
    },
    Reloaded {
        revision: ConfigRevision,
    },
    Restarted {
        revision: ConfigRevision,
    },
    /// The process spec itself changed (a different core, binary, or launch
    /// option), so the old epoch was stopped and a new one started. Distinct
    /// from [`Self::Restarted`], which replaces the process inside one epoch.
    Switched {
        revision: ConfigRevision,
    },
    RolledBack {
        revision: ConfigRevision,
        failed_apply: String,
    },
    DurabilityUncertain {
        outcome: Box<ApplyOutcome>,
        warning: String,
    },
}

/// A cheap-to-clone handle; every clone shares the same orchestrator state.
#[derive(Clone)]
pub struct CoreManager {
    inner: Arc<Inner>,
}

pub struct CoreManagerBuilder {
    options: ManagerOptions,
    probes: ProbePlan,
    backend: Option<Arc<dyn RuntimeBackend>>,
    dns: Option<Arc<dyn DnsController>>,
}

struct Inner {
    options: ManagerOptions,
    backend: Arc<dyn RuntimeBackend>,
    dns: Option<Arc<dyn DnsController>>,
    store: RuntimeConfigStore,
    ctrl: tokio::sync::Mutex<Ctrl>,
    status_tx: watch::Sender<CoreStatus>,
    /// Outlives every epoch, so callers can subscribe before the first start and
    /// keep receiving across restarts and core switches.
    log_tx: broadcast::Sender<Arc<LogFrame>>,
    version_cache: VersionCache,
    /// `Some` while the JSONL sink is running. `None` means the caller turned it
    /// off, and there is deliberately no path to report — nothing will ever
    /// appear there.
    log_dir: Option<camino::Utf8PathBuf>,
    /// Dropping the manager without calling `shutdown()` abandons at most the
    /// final batch. This is diagnostic data and best-effort by design;
    /// `shutdown()` is the graceful path.
    log_sink: tokio::sync::Mutex<Option<log_sink::SinkHandle>>,
    // Declared last so ordinary Inner destruction drops instances/tasks before
    // releasing directory ownership.
    _runtime_lock: RuntimeDirectoryLock,
}

struct Ctrl {
    epochs: EpochAllocator,
    current: Option<Active>,
    last_spec: Option<InstanceSpec>,
    quarantine: Vec<QuarantinedEpoch>,
    /// The persisted-and-live DNS override, when a controller is injected and
    /// an override is in place. Serialized by the control lock like the rest.
    dns_record: Option<DnsOverrideRecord>,
}

impl Ctrl {
    fn new(max_epoch: u64) -> Self {
        Self {
            epochs: EpochAllocator::seeded(max_epoch),
            current: None,
            last_spec: None,
            quarantine: Vec::new(),
            dns_record: None,
        }
    }
}

/// Hands out epoch numbers: monotone for the manager's lifetime and seeded
/// past every artifact found on disk, so a number is never reused within one
/// runtime directory. It lives under the control lock because every allocation
/// happens inside a control transaction, which is also why it is not atomic.
/// Never returns 0, which is reserved for "no epoch"; exhausting 2^64 epochs in
/// one directory is an invariant violation, not a wrap.
struct EpochAllocator {
    last: u64,
}

impl EpochAllocator {
    fn seeded(max_seen: u64) -> Self {
        Self { last: max_seen }
    }

    fn next(&mut self) -> Epoch {
        self.last = self.last.checked_add(1).expect("epoch space exhausted");
        Epoch::new(self.last).expect("a checked increment cannot produce zero")
    }
}

#[derive(Debug, Clone)]
struct QuarantinedEpoch {
    epoch: Epoch,
    reason: String,
    death_proven: bool,
}

/// Everything the manager knows about one epoch besides the live process.
/// Built once by a prepare step, carried whole into [`Active`], and reused
/// verbatim to relaunch the same epoch on rollback. Deliberately not `Clone`:
/// every consumer moves or borrows it; a clone would be two sources of truth
/// for one epoch.
struct EpochPlan {
    source_spec: InstanceSpec,
    effective_spec: InstanceSpec,
    controller: ResolvedController,
    revision: ConfigRevision,
    capabilities: EnumSet<Feature>,
    runtime_features: EnumSet<RuntimeFeature>,
    source_document: Mapping,
    effective_document: Mapping,
}

impl EpochPlan {
    fn classification_view(&self) -> mihomo::ConfigClassificationView<'_> {
        mihomo::ConfigClassificationView {
            source: &self.source_document,
            effective: &self.effective_document,
            spec: &self.source_spec,
        }
    }
}

struct Active {
    instance: Box<dyn RuntimeInstance>,
    forwarder: tokio::task::JoinHandle<()>,
    plan: EpochPlan,
}

/// A stop that could not be proven, carrying the epoch it belongs to. What to
/// do about it — discard a half-built candidate first, wrap the error, publish,
/// latch quarantine — differs per transaction and stays with the caller, in the
/// order that caller already has.
struct RetireFailure {
    epoch: Epoch,
    error: Error,
}

struct PreparedGraceful {
    plan: EpochPlan,
    full_staged: StagedRuntimeConfig,
    restoration: Option<(Box<clash_api::ConfigPatch>, mihomo::RuntimeProjection)>,
}

struct PreparedApply {
    plan: EpochPlan,
    staged: StagedRuntimeConfig,
}

impl CoreManagerBuilder {
    pub fn readiness_probe(mut self, probe: ProbeHandle) -> Self {
        self.probes.readiness = Some(probe);
        self
    }

    pub fn liveness_probe(mut self, probe: ProbeHandle) -> Self {
        self.probes.liveness = Some(probe);
        self.probes.liveness_with_readiness = false;
        self
    }

    pub fn liveness_with_readiness_probe(mut self) -> Self {
        self.probes.liveness = None;
        self.probes.liveness_with_readiness = true;
        self
    }

    /// Replaces the default process backend. The probe methods on this builder
    /// configure the default backend only; a custom backend owns its own
    /// probing.
    pub fn runtime_backend(mut self, backend: Arc<dyn RuntimeBackend>) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Injects the host DNS override component (amendment A5 ③). Without one
    /// the manager has zero DNS behavior.
    pub fn dns_controller(mut self, dns: Arc<dyn DnsController>) -> Self {
        self.dns = Some(dns);
        self
    }

    pub async fn build(self) -> Result<CoreManager, Error> {
        CoreManager::build_configured(self).await
    }
}

impl CoreManager {
    pub fn builder(options: ManagerOptions) -> CoreManagerBuilder {
        CoreManagerBuilder {
            options,
            probes: ProbePlan::default(),
            backend: None,
            dns: None,
        }
    }

    pub async fn new(options: ManagerOptions) -> Result<Self, Error> {
        Self::builder(options).build().await
    }

    async fn build_configured(builder: CoreManagerBuilder) -> Result<Self, Error> {
        let CoreManagerBuilder {
            options,
            probes,
            backend,
            dns,
        } = builder;
        let runtime_dir = options
            .runtime_dir
            .clone()
            .ok_or_else(|| Error::InvalidManagerOptions("runtime_dir is required".into()))?;
        let store = RuntimeConfigStore::new(runtime_dir).await?;
        let runtime_lock = store.acquire_ownership().await?;

        // Validated under every policy: a template that cannot produce an
        // endpoint is a configuration error whether or not this core ends up
        // selecting local IPC, and construction is the caller's last chance to
        // fix it.
        config::validate_managed_endpoint(store.dir(), options.controller_template.as_deref())?;
        for (name, timeout) in [
            ("control_timeout", options.control_timeout),
            ("reconcile_timeout", options.reconcile_timeout),
            ("stop_timeout", options.stop_timeout),
            ("dns_timeout", options.dns_timeout),
        ] {
            if timeout.is_zero() {
                return Err(Error::InvalidManagerOptions(format!(
                    "{name} must be greater than zero"
                )));
            }
        }
        // Validated whether or not the sink is enabled, for the same reason the
        // controller template above is validated under every policy:
        // construction is the caller's last chance to hear about it.
        if options.log_max_bytes == 0 {
            return Err(Error::InvalidManagerOptions(
                "log_max_bytes must be greater than zero".into(),
            ));
        }
        if options.log_max_files == 0 {
            return Err(Error::InvalidManagerOptions(
                "log_max_files must be greater than zero".into(),
            ));
        }
        let max_epoch = sweep_orphans(&store).await?;
        dns_sync::reconcile_orphan_record(&store, dns.as_deref(), options.dns_timeout).await;
        let (status_tx, _) = watch::channel(CoreStatus::initial());
        let (log_tx, _) = broadcast::channel(LOG_CHANNEL_CAPACITY);
        // Subscribed here rather than inside the task, and before any instance
        // can exist, so the sink cannot miss the start-up burst.
        let (log_dir, log_sink) = if options.log_sink_enabled {
            let dir = log_sink::prepare_dir(store.dir()).await?;
            let handle = log_sink::spawn(
                dir.clone(),
                SinkOptions {
                    max_bytes: options.log_max_bytes,
                    max_files: options.log_max_files,
                },
                log_tx.subscribe(),
                options.cancel_token.child_token(),
            )
            .await?;
            (Some(dir), Some(handle))
        } else {
            (None, None)
        };
        let backend = backend.unwrap_or_else(|| {
            Arc::new(ProcessRuntimeBackend::new(
                probes,
                options.cancel_token.clone(),
            ))
        });
        Ok(Self {
            inner: Arc::new(Inner {
                options,
                backend,
                dns,
                store,
                ctrl: tokio::sync::Mutex::new(Ctrl::new(max_epoch)),
                status_tx,
                log_tx,
                version_cache: VersionCache::default(),
                log_dir,
                log_sink: tokio::sync::Mutex::new(log_sink),
                _runtime_lock: runtime_lock,
            }),
        })
    }

    pub fn subscribe(&self) -> watch::Receiver<CoreStatus> {
        self.inner.status_tx.subscribe()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<Arc<LogFrame>> {
        self.inner.log_tx.subscribe()
    }

    /// Where the JSONL core-log archive is written, or `None` when the sink is
    /// disabled. Constant for the manager's lifetime, which is why it is an
    /// accessor and not a field on the status snapshot: putting it there would
    /// republish an unchanging string on every transition.
    pub fn log_dir(&self) -> Option<&camino::Utf8Path> {
        self.inner.log_dir.as_deref()
    }

    /// Read credentials from the applied plan, never from a desired config.
    pub async fn api_connection(&self) -> Option<crate::ApiConnection> {
        let ctrl = self.inner.ctrl.lock().await;
        let active = ctrl.current.as_ref()?;
        let snapshot = active.instance.state().borrow().clone();
        if !matches!(snapshot.state, crate::InstanceState::Running { .. }) {
            return None;
        }
        Some(crate::ApiConnection {
            instance_id: snapshot.instance_id?,
            controller: active.plan.controller.clone(),
        })
    }

    pub fn status(&self) -> CoreStatus {
        self.inner.status_tx.borrow().clone()
    }

    /// Test-only fault hook for the installed-but-parent-sync-failed branch.
    #[cfg(feature = "test-hooks")]
    #[doc(hidden)]
    pub fn inject_runtime_parent_sync_failure_once_for_test(&self) {
        self.inner.store.inject_replace_parent_sync_failure_once();
    }

    pub async fn start(&self, spec: InstanceSpec) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        reject_quarantine(&ctrl)?;
        let running = ctrl
            .current
            .as_ref()
            .is_some_and(|active| !active.instance.state().borrow().state.is_terminal());
        if running {
            return Err(Error::AlreadyRunning);
        }
        self.discard_stale(&mut ctrl).await?;
        self.start_locked(&mut ctrl, spec).await
    }

    async fn start_locked(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error> {
        let epoch = ctrl.epochs.next();
        let snapshot = match ConfigSnapshot::load(&spec.config_path).await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.publish_terminal_error(&error);
                return Err(error);
            }
        };
        let prepared = match self.prepare_launch(&spec, epoch, &snapshot).await {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = self.inner.store.cleanup_epoch(epoch).await;
                self.publish_terminal_error(&error);
                return Err(error);
            }
        };
        self.start_prepared(ctrl, prepared).await
    }

    async fn start_prepared(&self, ctrl: &mut Ctrl, plan: EpochPlan) -> Result<(), Error> {
        let epoch = plan.revision.epoch;
        self.inner
            .publish(CoreState::Starting { epoch }, Some(&plan));
        let instance = match self.spawn_instance(&plan).await {
            Ok(instance) => instance,
            Err(error) => {
                let _ = self.inner.store.cleanup_epoch(epoch).await;
                self.publish_terminal_error(&error);
                return Err(error);
            }
        };

        if let Err(readiness_error) = instance.wait_ready().await {
            match instance
                .stop_and_confirm_dead(self.inner.options.stop_timeout)
                .await
            {
                Ok(()) => {
                    let _ = self.inner.store.cleanup_epoch(epoch).await;
                    self.publish_terminal_error(&readiness_error);
                    return Err(readiness_error);
                }
                Err(stop_error) => {
                    let error = Error::StopUnconfirmed(format!(
                        "{readiness_error}; failed to stop rejected initial instance: {stop_error}"
                    ));
                    return Err(self.latch_quarantine(ctrl, epoch, error));
                }
            }
        }

        let pid = instance.pid().unwrap_or_default();
        self.inner
            .publish_instance(instance.as_ref(), CoreState::Running { epoch, pid }, &plan);
        self.install(ctrl, instance, plan);
        Ok(())
    }

    /// Stops the active instance even while another epoch is quarantined.
    ///
    /// This intentionally bypasses the quarantine gate so callers can reduce
    /// the number of possibly live processes; it does not clear quarantine.
    pub async fn stop(&self) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        // Restore at the head of the stop transaction: resolution must never
        // point at a core that is being torn down.
        self.dns_restore(&mut ctrl).await;
        let Some(active) = ctrl.current.take() else {
            return Err(Error::NotStarted);
        };
        let Active {
            instance,
            forwarder,
            plan,
        } = active;
        let captured_status = instance.state().borrow().clone();
        abort_and_await(forwarder).await;
        if captured_status.state.is_terminal() {
            let epoch = instance.epoch();
            self.inner.publish(
                instance_core_state(epoch, &captured_status.state),
                Some(&plan),
            );
            if let Err(error) = instance
                .stop_and_confirm_dead(self.inner.options.stop_timeout)
                .await
            {
                if matches!(error, Error::StopUnconfirmed(_)) {
                    return Err(self.latch_quarantine(&mut ctrl, epoch, error));
                }
                return Err(error);
            }
            if let Err(error) = self.inner.store.cleanup_epoch(epoch).await {
                self.publish_terminal_error(&error);
                return Err(error);
            }
            return Err(Error::NotStarted);
        }
        let epoch = instance.epoch();
        self.inner
            .publish(CoreState::Stopping { epoch }, Some(&plan));
        if let Err(error) = instance
            .stop_and_confirm_dead(self.inner.options.stop_timeout)
            .await
        {
            if matches!(error, Error::StopUnconfirmed(_)) {
                return Err(self.latch_quarantine(&mut ctrl, epoch, error));
            }
            self.publish_terminal_error(&error);
            return Err(error);
        }
        if let Err(error) = self.inner.store.cleanup_epoch(epoch).await {
            self.publish_terminal_error(&error);
            return Err(error);
        }
        self.inner.publish(
            CoreState::Stopped {
                reason: Some(StopReason::User),
            },
            None,
        );
        Ok(())
    }

    pub async fn check_config(&self, spec: &InstanceSpec) -> Result<(), Error> {
        self.inner.backend.check_config(spec).await
    }

    async fn resolve_features(&self, core: &CoreSpec) -> Result<ResolvedFeatures, Error> {
        crate::capability::resolve_features(
            &self.inner.version_cache,
            core,
            self.inner.options.local_ipc_policy,
        )
        .await
    }

    fn warn_http_fallback(
        &self,
        core: &CoreSpec,
        resolved_version: Option<&str>,
        rewrote_controller: bool,
    ) {
        if self.inner.options.local_ipc_policy == LocalIpcPolicy::Prefer && !rewrote_controller {
            tracing::warn!(
                kind = %core.kind,
                version = resolved_version.or(core.version.as_deref()).unwrap_or("unknown"),
                "local IPC is unsupported; falling back to the configured HTTP controller"
            );
        }
    }

    /// Stops the active instance even while another epoch is quarantined.
    ///
    /// Like [`Self::stop`], shutdown intentionally bypasses the quarantine
    /// gate and never treats an unrelated uncertain epoch as recovered.
    pub async fn shutdown(&self) -> Result<(), Error> {
        // Held through sink finalization so no control-plane operation can interleave
        // between core stop and archive teardown.
        let mut ctrl = self.inner.ctrl.lock().await;
        // Same head restore as `stop`.
        self.dns_restore(&mut ctrl).await;
        let result: Result<(), Error> = async {
            if let Some(active) = ctrl.current.take() {
                let Active {
                    instance,
                    forwarder,
                    plan,
                } = active;
                abort_and_await(forwarder).await;
                let epoch = instance.epoch();
                self.inner
                    .publish(CoreState::Stopping { epoch }, Some(&plan));
                if let Err(error) = instance
                    .stop_and_confirm_dead(self.inner.options.stop_timeout)
                    .await
                {
                    if matches!(error, Error::StopUnconfirmed(_)) {
                        return Err(self.latch_quarantine(&mut ctrl, epoch, error));
                    }
                    self.publish_terminal_error(&error);
                    return Err(error);
                }
                if let Err(error) = self.inner.store.cleanup_epoch(epoch).await {
                    self.publish_terminal_error(&error);
                    return Err(error);
                }
                self.inner.publish(
                    CoreState::Stopped {
                        reason: Some(StopReason::User),
                    },
                    None,
                );
            }
            Ok(())
        }
        .await;
        let log_sink = self.inner.log_sink.lock().await.take();
        if let Some(log_sink) = log_sink {
            log_sink.shutdown().await;
        }
        result
    }

    async fn spawn_instance(&self, plan: &EpochPlan) -> Result<Box<dyn RuntimeInstance>, Error> {
        self.inner
            .backend
            .launch(RuntimeLaunchRequest {
                effective_spec: plan.effective_spec.clone(),
                epoch: plan.revision.epoch,
                controller: plan.controller.clone(),
                log_tx: self.inner.log_tx.clone(),
            })
            .await
    }

    /// The one place an epoch enters the slot: forwarder, retained spec,
    /// current. Publishes nothing. Every caller keeps its own `Running`
    /// publication where it is today — before this call in `start` and the
    /// graceful switch, after it in the apply compensations. The forwarder
    /// publishes without the control lock, so which frame reaches the watch
    /// first is observable downstream and is not this function's to decide.
    fn install<'c>(
        &self,
        ctrl: &'c mut Ctrl,
        instance: Box<dyn RuntimeInstance>,
        plan: EpochPlan,
    ) -> &'c mut Active {
        let forwarder = spawn_forwarder(&self.inner, instance.state(), plan.revision.epoch);
        ctrl.last_spec = Some(plan.source_spec.clone());
        ctrl.current.insert(Active {
            instance,
            forwarder,
            plan,
        })
    }

    /// The mechanics every stop shares: take the epoch out of the slot, silence
    /// its forwarder, prove the process dead. Publishes nothing and latches no
    /// quarantine — an unproven stop comes back as [`RetireFailure`] for the
    /// caller to place in its own sequence.
    async fn retire_current(&self, ctrl: &mut Ctrl) -> Result<Option<EpochPlan>, RetireFailure> {
        let Some(Active {
            instance,
            forwarder,
            plan,
        }) = ctrl.current.take()
        else {
            return Ok(None);
        };
        abort_and_await(forwarder).await;
        let epoch = plan.revision.epoch;
        instance
            .stop_and_confirm_dead(self.inner.options.stop_timeout)
            .await
            .map_err(|error| RetireFailure { epoch, error })?;
        Ok(Some(plan))
    }

    /// Stale-epoch hygiene shared by start, switch and reconcile: a terminal
    /// epoch still sitting in the slot is retired and its artifacts removed
    /// before the slot is reused.
    async fn discard_stale(&self, ctrl: &mut Ctrl) -> Result<(), Error> {
        match self.retire_current(ctrl).await {
            Ok(None) => Ok(()),
            Ok(Some(plan)) => self.inner.store.cleanup_epoch(plan.revision.epoch).await,
            Err(RetireFailure { epoch, error }) => {
                Err(if matches!(error, Error::StopUnconfirmed(_)) {
                    self.latch_quarantine(ctrl, epoch, error)
                } else {
                    error
                })
            }
        }
    }
}

async fn abort_and_await(mut forwarder: tokio::task::JoinHandle<()>) {
    forwarder.abort();
    let _ = (&mut forwarder).await;
}

fn spawn_forwarder(
    inner: &Arc<Inner>,
    mut state_rx: watch::Receiver<InstanceStatus>,
    epoch: Epoch,
) -> tokio::task::JoinHandle<()> {
    let inner = Arc::downgrade(inner);
    tokio::spawn(async move {
        while state_rx.changed().await.is_ok() {
            let status = state_rx.borrow_and_update().clone();
            let terminal = status.state.is_terminal();
            let Some(inner) = inner.upgrade() else {
                break;
            };
            inner.publish_epoch_status(epoch, status);
            if terminal {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::EpochAllocator;
    use crate::epoch::epoch;

    #[test]
    fn epochs_are_monotone_past_the_seed_and_never_zero() {
        let mut fresh = EpochAllocator::seeded(0);
        assert_eq!(fresh.next(), epoch(1));
        let mut seeded = EpochAllocator::seeded(7);
        assert_eq!((seeded.next(), seeded.next()), (epoch(8), epoch(9)));
    }

    #[test]
    #[should_panic(expected = "epoch space exhausted")]
    fn epoch_exhaustion_is_an_invariant_violation_not_a_wrap() {
        EpochAllocator::seeded(u64::MAX).next();
    }
}
