use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::Arc,
};

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    ApplyOutcome, ConfigInput, ConfigRevision, ControlOptions, CoreCommand, CoreCommandEnvelope,
    CoreControl, CoreError as ControlError, CoreErrorKind, CoreKind, CoreManager as Manager,
    CoreSpec, CoreState as ManagerCoreState, CoreStatus, Error as ManagerError, ExecutorExit,
    HealthState, HealthStatus, Host, InstanceOptions, InstanceSpec, LocalIpcPolicy, LogFrame,
    LogLevel, ManagerOptions, OperationHandle, OperationId, OperationOutput, OperationState,
    ReconcileRequest, RevisionId,
};
use nyanpasu_ipc::api::{
    R, RBuilder,
    core::{
        apply::{ApplyOutcomeKind, CoreApplyData},
        v2::{
            CoreCommandInfo, CoreOperationReq, CoreSubmitReq, OperationErrorInfo, OperationInfo,
            OperationOutputInfo, OperationPhase, ReconcileOutcomeInfo, ReconcileOutcomeKind,
        },
    },
    status::{
        ConfigRevisionInfo, CoreControllerInfo, CoreHealthInfo, CoreHealthState, CoreInfos,
        CoreState, CoreStateDetail, RevisionIdInfo,
    },
    ws::events::Event as WsEvent,
};
use nyanpasu_utils::core::{ClashCoreType, CoreType};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{Semaphore, broadcast::error::RecvError, watch};
use tracing::instrument;

use super::{consts::RuntimeInfos, events::EventHub};

const CORE_LOG_TARGET: &str = "nyanpasu_service::core";

/// Concurrent `/core/check` operations a service will run. Each one spawns a
/// core binary, so this is a resource bound, not a fairness knob: two lets an
/// honest client fire a second check while the first is running, and refuses
/// the flood a third would begin. Per-service rather than a `static` so the
/// bound is testable without the runtime crate's tests interfering with each
/// other.
const MAX_CONCURRENT_CHECKS: usize = 2;

/// Long-poll clamp for `/v2/core/operation`, kept well under the route
/// middleware's 120s liveness bound.
const MAX_OPERATION_WAIT_MS: u64 = 90_000;

/// Upper bound on the executor-first shutdown: the transaction already in
/// flight has to finish before the queued `Shutdown` can run. Exceeding it is
/// reported, never waited out — a leaked core is collected by the next
/// construction's orphan sweep, a hung daemon exit is not collected by anything.
const CORE_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(90);

/// Legacy wire strings the GUI branches on. These are protocol, not
/// diagnostics: changing any of them is a breaking change to clash-nyanpasu.
pub(crate) const MSG_CORE_ALREADY_RUNNING: &str = "core is already running";
pub(crate) const MSG_CORE_ALREADY_STOPPED: &str = "core is already stopped";
pub(crate) const MSG_CORE_NOT_STARTED: &str = "core have not been started yet";

/// A failed operation, carrying its wire `error_kind` next to its message.
///
/// `start`/`stop`/`restart` predate `error_kind` and keep returning
/// `anyhow::Error`; the S8 operations need the classification, and the only
/// place it can be derived without downcasting is where the `ManagerError` is
/// still typed.
pub(crate) struct OpError {
    kind: Option<CoreErrorKind>,
    message: String,
    /// The producer's answer, when it has one. Left `None` everywhere the
    /// service is only classifying a failure it observed: the kind's default
    /// is the client's fallback, and repeating it here would turn a guess into
    /// an assertion on the wire.
    retryable: Option<bool>,
}

impl OpError {
    /// A failure the service cannot classify. Omitting the kind is correct
    /// here: a guessed one is worse than none.
    fn plain(message: impl Into<String>) -> Self {
        Self {
            kind: None,
            message: message.into(),
            retryable: None,
        }
    }

    /// A failure the service *can* classify, where the classification is a fact
    /// about the failure and not a guess about its cause.
    fn with_kind(kind: CoreErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind: Some(kind),
            message: message.into(),
            retryable: None,
        }
    }

    /// Pins retryability the control plane already decided, rather than leaving
    /// the client to infer it from the kind.
    fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = Some(retryable);
        self
    }

    /// The error envelope for this failure, `error_kind` included.
    pub(crate) fn into_envelope<T>(self) -> R<'static, T>
    where
        T: Serialize + DeserializeOwned + std::fmt::Debug,
    {
        RBuilder::other_error_with_kind(Cow::Owned(self.message), self.kind, self.retryable)
    }
}

impl From<ManagerError> for OpError {
    fn from(error: ManagerError) -> Self {
        Self {
            kind: error.kind(),
            retryable: None,
            message: match &error {
                // The legacy wire string the GUI already branches on, so
                // `apply` on a stopped core reads exactly like `restart` on
                // one instead of inventing a second phrasing.
                ManagerError::NotStarted => MSG_CORE_NOT_STARTED.to_owned(),
                _ => error.to_string(),
            },
        }
    }
}

impl From<anyhow::Error> for OpError {
    fn from(error: anyhow::Error) -> Self {
        Self::plain(error.to_string())
    }
}

struct Inner {
    manager: Manager,
    /// The v2 control plane over the same manager: bounded queue, idempotency
    /// registry, cancellation isolation. The v1 methods below keep calling the
    /// manager directly until the bridge stage deletes them.
    control: CoreControl,
    /// Wire-type echo: the manager knows nothing about the alpha variants.
    ///
    /// A watch channel rather than a lock, because committing the echo has to be
    /// *observable*: it is the only signal a connected client has that the
    /// type it was last told about changed, and the manager will not publish a
    /// transition on its behalf. The bridge task holds only a receiver — an
    /// `Arc<Inner>` inside that task would keep the manager alive, so its watch
    /// channel would never close and the task would never exit.
    requested_core: watch::Sender<Option<CoreType>>,
    /// The executor sequence of the newest reconcile whose echo has been
    /// applied. Reconciles run serially, but the tasks watching them do not:
    /// two completions can be observed in either order, and applying the older
    /// one second would leave the wire claiming a type the runtime no longer
    /// has. Guarding the read-compare-write together is what makes the echo
    /// last-writer-by-execution-order rather than last-observer-wins.
    echo_applied: parking_lot::Mutex<Option<u64>>,
    /// Serializes adapter-level control ops and carries the closing latch.
    control_state: tokio::sync::Mutex<ControlState>,
    /// F2 lands here too; see §2.2.
    check_slots: Semaphore,
}

struct ControlState {
    closing: bool,
}

#[derive(Clone)]
pub struct CoreManagerService {
    inner: Arc<Inner>,
}

impl CoreManagerService {
    pub async fn new(
        runtime_dir: Utf8PathBuf,
        local_ipc_policy: LocalIpcPolicy,
        data_dir: Utf8PathBuf,
    ) -> Result<Self, anyhow::Error> {
        let source_dir = runtime_dir.join("v2-sources");
        let manager = Manager::new(ManagerOptions {
            runtime_dir: Some(runtime_dir),
            local_ipc_policy,
            ..ManagerOptions::default()
        })
        .await?;
        let core_control =
            CoreControl::spawn(manager.clone(), ControlOptions::new(source_dir, data_dir));
        Ok(Self {
            inner: Arc::new(Inner {
                manager,
                control: core_control,
                requested_core: watch::Sender::new(None),
                echo_applied: parking_lot::Mutex::new(None),
                control_state: tokio::sync::Mutex::new(ControlState { closing: false }),
                check_slots: Semaphore::new(MAX_CONCURRENT_CHECKS),
            }),
        })
    }

    /// State → ws events, and core logs → both the ws log ring and tracing.
    pub fn spawn_bridges(&self, hub: EventHub) {
        // Only the two receivers are moved in, never the adapter: see `Inner`.
        tokio::spawn(status_bridge(
            self.inner.manager.subscribe(),
            self.inner.requested_core.subscribe(),
            hub.clone(),
        ));

        let mut logs = self.inner.manager.subscribe_logs();
        tokio::spawn(async move {
            loop {
                match logs.recv().await {
                    Ok(frame) => {
                        // Nothing is mapped, clamped or copied here: the frame
                        // the parser bounded is the frame the ws ring gets.
                        // Tracing borrows it first so the subscription's own
                        // reference can then be moved on rather than cloned.
                        // A subscriber connecting mid-frame misses that frame,
                        // consistent with the stream's no-replay semantics.
                        forward_log(&frame);
                        if hub.has_log_subscribers() {
                            hub.send_log(frame);
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        tracing::warn!("core log bridge dropped {skipped} frames")
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        });
    }

    /// Stop the core and clean runtime artifacts. Idempotent; errors are logged, not returned.
    ///
    /// The stop goes through the v2 executor, which owns the closing latch:
    /// calling the manager directly would leave the executor accepting work,
    /// and a reconcile queued a moment earlier would put a core back up behind
    /// the shutdown that was supposed to end it. The v1 latch below is kept
    /// only as the v1 routing mirror it always was.
    pub async fn shutdown(&self) {
        let mut control = self.inner.control_state.lock().await;
        if control.closing {
            return;
        }
        control.closing = true;
        drop(control);

        match tokio::time::timeout(CORE_SHUTDOWN_TIMEOUT, self.inner.control.shutdown()).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) if self.inner.control.executor_is_closed() => {
                // Not a bypass: with the executor gone there is no transaction
                // owner left, and the manager is the only remaining way to stop
                // a core this process still owns.
                tracing::error!(
                    "the control executor is gone ({error}); stopping the core directly"
                );
                if let Err(error) = self.inner.manager.shutdown().await {
                    tracing::error!("failed to stop the core on shutdown: {error}");
                }
            }
            Ok(Err(error)) => tracing::error!("failed to stop the core on shutdown: {error}"),
            Err(_) => tracing::error!(
                "the core did not shut down within {CORE_SHUTDOWN_TIMEOUT:?}; \
                 leaving it to the next orphan sweep"
            ),
        }
    }

    /// Resolves when the v2 control executor exits, so the daemon can refuse to
    /// keep serving a control plane that is no longer there.
    pub async fn until_control_closed(&self) -> ExecutorExit {
        self.inner.control.until_closed().await
    }

    #[instrument(skip(self, infos))]
    pub async fn start(
        &self,
        infos: &RuntimeInfos,
        core_type: &CoreType,
        config_path: &Utf8Path,
    ) -> Result<(), anyhow::Error> {
        let control = self.inner.control_state.lock().await;
        if control.closing {
            anyhow::bail!("service is shutting down");
        }
        let config_path = config_path.canonicalize_utf8()?;
        let config_path =
            Utf8PathBuf::from_path_buf(dunce::simplified(config_path.as_std_path()).to_path_buf())
                .expect("a canonical UTF-8 path stays UTF-8 after simplification");
        tokio::fs::metadata(&config_path).await?; // check if the file exists
        if !matches!(
            self.inner.manager.status().state,
            ManagerCoreState::Stopped { .. }
        ) {
            anyhow::bail!(MSG_CORE_ALREADY_RUNNING);
        }
        let spec = self
            .instance_spec(infos, core_type, config_path)
            .map_err(|error| anyhow::anyhow!(error.message))?;
        tracing::info!(
            core_type = %core_type,
            kind = %spec.core.kind,
            working_dir = %spec.working_dir,
            binary_path = %spec.core.binary_path,
            config_path = %spec.config_path,
            "Starting Core"
        );
        self.inner.manager.start(spec).await?;
        self.publish_requested_core(Some(core_type));
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), anyhow::Error> {
        let control = self.inner.control_state.lock().await;
        if control.closing {
            anyhow::bail!("service is shutting down");
        }
        match self.inner.manager.stop().await {
            Ok(()) => Ok(()),
            Err(ManagerError::NotStarted) => anyhow::bail!(MSG_CORE_ALREADY_STOPPED),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn restart(&self) -> Result<(), anyhow::Error> {
        let control = self.inner.control_state.lock().await;
        if control.closing {
            anyhow::bail!("service is shutting down");
        }
        match self.inner.manager.restart().await {
            Ok(_outcome) => Ok(()),
            Err(ManagerError::NotStarted) => anyhow::bail!(MSG_CORE_NOT_STARTED),
            Err(error) => Err(error.into()),
        }
    }

    /// Apply `config_file` to the running core.
    ///
    /// The manager classifies the change and routes it: in-place patch, reload,
    /// same-epoch restart with rollback, or a full core switch when the process
    /// spec changed (which is what a different `core_type` produces). A stopped
    /// core is an error, never an implicit start (report §7 R2).
    #[instrument(skip(self, infos))]
    pub async fn apply(
        &self,
        infos: &RuntimeInfos,
        core_type: &CoreType,
        config_file: &Path,
        expected_revision: Option<&RevisionIdInfo>,
    ) -> Result<CoreApplyData, OpError> {
        let control = self.inner.control_state.lock().await;
        if control.closing {
            return Err(OpError::plain("service is shutting down"));
        }
        let config_path = canonical_config_path(config_file).await?;
        let spec = self.instance_spec(infos, core_type, config_path)?;
        let outcome = self
            .inner
            .manager
            .apply_config(spec, expected_revision.map(map_revision_id))
            .await?;
        let data = map_apply_outcome(&outcome);
        tracing::info!(
            outcome = ?data.outcome,
            epoch = data.revision.epoch,
            generation = data.revision.generation,
            "Applied config"
        );
        // A rollback put the *old* spec back, so the wire echo must not claim
        // the core type that was asked for — but the snapshot still moved, so
        // publish either way.
        self.publish_requested_core(
            (data.outcome != ApplyOutcomeKind::RolledBack).then_some(core_type),
        );
        Ok(data)
    }

    /// Dry-run a config against a core binary. Never touches the running core.
    #[instrument(skip(self, infos))]
    pub async fn check(
        &self,
        infos: &RuntimeInfos,
        core_type: &CoreType,
        config_file: &Path,
    ) -> Result<(), OpError> {
        {
            // Released before the check runs: it spawns the core binary, and
            // holding the adapter latch across an external process would block
            // start/stop/restart for as long as that takes.
            let control = self.inner.control_state.lock().await;
            if control.closing {
                return Err(OpError::plain("service is shutting down"));
            }
        }
        // Refused, not queued: waiting would convert a flood into a backlog of
        // futures that each still intend to spawn a core, and the caller would
        // learn nothing until its request timed out.
        let _slot = self.inner.check_slots.try_acquire().map_err(|_| {
            OpError::plain(format!(
                "at most {MAX_CONCURRENT_CHECKS} config checks may run at once; retry"
            ))
        })?;
        let config_path = canonical_config_path(config_file).await?;
        let spec = self.instance_spec(infos, core_type, config_path)?;
        self.inner.manager.check_config(&spec).await?;
        Ok(())
    }

    /// Clear the quarantine latch left by an epoch whose death could not be
    /// confirmed. Idempotent: succeeds when nothing was quarantined.
    #[instrument(skip(self))]
    pub async fn recover(&self) -> Result<(), OpError> {
        let control = self.inner.control_state.lock().await;
        if control.closing {
            return Err(OpError::plain("service is shutting down"));
        }
        self.inner.manager.recover_quarantine().await?;
        Ok(())
    }

    pub async fn status(&self) -> CoreInfos {
        project_core_infos(
            &self.inner.manager.status(),
            self.inner.requested_core.borrow().clone(),
        )
    }

    /// Where the manager archives core logs, or `None` when its sink is off.
    /// Constant for the manager's lifetime, so it is read on demand rather than
    /// carried in the status snapshot.
    pub fn core_log_dir(&self) -> Option<PathBuf> {
        self.inner
            .manager
            .log_dir()
            .map(|dir| dir.as_std_path().to_path_buf())
    }

    /// `POST /v2/core/submit`: admission into the control plane. Synchronous —
    /// the executor owns the transaction from here on, and a dropped
    /// connection cancels nothing.
    pub fn submit_v2(
        &self,
        infos: &RuntimeInfos,
        request: &CoreSubmitReq<'_>,
    ) -> Result<OperationInfo, OpError> {
        let id: OperationId = request.operation_id.parse().map_err(|_| {
            OpError::plain(format!("malformed operation id: {}", request.operation_id))
        })?;
        let mut echoed_core = None;
        let command = match &request.command {
            CoreCommandInfo::Reconcile {
                core_type,
                config,
                expected_digest,
                expected_applied,
            } => {
                // Binary resolution and kind mapping are host policy; reuse
                // the v1 spec builder for both. The placeholder config path is
                // never read — the config ships as bytes and the control plane
                // materializes it itself.
                let spec = self.instance_spec(infos, core_type, Utf8PathBuf::new())?;
                echoed_core = Some(core_type.clone().into_owned());
                CoreCommand::Reconcile(Box::new(ReconcileRequest {
                    core: spec.core,
                    config: ConfigInput::Inline {
                        bytes: config.as_bytes().to_vec(),
                        expected_digest: expected_digest.as_ref().map(|digest| digest.to_string()),
                    },
                    options: spec.options,
                    expected_applied: expected_applied.as_ref().map(map_revision_id),
                }))
            }
            CoreCommandInfo::Stop => CoreCommand::Stop,
            CoreCommandInfo::Recover => CoreCommand::Recover,
        };
        let handle = self
            .inner
            .control
            .submit(CoreCommandEnvelope {
                operation_id: id,
                command,
            })
            .map_err(op_error_from_control)?;
        let admitted = map_operation(handle.id(), handle.state());
        // After admission, and only for the submit that registered it: a
        // rejected envelope must leave no watcher behind, and an idempotent
        // re-submit must not add a second one to the same operation.
        if let Some(core_type) = echoed_core
            && handle.newly_admitted()
        {
            self.watch_reconcile_echo(handle, core_type);
        }
        Ok(admitted)
    }

    /// `POST /v2/core/operation`: registry query, optionally long-polling.
    pub async fn operation_v2(
        &self,
        request: &CoreOperationReq<'_>,
    ) -> Result<OperationInfo, OpError> {
        let id: OperationId = request.operation_id.parse().map_err(|_| {
            OpError::plain(format!("malformed operation id: {}", request.operation_id))
        })?;
        let state = match request.wait_ms {
            Some(wait_ms) => {
                let bound = std::time::Duration::from_millis(wait_ms.min(MAX_OPERATION_WAIT_MS));
                self.inner.control.wait_operation(id, bound).await
            }
            None => self.inner.control.operation(id),
        };
        state.map(|state| map_operation(id, state)).ok_or_else(|| {
            OpError::plain(format!(
                "unknown operation {id}: the registry entry was evicted or never existed; \
                 re-read /v2/core/status and rely on the revision CAS"
            ))
        })
    }

    /// Keeps the v1 wire-type echo truthful for v2 reconciles: committed only
    /// when the transaction actually put that core type in place, observed
    /// from the operation itself rather than assumed at admission.
    ///
    /// No bound on the wait: every operation reaches a terminal state, an
    /// executor death included, so the watcher always ends.
    fn watch_reconcile_echo(&self, handle: OperationHandle, core_type: CoreType) {
        let service = self.clone();
        let sequence = handle.sequence();
        tokio::spawn(async move {
            let state = match handle.wait().await {
                Ok(output) => OperationState::Succeeded(output),
                Err(error) => OperationState::Failed(error),
            };
            let commits = echo_commits(&state);
            if !commits
                && !matches!(
                    state,
                    OperationState::Succeeded(OperationOutput::Reconciled(_))
                )
            {
                // The transaction never reached a commit point, so it says
                // nothing about the echo either way.
                return;
            }
            let mut applied = service.inner.echo_applied.lock();
            if applied.is_some_and(|latest| latest >= sequence) {
                // A later reconcile already had the last word.
                return;
            }
            *applied = Some(sequence);
            if commits {
                service.publish_requested_core(Some(&core_type));
            } else {
                // A rollback restored the old spec, so the echo must not claim
                // the requested type — but the revision and state still moved,
                // so clients need the fresh snapshot. Same rule as v1 apply.
                service.publish_requested_core(None);
            }
        });
    }

    /// Publish the wire-type echo the bridge projects into status snapshots.
    ///
    /// `Some` commits a new type, `None` republishes the current one; either way
    /// the send notifies, which is the point. A rolled-back apply must not move
    /// the echo but still moved the revision and possibly the state, so its
    /// clients need the fresh snapshot too.
    ///
    /// `send_modify`, never `send`: `send` fails when nothing is subscribed and
    /// does not store the value, so a service whose bridges were never spawned
    /// would silently lose the echo and report `"type": null` from `/status`
    /// forever.
    fn publish_requested_core(&self, committed: Option<&CoreType>) {
        self.inner.requested_core.send_modify(|echo| {
            if let Some(core_type) = committed {
                *echo = Some(core_type.clone());
            }
        });
    }

    /// The manager-facing spec for a wire request.
    ///
    /// Returns `OpError` rather than `anyhow::Error` because this is where the
    /// binary lookup fails, and `find_binary_path`'s only possible error is a
    /// missing binary (`:601-616`) — a fact worth classifying. The other two
    /// failures here (a non-UTF-8 directory, an unsupported core kind) stay
    /// unclassified on purpose.
    fn instance_spec(
        &self,
        infos: &RuntimeInfos,
        core_type: &CoreType,
        config_path: Utf8PathBuf,
    ) -> Result<InstanceSpec, OpError> {
        let working_dir =
            Utf8PathBuf::from_path_buf(infos.nyanpasu_data_dir.clone()).map_err(|path| {
                OpError::plain(format!(
                    "nyanpasu data dir is not UTF-8: {}",
                    path.display()
                ))
            })?;
        let binary_path = find_binary_path(infos, core_type)
            .map_err(|error| OpError::with_kind(CoreErrorKind::BinaryNotFound, error.to_string()))
            .and_then(|path| {
                Utf8PathBuf::from_path_buf(path).map_err(|path| {
                    OpError::plain(format!("core binary path is not UTF-8: {}", path.display()))
                })
            })?;
        let kind = core_kind(core_type).map_err(|error| OpError::plain(error.to_string()))?;
        Ok(InstanceSpec {
            core: CoreSpec {
                kind,
                binary_path,
                version: None,
                features: Vec::new(),
            },
            config_path,
            working_dir,
            // The manager owns the pid record and points it at its runtime dir.
            pid_file: None,
            options: InstanceOptions::default(),
        })
    }
}

/// Manager transitions and requested-type commits → ws events.
///
/// Two sources, one emitter. `states` carries the manager's own snapshots and is
/// the sole input to the legacy `CoreStateChanged` stream, whose guards below
/// are unchanged. `requested_core` carries the adapter's wire-type echo, which
/// the manager knows nothing about: without watching it, a connected client
/// would keep the type it was last told about until the manager's next
/// transition, because committing the echo publishes nothing by itself.
///
/// Ordering, stated honestly: every frame is a snapshot of state observed at or
/// after the change that woke this task, and after any quiescent moment the last
/// frame carries the committed values. It is not a 1:1 log of changes — `watch`
/// coalesces, and when both sources move at once the two arms can emit two
/// `CoreStatusChanged` frames carrying the same status. Snapshots are
/// idempotent, so that is harmless.
///
/// This is a free function, not a closure, for two reasons: it must not capture
/// `Arc<Inner>` (its only exit condition is the manager's watch channel closing,
/// which cannot happen while the manager is alive), and a test can drive it with
/// hand-built channels.
async fn status_bridge(
    mut states: watch::Receiver<CoreStatus>,
    mut requested_core: watch::Receiver<Option<CoreType>>,
    hub: EventHub,
) {
    let raw = states.borrow_and_update().clone();
    let mut last = map_core_state(&raw.state);
    loop {
        tokio::select! {
            changed = states.changed() => {
                if changed.is_err() {
                    break;
                }
                let raw = states.borrow_and_update().clone();
                let next = map_core_state(&raw.state);
                // The snapshot leads and is never suppressed: it carries every
                // manager transition, including the ones the two-valued legacy
                // state cannot express (Starting, Restarting, and the
                // Stopping/Switching arrivals the guards below drop). Emitting
                // it here rather than after the guards is what keeps the legacy
                // path below untouched. (Until L3 there was a second reason not
                // to log here: the line would have come back round as an event
                // of its own. No tracing output becomes an event now.)
                let requested = requested_core.borrow_and_update().clone();
                hub.send(WsEvent::new_core_status_changed(project_core_infos(
                    &raw, requested,
                )));
                if matches!(
                    raw.state,
                    ManagerCoreState::Stopping { .. } | ManagerCoreState::Switching { .. }
                ) && matches!(last, CoreState::Stopped(_))
                {
                    continue;
                }
                if same_ipc_state(&last, &next) {
                    continue;
                }
                tracing::info!("State changed: {:?}", next);
                hub.send(WsEvent::new_core_state_changed(next.clone()));
                last = next;
            }
            changed = requested_core.changed() => {
                if changed.is_err() {
                    break;
                }
                // The echo moved without the manager moving, so this refreshes
                // the status snapshot and nothing else — the legacy state
                // carries no type and has no transition to report.
                // `states.borrow()`, never `borrow_and_update`: marking a
                // manager transition seen here would swallow it from the legacy
                // path above.
                let raw = states.borrow().clone();
                let requested = requested_core.borrow_and_update().clone();
                hub.send(WsEvent::new_core_status_changed(project_core_infos(
                    &raw, requested,
                )));
            }
        }
    }
}

/// One-way wire → manager mapping; the manager has no alpha variants.
fn core_kind(core_type: &CoreType) -> Result<CoreKind, anyhow::Error> {
    match core_type {
        CoreType::Clash(ClashCoreType::Mihomo | ClashCoreType::MihomoAlpha) => Ok(CoreKind::Mihomo),
        CoreType::Clash(ClashCoreType::ClashRust | ClashCoreType::ClashRustAlpha) => {
            Ok(CoreKind::ClashRust)
        }
        CoreType::Clash(ClashCoreType::ClashPremium) => Ok(CoreKind::ClashPremium),
        CoreType::Clash(ClashCoreType::Meow) => Ok(CoreKind::Meow),
        CoreType::SingBox => anyhow::bail!("sing-box is not a supported core"),
    }
}

/// Lossy projection onto the unchanged wire state.
fn map_core_state(state: &ManagerCoreState) -> CoreState {
    match state {
        ManagerCoreState::Running { .. }
        | ManagerCoreState::Switching { .. }
        | ManagerCoreState::Stopping { .. } => CoreState::Running,
        ManagerCoreState::Starting { .. } | ManagerCoreState::Restarting { .. } => {
            CoreState::Stopped(None)
        }
        ManagerCoreState::Stopped { reason } => {
            CoreState::Stopped(reason.as_ref().map(ToString::to_string))
        }
        // `CoreState` is `#[non_exhaustive]`; an unknown state is not proven running.
        _ => CoreState::Stopped(None),
    }
}

/// The controller endpoint, minus anything credential-bearing.
///
/// The manager never publishes the secret (only `ResolvedController.host`
/// reaches the watch channel), but an HTTP endpoint is parsed from the
/// caller's own `external-controller` value, which can carry userinfo — so
/// strip it here rather than trusting the shape of that string.
fn map_controller(host: &Host) -> Option<CoreControllerInfo> {
    match host {
        Host::NamedPipe(path) => Some(CoreControllerInfo::NamedPipe(path.clone())),
        Host::UnixSocket(path) => Some(CoreControllerInfo::UnixSocket(path.clone())),
        Host::Http(url) => {
            let mut url = url.clone();
            let _ = url.set_username("");
            let _ = url.set_password(None);
            Some(CoreControllerInfo::Http(url.to_string()))
        }
        // `Host` is `#[non_exhaustive]`; an unknown transport is reported as
        // absent rather than guessed at.
        _ => None,
    }
}

fn map_health(health: &HealthStatus) -> CoreHealthInfo {
    CoreHealthInfo {
        state: match health.state {
            HealthState::Starting => CoreHealthState::Starting,
            HealthState::Healthy => CoreHealthState::Healthy,
            HealthState::Unhealthy => CoreHealthState::Unhealthy,
            // `HealthState` is `#[non_exhaustive]`; an unknown state is not
            // proven healthy.
            _ => CoreHealthState::Unhealthy,
        },
        changed_at: health.changed_at,
        consecutive_failures: health.consecutive_failures,
        last_error: health.last_error.clone(),
        last_success_at: health.last_success_at,
    }
}

/// `runtime_path` is dropped on purpose: it points inside the manager's
/// 0o700 runtime directory, which the client cannot read.
fn map_revision(revision: &ConfigRevision) -> ConfigRevisionInfo {
    ConfigRevisionInfo {
        epoch: revision.epoch,
        generation: revision.generation,
        source_hash: revision.source_hash.clone(),
        effective_hash: revision.effective_hash.clone(),
    }
}

/// The wire CAS token, as the manager compares it.
fn map_revision_id(info: &RevisionIdInfo) -> RevisionId {
    RevisionId {
        epoch: info.epoch,
        generation: info.generation,
        effective_hash: info.effective_hash.clone(),
    }
}

/// Project an apply result onto the wire.
///
/// `DurabilityUncertain` is a wrapper, not an outcome, and the apply path can
/// wrap twice — a commit warning around a restore warning
/// (`manager/apply.rs:148` around `:445`) — so unwrap to the real outcome and
/// keep every warning. `ApplyOutcome` is not `#[non_exhaustive]`: if the
/// manager grows a variant, this match must fail to compile rather than
/// silently mislabel it.
/// Whether an operation's terminal state committed the core type it asked for.
/// Only a reconcile can, and only one that was not rolled back:
/// `DurabilityUncertain` wraps the real outcome and is unwrapped first.
fn echo_commits(state: &OperationState) -> bool {
    let OperationState::Succeeded(OperationOutput::Reconciled(outcome)) = state else {
        return false;
    };
    let mut effective = outcome;
    while let ApplyOutcome::DurabilityUncertain { outcome, .. } = effective {
        effective = &**outcome;
    }
    !matches!(effective, ApplyOutcome::RolledBack { .. })
}

fn op_error_from_control(error: ControlError) -> OpError {
    let retryable = error.retryable;
    match error.kind {
        Some(kind) => OpError::with_kind(kind, error.message),
        None => OpError::plain(error.message),
    }
    .retryable(retryable)
}

fn map_operation(id: OperationId, state: OperationState) -> OperationInfo {
    let (phase, output, error) = match state {
        OperationState::Queued => (OperationPhase::Queued, None, None),
        OperationState::Running => (OperationPhase::Running, None, None),
        OperationState::Succeeded(output) => (
            OperationPhase::Succeeded,
            Some(map_operation_output(output)),
            None,
        ),
        OperationState::Failed(failure) => (
            OperationPhase::Failed,
            None,
            Some(OperationErrorInfo {
                kind: failure.kind.map(|kind| Cow::Borrowed(kind.as_str())),
                message: failure.message,
                retryable: failure.retryable,
            }),
        ),
    };
    OperationInfo {
        id: id.to_string(),
        phase,
        output,
        error,
    }
}

fn map_operation_output(output: OperationOutput) -> OperationOutputInfo {
    match output {
        OperationOutput::Reconciled(outcome) => {
            OperationOutputInfo::Reconciled(map_reconcile_outcome(&outcome))
        }
        OperationOutput::Stopped => OperationOutputInfo::Stopped,
        OperationOutput::Recovered => OperationOutputInfo::Recovered,
        OperationOutput::ShutDown => OperationOutputInfo::ShutDown,
    }
}

/// The v2 sibling of [`map_apply_outcome`]: same durability unwrapping, plus
/// the `Started` variant only `reconcile` produces.
fn map_reconcile_outcome(outcome: &ApplyOutcome) -> ReconcileOutcomeInfo {
    let mut warnings = Vec::new();
    let mut current = outcome;
    while let ApplyOutcome::DurabilityUncertain { outcome, warning } = current {
        warnings.push(warning.clone());
        current = &**outcome;
    }
    let (kind, revision, failed_apply) = match current {
        ApplyOutcome::Started { revision } => (ReconcileOutcomeKind::Started, revision, None),
        ApplyOutcome::Noop { revision } => (ReconcileOutcomeKind::Noop, revision, None),
        ApplyOutcome::Patched { revision } => (ReconcileOutcomeKind::Patched, revision, None),
        ApplyOutcome::Reloaded { revision } => (ReconcileOutcomeKind::Reloaded, revision, None),
        ApplyOutcome::Restarted { revision } => (ReconcileOutcomeKind::Restarted, revision, None),
        ApplyOutcome::Switched { revision } => (ReconcileOutcomeKind::Switched, revision, None),
        ApplyOutcome::RolledBack {
            revision,
            failed_apply,
        } => (
            ReconcileOutcomeKind::RolledBack,
            revision,
            Some(failed_apply.clone()),
        ),
        ApplyOutcome::DurabilityUncertain { .. } => {
            unreachable!("unwrapped by the loop above")
        }
    };
    ReconcileOutcomeInfo {
        outcome: kind,
        revision: map_revision(revision),
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
        failed_apply,
    }
}

fn map_apply_outcome(outcome: &ApplyOutcome) -> CoreApplyData {
    let mut warnings = Vec::new();
    let mut current = outcome;
    while let ApplyOutcome::DurabilityUncertain { outcome, warning } = current {
        warnings.push(warning.clone());
        current = &**outcome;
    }
    let (kind, revision, failed_apply) = match current {
        ApplyOutcome::Noop { revision } => (ApplyOutcomeKind::Noop, revision, None),
        ApplyOutcome::Patched { revision } => (ApplyOutcomeKind::Patched, revision, None),
        ApplyOutcome::Reloaded { revision } => (ApplyOutcomeKind::Reloaded, revision, None),
        ApplyOutcome::Restarted { revision } => (ApplyOutcomeKind::Restarted, revision, None),
        // Live since S10: the manager reports the core-switch path separately
        // from a same-epoch restart, so the wire value S8 declared and never
        // sent is finally produced here.
        ApplyOutcome::Switched { revision } => (ApplyOutcomeKind::Switched, revision, None),
        ApplyOutcome::RolledBack {
            revision,
            failed_apply,
        } => (
            ApplyOutcomeKind::RolledBack,
            revision,
            Some(failed_apply.clone()),
        ),
        ApplyOutcome::DurabilityUncertain { .. } => {
            unreachable!("unwrapped by the loop above")
        }
        // The v1 apply handler feeds this from `apply_config`, which requires
        // a running core; only the v2 `reconcile` path can cold-start.
        ApplyOutcome::Started { .. } => {
            unreachable!("apply_config never cold-starts a core")
        }
    };
    CoreApplyData {
        outcome: kind,
        revision: map_revision(revision),
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
        failed_apply,
    }
}

/// The lossless counterpart to `map_core_state`.
fn map_state_detail(state: &ManagerCoreState) -> Option<CoreStateDetail> {
    match state {
        ManagerCoreState::Stopped { reason } => Some(CoreStateDetail::Stopped {
            reason: reason.as_ref().map(ToString::to_string),
        }),
        ManagerCoreState::Starting { epoch } => Some(CoreStateDetail::Starting { epoch: *epoch }),
        ManagerCoreState::Running { epoch, pid } => Some(CoreStateDetail::Running {
            epoch: *epoch,
            pid: *pid,
        }),
        ManagerCoreState::Restarting { epoch, attempt } => Some(CoreStateDetail::Restarting {
            epoch: *epoch,
            attempt: *attempt,
        }),
        ManagerCoreState::Switching { from, to } => Some(CoreStateDetail::Switching {
            from: *from,
            to: *to,
        }),
        ManagerCoreState::Stopping { epoch } => Some(CoreStateDetail::Stopping { epoch: *epoch }),
        // `CoreState` is `#[non_exhaustive]`; an unknown state has no faithful
        // projection, so it is reported as absent.
        _ => None,
    }
}

/// The wire type carries no `PartialEq`, so equality is spelled out here.
fn same_ipc_state(previous: &CoreState, next: &CoreState) -> bool {
    match (previous, next) {
        (CoreState::Running, CoreState::Running) => true,
        (CoreState::Stopped(previous), CoreState::Stopped(next)) => previous == next,
        _ => false,
    }
}

/// The wire projection of a manager snapshot.
///
/// One function so `/status` and the `CoreStatusChanged` frame cannot drift:
/// the event *is* the snapshot (report §4 P1-A). `state` stays the lossy
/// two-valued field the GUI has always consumed; `detail` is the faithful
/// six-state view beside it.
fn project_core_infos(status: &CoreStatus, requested_core: Option<CoreType>) -> CoreInfos {
    CoreInfos {
        r#type: requested_core,
        state: map_core_state(&status.state),
        state_changed_at: status.changed_at,
        config_path: status
            .spec
            .as_ref()
            .map(|spec| spec.config_path.as_std_path().to_path_buf()),
        controller: status.controller.as_ref().and_then(map_controller),
        health: status.health.as_ref().map(map_health),
        revision: status.revision.as_ref().map(map_revision),
        detail: map_state_detail(&status.state),
    }
}

/// Mirror the core's console output into the service's own tracing stream, for
/// an operator watching a terminal.
///
/// Nothing here is above `DEBUG`, deliberately. Core output now has two homes
/// that outlive the process — the JSONL archive and the event stream — so
/// putting it in the service's log file at the default filter would be a third
/// copy of the same bytes. `RUST_LOG=nyanpasu_service::core=debug` turns it back
/// on by itself, without the noise of a full `--verbose`.
///
/// The severity is not lost, only un-promoted: `core_level` carries the parsed
/// level as a field. That also retires a real defect — the level used to come
/// from the *pipe*, so every stderr line was reported as an error however
/// harmless it was.
fn forward_log(frame: &LogFrame) {
    let kind = frame.kind;
    let epoch = frame.epoch;
    let stream = frame.stream;
    let core_level = frame.level;
    let raw = &frame.raw;
    match core_level {
        LogLevel::Trace => {
            tracing::trace!(target: CORE_LOG_TARGET, ?core_level, ?stream, %kind, epoch, "{raw}")
        }
        _ => {
            tracing::debug!(target: CORE_LOG_TARGET, ?core_level, ?stream, %kind, epoch, "{raw}")
        }
    }
}

// TODO: support system path search via a config or flag
/// Search the binary path of the core: Data Dir -> Sidecar Dir
fn find_binary_path(infos: &RuntimeInfos, core_type: &CoreType) -> std::io::Result<PathBuf> {
    let data_dir = &infos.nyanpasu_data_dir;
    let binary_path = data_dir.join(core_type.get_executable_name());
    if binary_path.exists() {
        return Ok(binary_path);
    }
    let app_dir = &infos.nyanpasu_app_dir;
    let binary_path = app_dir.join(core_type.get_executable_name());
    if binary_path.exists() {
        return Ok(binary_path);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{} not found", core_type.get_executable_name()),
    ))
}

/// The absolute, non-verbatim, UTF-8 path the manager is handed for a config.
///
/// Same shape as `start`'s inline version: canonicalizing makes the source path
/// a stable identity, and `dunce::simplified` strips the Windows `\\?\` prefix
/// the core's own command line cannot use. Canonicalization is also the
/// existence check, which is why `NotFound` is classified here and nothing else
/// is: any other io error is a permission or device failure this layer cannot
/// name without guessing. The path stays in the message — the caller supplied
/// it and it is the only way to tell which of two paths failed.
async fn canonical_config_path(config_file: &Path) -> Result<Utf8PathBuf, OpError> {
    let canonical = tokio::fs::canonicalize(config_file)
        .await
        .map_err(|error| {
            let message = format!(
                "failed to resolve config path {}: {error}",
                config_file.display()
            );
            match error.kind() {
                std::io::ErrorKind::NotFound => {
                    OpError::with_kind(CoreErrorKind::ConfigNotFound, message)
                }
                _ => OpError::plain(message),
            }
        })?;
    Utf8PathBuf::from_path_buf(dunce::simplified(&canonical).to_path_buf())
        .map_err(|path| OpError::plain(format!("config path is not UTF-8: {}", path.display())))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use nyanpasu_core_manager::StopReason;
    use nyanpasu_ipc::api::ws::events::Event as TestEvent;
    use tokio::sync::watch;

    use super::*;

    fn simulate(states: &[ManagerCoreState]) -> Vec<String> {
        let mut last = CoreState::Stopped(None);
        let mut emitted = Vec::new();
        for raw in states {
            let next = map_core_state(raw);
            if matches!(
                raw,
                ManagerCoreState::Stopping { .. } | ManagerCoreState::Switching { .. }
            ) && matches!(last, CoreState::Stopped(_))
            {
                continue;
            }
            if same_ipc_state(&last, &next) {
                continue;
            }
            emitted.push(format!("{next:?}"));
            last = next;
        }
        emitted
    }

    // covered by S2 route-level tests

    #[test]
    fn manager_states_map_onto_the_wire_states() {
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Stopped { reason: None })
            ),
            "Stopped(None)"
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Starting { epoch: 1 })
            ),
            "Stopped(None)"
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Running { epoch: 1, pid: 42 })
            ),
            "Running"
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Restarting {
                    epoch: 1,
                    attempt: 2,
                })
            ),
            "Stopped(None)"
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Switching {
                    from: Some(1),
                    to: 2,
                })
            ),
            "Running"
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Stopping { epoch: 2 })
            ),
            "Running"
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Stopped {
                    reason: Some(StopReason::Finished),
                })
            ),
            r#"Stopped(Some("core exited"))"#
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Stopped {
                    reason: Some(StopReason::User),
                })
            ),
            r#"Stopped(Some("stopped by user"))"#
        );
        assert_eq!(
            format!(
                "{:?}",
                map_core_state(&ManagerCoreState::Stopped {
                    reason: Some(StopReason::Error("boom".to_owned())),
                })
            ),
            r#"Stopped(Some("boom"))"#
        );
    }

    #[test]
    fn core_types_map_onto_manager_kinds() {
        let cases = [
            (ClashCoreType::Mihomo, CoreKind::Mihomo),
            (ClashCoreType::MihomoAlpha, CoreKind::Mihomo),
            (ClashCoreType::ClashRust, CoreKind::ClashRust),
            (ClashCoreType::ClashRustAlpha, CoreKind::ClashRust),
            (ClashCoreType::ClashPremium, CoreKind::ClashPremium),
            (ClashCoreType::Meow, CoreKind::Meow),
        ];
        for (core_type, expected) in cases {
            assert_eq!(core_kind(&CoreType::Clash(core_type)).unwrap(), expected);
        }
        assert!(core_kind(&CoreType::SingBox).is_err());
    }

    #[test]
    fn the_state_bridge_forwards_only_real_transitions() {
        let states = [
            ManagerCoreState::Starting { epoch: 1 },
            ManagerCoreState::Running { epoch: 1, pid: 42 },
            ManagerCoreState::Switching {
                from: Some(1),
                to: 2,
            },
            ManagerCoreState::Stopping { epoch: 2 },
            ManagerCoreState::Running { epoch: 2, pid: 43 },
            ManagerCoreState::Stopped {
                reason: Some(StopReason::User),
            },
            ManagerCoreState::Stopped {
                reason: Some(StopReason::Error("boom".to_owned())),
            },
        ];

        assert_eq!(
            simulate(&states),
            [
                "Running",
                r#"Stopped(Some("stopped by user"))"#,
                r#"Stopped(Some("boom"))"#,
            ]
        );
    }

    #[test]
    fn terminal_stop_is_not_resurrected_by_shutdown() {
        let states = [
            ManagerCoreState::Running { epoch: 1, pid: 42 },
            ManagerCoreState::Stopped {
                reason: Some(StopReason::Error("boom".to_owned())),
            },
            ManagerCoreState::Stopping { epoch: 1 },
            ManagerCoreState::Stopped {
                reason: Some(StopReason::User),
            },
        ];

        assert_eq!(
            simulate(&states),
            [
                "Running",
                r#"Stopped(Some("boom"))"#,
                r#"Stopped(Some("stopped by user"))"#,
            ]
        );
    }

    #[test]
    fn manager_states_map_onto_the_wire_state_detail() {
        let cases = [
            (
                ManagerCoreState::Stopped { reason: None },
                CoreStateDetail::Stopped { reason: None },
            ),
            (
                ManagerCoreState::Stopped {
                    reason: Some(StopReason::User),
                },
                CoreStateDetail::Stopped {
                    reason: Some("stopped by user".to_owned()),
                },
            ),
            (
                ManagerCoreState::Starting { epoch: 1 },
                CoreStateDetail::Starting { epoch: 1 },
            ),
            (
                ManagerCoreState::Running { epoch: 1, pid: 42 },
                CoreStateDetail::Running { epoch: 1, pid: 42 },
            ),
            (
                ManagerCoreState::Restarting {
                    epoch: 1,
                    attempt: 2,
                },
                CoreStateDetail::Restarting {
                    epoch: 1,
                    attempt: 2,
                },
            ),
            (
                ManagerCoreState::Switching {
                    from: Some(1),
                    to: 2,
                },
                CoreStateDetail::Switching {
                    from: Some(1),
                    to: 2,
                },
            ),
            (
                ManagerCoreState::Stopping { epoch: 2 },
                CoreStateDetail::Stopping { epoch: 2 },
            ),
        ];
        for (raw, expected) in cases {
            assert_eq!(map_state_detail(&raw), Some(expected));
        }
    }

    /// The detail projection must disagree with the lossy wire state exactly
    /// where the report says it does: a crash loop and a start-up both look
    /// stopped on the old field.
    #[test]
    fn the_detail_field_recovers_what_the_wire_state_flattens() {
        let restarting = ManagerCoreState::Restarting {
            epoch: 1,
            attempt: 3,
        };
        assert!(matches!(
            map_core_state(&restarting),
            CoreState::Stopped(None)
        ));
        assert_eq!(
            map_state_detail(&restarting),
            Some(CoreStateDetail::Restarting {
                epoch: 1,
                attempt: 3,
            })
        );
    }

    #[test]
    fn controller_hosts_map_onto_the_wire_controller() {
        assert_eq!(
            map_controller(&Host::named_pipe(r"\\.\pipe\nyanpasu\core-1")),
            Some(CoreControllerInfo::NamedPipe(std::path::PathBuf::from(
                r"\\.\pipe\nyanpasu\core-1"
            )))
        );
        assert_eq!(
            map_controller(&Host::unix_socket("/run/nyanpasu/core-1.sock")),
            Some(CoreControllerInfo::UnixSocket(std::path::PathBuf::from(
                "/run/nyanpasu/core-1.sock"
            )))
        );
        assert_eq!(
            map_controller(&Host::http("127.0.0.1:9090").unwrap()),
            Some(CoreControllerInfo::Http(
                "http://127.0.0.1:9090/".to_owned()
            ))
        );
    }

    /// The HTTP endpoint is parsed from the caller's own config value; if that
    /// value carried userinfo, it must not reach the wire.
    #[test]
    fn http_controller_credentials_never_reach_the_wire() {
        for raw in [
            "http://user:pass@127.0.0.1:9090",
            "http://user@127.0.0.1:9090",
        ] {
            assert_eq!(
                map_controller(&Host::http(raw).unwrap()),
                Some(CoreControllerInfo::Http(
                    "http://127.0.0.1:9090/".to_owned()
                )),
                "raw = {raw}"
            );
        }
    }

    #[test]
    fn the_revision_projection_drops_the_runtime_path() {
        let revision = ConfigRevision {
            epoch: 3,
            generation: 7,
            source_hash: "0123456789abcdef".to_owned(),
            effective_hash: "fedcba9876543210".to_owned(),
            runtime_path: "/srv/data/core-runtime/config-3.yaml".into(),
        };
        assert_eq!(
            map_revision(&revision),
            ConfigRevisionInfo {
                epoch: 3,
                generation: 7,
                source_hash: "0123456789abcdef".to_owned(),
                effective_hash: "fedcba9876543210".to_owned(),
            }
        );
    }

    #[test]
    fn health_states_map_onto_the_wire_health() {
        // `HealthStatus::starting()` is `pub(crate)` to the manager
        // (`state.rs:41`) and unreachable from here, but all five fields are
        // `pub` and the struct is not `#[non_exhaustive]`.
        let health = HealthStatus {
            state: HealthState::Unhealthy,
            changed_at: 1_700_000_000_123,
            consecutive_failures: 4,
            last_error: Some("probe timed out".to_owned()),
            last_success_at: Some(1_700_000_000_000),
        };
        let mapped = map_health(&health);
        assert_eq!(mapped.state, CoreHealthState::Unhealthy);
        assert_eq!(mapped.changed_at, 1_700_000_000_123);
        assert_eq!(mapped.consecutive_failures, 4);
        assert_eq!(mapped.last_error.as_deref(), Some("probe timed out"));
        assert_eq!(mapped.last_success_at, Some(1_700_000_000_000));
    }

    fn revision(generation: u64) -> ConfigRevision {
        ConfigRevision {
            epoch: 3,
            generation,
            source_hash: "0123456789abcdef".to_owned(),
            effective_hash: "fedcba9876543210".to_owned(),
            runtime_path: "/srv/data/core-runtime/config-3.yaml".into(),
        }
    }

    #[test]
    fn apply_outcomes_map_onto_the_wire_kinds() {
        let cases = [
            (
                ApplyOutcome::Noop {
                    revision: revision(7),
                },
                ApplyOutcomeKind::Noop,
            ),
            (
                ApplyOutcome::Patched {
                    revision: revision(8),
                },
                ApplyOutcomeKind::Patched,
            ),
            (
                ApplyOutcome::Reloaded {
                    revision: revision(9),
                },
                ApplyOutcomeKind::Reloaded,
            ),
            (
                ApplyOutcome::Restarted {
                    revision: revision(10),
                },
                ApplyOutcomeKind::Restarted,
            ),
            (
                ApplyOutcome::Switched {
                    revision: revision(11),
                },
                ApplyOutcomeKind::Switched,
            ),
        ];
        for (outcome, expected) in cases {
            let data = map_apply_outcome(&outcome);
            assert_eq!(data.outcome, expected);
            assert_eq!(data.revision.effective_hash, "fedcba9876543210");
            assert!(data.warning.is_none());
            assert!(data.failed_apply.is_none());
        }
    }

    /// The report's sharpest requirement: a rollback must not be
    /// indistinguishable from a success. The core is running the OLD config, so
    /// the reported revision is the old one and the rejection reason comes with
    /// it.
    #[test]
    fn a_rolled_back_apply_reports_the_old_revision_and_the_failure() {
        let data = map_apply_outcome(&ApplyOutcome::RolledBack {
            revision: revision(7),
            failed_apply: "core failed to start".to_owned(),
        });
        assert_eq!(data.outcome, ApplyOutcomeKind::RolledBack);
        assert_eq!(data.revision.generation, 7);
        assert_eq!(data.failed_apply.as_deref(), Some("core failed to start"));
    }

    /// `DurabilityUncertain` is a wrapper, and the apply path can wrap twice —
    /// a commit warning around a restore warning. Both must survive, and the
    /// wrapper must never be reported as the outcome.
    #[test]
    fn durability_warnings_unwrap_to_the_real_outcome() {
        let single = ApplyOutcome::DurabilityUncertain {
            outcome: Box::new(ApplyOutcome::Patched {
                revision: revision(8),
            }),
            warning: "directory sync failed".to_owned(),
        };
        let data = map_apply_outcome(&single);
        assert_eq!(data.outcome, ApplyOutcomeKind::Patched);
        assert_eq!(data.warning.as_deref(), Some("directory sync failed"));

        let nested = ApplyOutcome::DurabilityUncertain {
            outcome: Box::new(ApplyOutcome::DurabilityUncertain {
                outcome: Box::new(ApplyOutcome::RolledBack {
                    revision: revision(7),
                    failed_apply: "boom".to_owned(),
                }),
                warning: "restore sync failed".to_owned(),
            }),
            warning: "commit sync failed".to_owned(),
        };
        let data = map_apply_outcome(&nested);
        assert_eq!(data.outcome, ApplyOutcomeKind::RolledBack);
        assert_eq!(
            data.warning.as_deref(),
            Some("commit sync failed; restore sync failed")
        );
        assert_eq!(data.failed_apply.as_deref(), Some("boom"));
    }

    /// `apply` on a stopped core answers with the same string `restart` does,
    /// so a GUI branching on it keeps working, and gains the kind beside it.
    #[test]
    fn the_not_started_failure_keeps_the_legacy_wire_string() {
        let error = OpError::from(ManagerError::NotStarted);
        assert_eq!(error.message, MSG_CORE_NOT_STARTED);
        assert_eq!(error.kind, Some(CoreErrorKind::NotStarted));
    }

    #[test]
    fn expected_revisions_cross_the_wire_unchanged() {
        assert_eq!(
            map_revision_id(&RevisionIdInfo {
                epoch: 3,
                generation: 7,
                effective_hash: "fedcba9876543210".to_owned(),
            }),
            RevisionId {
                epoch: 3,
                generation: 7,
                effective_hash: "fedcba9876543210".to_owned(),
            }
        );
    }

    fn status_of(state: ManagerCoreState) -> CoreStatus {
        CoreStatus {
            state,
            changed_at: 42,
            health: None,
            spec: None,
            controller: None,
            revision: None,
        }
    }

    fn mihomo() -> CoreType {
        CoreType::Clash(ClashCoreType::Mihomo)
    }

    /// The `type` a `CoreStatusChanged` frame carries, or a panic naming what
    /// arrived instead.
    fn status_frame_type(event: TestEvent) -> Option<CoreType> {
        match event {
            TestEvent::CoreStatusChanged(infos) => infos.r#type,
            other => panic!("expected a CoreStatusChanged frame, got {other:?}"),
        }
    }

    async fn test_service() -> (tempfile::TempDir, CoreManagerService) {
        let dir = tempfile::tempdir().expect("tempdir");
        let runtime_dir = Utf8PathBuf::from_path_buf(dir.path().join("core-runtime"))
            .expect("temp path is UTF-8");
        let data_dir = Utf8PathBuf::from_path_buf(dir.path().join("nyanpasu-data"))
            .expect("temp path is UTF-8");
        let service = CoreManagerService::new(runtime_dir, LocalIpcPolicy::Disable, data_dir)
            .await
            .expect("the manager builds on a fresh runtime dir");
        (dir, service)
    }

    fn test_infos(root: &std::path::Path) -> RuntimeInfos {
        RuntimeInfos {
            service_data_dir: root.join("service-data"),
            service_config_dir: root.join("service-config"),
            nyanpasu_config_dir: root.join("nyanpasu-config"),
            nyanpasu_data_dir: root.join("nyanpasu-data"),
            nyanpasu_app_dir: root.join("nyanpasu-app"),
        }
    }

    /// The review finding, pinned: committing the echo has to reach a client
    /// that is already connected. The manager publishes nothing when the
    /// adapter's wire-type echo moves, so before this the frames from a
    /// cross-core apply carried the *previous* type until the next transition.
    #[tokio::test]
    async fn committing_the_requested_type_publishes_a_fresh_status_snapshot() {
        let states = watch::Sender::new(status_of(ManagerCoreState::Stopped { reason: None }));
        let requested = watch::Sender::new(None);
        let hub = EventHub::new();
        let mut events = hub.subscribe();
        let task = tokio::spawn(status_bridge(
            states.subscribe(),
            requested.subscribe(),
            hub.clone(),
        ));

        // A no-op echo notification proves the bridge has consumed its initial
        // manager snapshot before the transition below can be published.
        requested.send_modify(|_| {});
        let handshake = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("timed out waiting for the bridge readiness snapshot")
            .expect("the event hub must stay open");
        assert_eq!(status_frame_type(handshake), None);

        // A manager transition with no echo yet: the snapshot leads, the legacy
        // state follows.
        states.send_replace(status_of(ManagerCoreState::Running { epoch: 1, pid: 7 }));
        let status = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("timed out waiting for the manager status snapshot")
            .expect("the event hub must stay open");
        assert_eq!(status_frame_type(status), None);
        let state = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("timed out waiting for the legacy state transition")
            .expect("the event hub must stay open");
        assert!(matches!(
            state,
            TestEvent::CoreStateChanged(CoreState::Running)
        ));

        // The commit: one status snapshot carrying the committed type, and no
        // legacy frame — the legacy state carries no type and has no transition
        // to report.
        requested.send_replace(Some(mihomo()));
        let committed = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("timed out waiting for the committed type snapshot")
            .expect("the event hub must stay open");
        assert_eq!(status_frame_type(committed), Some(mihomo()));

        // Closing the manager's channel ends the task, so anything it was going
        // to send has been sent: the emptiness below is a fact, not a race.
        drop(states);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the bridge must exit when the manager drops")
            .expect("the bridge must not panic");
        assert!(
            events.try_recv().is_err(),
            "the echo commit must not produce a legacy state frame"
        );
    }

    /// A rolled-back apply must not move the echo, but its clients still need
    /// the snapshot: the revision moved even though the type did not. The
    /// adapter republishes the unchanged value, which `send_modify` notifies on.
    #[tokio::test]
    async fn republishing_an_unchanged_echo_still_refreshes_the_snapshot() {
        let states = watch::Sender::new(status_of(ManagerCoreState::Running { epoch: 1, pid: 7 }));
        let requested = watch::Sender::new(Some(mihomo()));
        let hub = EventHub::new();
        let mut events = hub.subscribe();
        let task = tokio::spawn(status_bridge(
            states.subscribe(),
            requested.subscribe(),
            hub.clone(),
        ));

        requested.send_modify(|_| {});
        let refreshed = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("timed out waiting for the refreshed status snapshot")
            .expect("the event hub must stay open");
        assert_eq!(status_frame_type(refreshed), Some(mihomo()));

        drop(states);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the bridge must exit when the manager drops")
            .expect("the bridge must not panic");
    }

    /// The invariant behind `Inner`'s comment: the task exits when the manager's
    /// watch channel closes. It must not be kept alive by the echo channel,
    /// which outlives nothing — a task that survives its manager would hold a
    /// subscription forever and never emit again.
    #[tokio::test]
    async fn the_status_bridge_exits_with_the_manager_even_while_the_echo_lives() {
        let states = watch::Sender::new(status_of(ManagerCoreState::Stopped { reason: None }));
        let requested = watch::Sender::new(None);
        let task = tokio::spawn(status_bridge(
            states.subscribe(),
            requested.subscribe(),
            EventHub::new(),
        ));
        drop(states);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the bridge must exit when the manager drops")
            .expect("the bridge must not panic");
        // The echo channel is still open and still writable; nobody is listening.
        requested.send_modify(|_| {});
    }

    /// `watch::Sender::send` fails when nothing is subscribed and drops the
    /// value on the floor. A service whose bridges were never spawned — every
    /// route test, and the window before `run` spawns them — would then report
    /// `"type": null` from `/status` forever.
    #[tokio::test]
    async fn the_echo_survives_a_service_with_no_bridge_subscribed() {
        let (_dir, service) = test_service().await;
        assert!(service.status().await.r#type.is_none());
        service.publish_requested_core(Some(&mihomo()));
        assert_eq!(service.status().await.r#type, Some(mihomo()));
    }

    /// Each check spawns a core binary, so the overflow is refused rather than
    /// queued. The permit is taken before the path is resolved, which is what
    /// makes the refusal observable here — and what proves an unresolvable path
    /// is *not* what produced it: with a slot free, the same call reaches the
    /// path and reports it with its kind.
    #[tokio::test]
    async fn a_third_concurrent_check_is_refused_before_it_can_spawn_a_core() {
        let (dir, service) = test_service().await;
        let infos = test_infos(dir.path());
        let missing = dir.path().join("nope.yaml");

        let held: Vec<_> = (0..MAX_CONCURRENT_CHECKS)
            .map(|_| {
                service
                    .inner
                    .check_slots
                    .try_acquire()
                    .expect("a fresh service has every slot free")
            })
            .collect();
        let refused = service
            .check(&infos, &mihomo(), &missing)
            .await
            .expect_err("the overflow must be refused");
        assert!(
            refused.message.contains("config checks may run at once"),
            "unexpected refusal: {}",
            refused.message
        );
        assert!(refused.kind.is_none(), "a refusal is not a manager error");

        drop(held);
        let resolved = service
            .check(&infos, &mihomo(), &missing)
            .await
            .expect_err("the path does not exist");
        assert_eq!(resolved.kind, Some(CoreErrorKind::ConfigNotFound));
        assert!(
            resolved.message.contains("nope.yaml"),
            "the failure must name the path: {}",
            resolved.message
        );
    }

    /// A missing core binary is the only failure `find_binary_path` can produce,
    /// so `apply`/`check` classify it instead of sending an unclassified
    /// envelope the GUI has to string-match.
    #[tokio::test]
    async fn a_missing_core_binary_is_classified_on_the_check_path() {
        let (dir, service) = test_service().await;
        let infos = test_infos(dir.path());
        std::fs::create_dir_all(&infos.nyanpasu_data_dir).unwrap();
        let config = infos.nyanpasu_data_dir.join("config.yaml");
        std::fs::write(&config, b"mixed-port: 7890\n").unwrap();

        let error = service
            .check(&infos, &mihomo(), &config)
            .await
            .expect_err("no binary exists under either dir");
        assert_eq!(error.kind, Some(CoreErrorKind::BinaryNotFound));
        assert!(
            error.message.contains(mihomo().get_executable_name()),
            "the failure must name the binary: {}",
            error.message
        );
    }

    /// Mirrors the restructured bridge loop: what the legacy `CoreStateChanged`
    /// stream carries (after the unchanged suppression rules) and what the
    /// snapshot stream carries (one per manager transition, none suppressed).
    fn simulate_both(statuses: &[CoreStatus]) -> (Vec<String>, Vec<Option<CoreStateDetail>>) {
        let mut last = CoreState::Stopped(None);
        let mut legacy = Vec::new();
        let mut snapshots = Vec::new();
        for raw in statuses {
            let next = map_core_state(&raw.state);
            snapshots.push(project_core_infos(raw, None).detail);
            if matches!(
                raw.state,
                ManagerCoreState::Stopping { .. } | ManagerCoreState::Switching { .. }
            ) && matches!(last, CoreState::Stopped(_))
            {
                continue;
            }
            if same_ipc_state(&last, &next) {
                continue;
            }
            legacy.push(format!("{next:?}"));
            last = next;
        }
        (legacy, snapshots)
    }

    /// The point of S9, restated after S11: the legacy stream keeps suppressing
    /// exactly what it always suppressed, while the snapshot stream carries
    /// every transition — including the crash-loop and start-up ones the
    /// two-valued legacy state cannot express (report §1.2).
    #[test]
    fn the_snapshot_stream_carries_every_transition_the_legacy_stream_suppresses() {
        let states = [
            ManagerCoreState::Starting { epoch: 1 },
            ManagerCoreState::Running { epoch: 1, pid: 42 },
            ManagerCoreState::Restarting {
                epoch: 1,
                attempt: 2,
            },
            ManagerCoreState::Running { epoch: 1, pid: 43 },
            ManagerCoreState::Stopping { epoch: 1 },
            ManagerCoreState::Stopped {
                reason: Some(StopReason::User),
            },
        ];
        let statuses: Vec<CoreStatus> = states.iter().cloned().map(status_of).collect();
        let (legacy, snapshots) = simulate_both(&statuses);

        // The lossy legacy stream, defect included: the restart in the middle
        // is reported as a stop, which is why the snapshot variant exists.
        assert_eq!(
            legacy,
            [
                "Running",
                "Stopped(None)",
                "Running",
                r#"Stopped(Some("stopped by user"))"#,
            ]
        );
        // …and the mirror above agrees with the untouched legacy helper.
        assert_eq!(legacy, simulate(&states));
        // One faithful frame per manager transition, none dropped.
        assert_eq!(
            snapshots,
            [
                Some(CoreStateDetail::Starting { epoch: 1 }),
                Some(CoreStateDetail::Running { epoch: 1, pid: 42 }),
                Some(CoreStateDetail::Restarting {
                    epoch: 1,
                    attempt: 2,
                }),
                Some(CoreStateDetail::Running { epoch: 1, pid: 43 }),
                Some(CoreStateDetail::Stopping { epoch: 1 }),
                Some(CoreStateDetail::Stopped {
                    reason: Some("stopped by user".to_owned()),
                }),
            ]
        );
    }

    /// The snapshot payload is the S7 projection unchanged: `state` stays the
    /// lossy field the wire has always carried, `detail` is the faithful one,
    /// and the type echo comes from the adapter rather than the manager.
    #[test]
    fn the_snapshot_payload_keeps_the_lossy_state_and_adds_the_faithful_detail() {
        let infos = project_core_infos(
            &status_of(ManagerCoreState::Restarting {
                epoch: 3,
                attempt: 2,
            }),
            Some(CoreType::Clash(ClashCoreType::MihomoAlpha)),
        );
        assert!(matches!(infos.state, CoreState::Stopped(None)));
        assert_eq!(
            infos.detail,
            Some(CoreStateDetail::Restarting {
                epoch: 3,
                attempt: 2
            })
        );
        assert_eq!(
            infos.r#type,
            Some(CoreType::Clash(ClashCoreType::MihomoAlpha))
        );
        assert_eq!(infos.state_changed_at, 42);
        assert!(infos.controller.is_none());
        assert!(infos.health.is_none());
        assert!(infos.revision.is_none());
        assert!(infos.config_path.is_none());
    }

    /// The echo names the core type a reconcile *put in place*. Anything else
    /// — a rollback, a non-reconcile outcome, a failure — must leave it alone.
    #[test]
    fn only_a_committed_reconcile_moves_the_wire_type_echo() {
        for outcome in [
            ApplyOutcome::Started {
                revision: revision(1),
            },
            ApplyOutcome::Noop {
                revision: revision(2),
            },
            ApplyOutcome::Patched {
                revision: revision(3),
            },
            ApplyOutcome::Reloaded {
                revision: revision(4),
            },
            ApplyOutcome::Restarted {
                revision: revision(5),
            },
            ApplyOutcome::Switched {
                revision: revision(6),
            },
        ] {
            assert!(
                super::echo_commits(&OperationState::Succeeded(OperationOutput::Reconciled(
                    outcome.clone()
                ))),
                "{outcome:?} committed the requested core type"
            );
        }

        let rolled_back = ApplyOutcome::RolledBack {
            revision: revision(7),
            failed_apply: "boom".to_owned(),
        };
        for outcome in [
            rolled_back.clone(),
            ApplyOutcome::DurabilityUncertain {
                outcome: Box::new(rolled_back),
                warning: "the parent directory was not synced".to_owned(),
            },
        ] {
            assert!(!super::echo_commits(&OperationState::Succeeded(
                OperationOutput::Reconciled(outcome)
            )));
        }

        for output in [
            OperationOutput::Stopped,
            OperationOutput::Recovered,
            OperationOutput::ShutDown,
        ] {
            assert!(!super::echo_commits(&OperationState::Succeeded(output)));
        }
        assert!(!super::echo_commits(&OperationState::Queued));
        assert!(!super::echo_commits(&OperationState::Running));
        assert!(!super::echo_commits(&OperationState::Failed(
            ControlError::new(CoreErrorKind::InvalidConfig, "bad", false)
        )));
    }
}

#[cfg(test)]
mod wire_strings {
    /// One of these three ("core is already running") cannot be produced by a
    /// route-level unit test — it requires an actually-running core — so the
    /// producer constants themselves are pinned here, and the two siblings'
    /// route tests prove the bail→envelope pipeline delivers them verbatim.
    #[test]
    fn the_legacy_core_error_strings_are_protocol() {
        assert_eq!(super::MSG_CORE_ALREADY_RUNNING, "core is already running");
        assert_eq!(super::MSG_CORE_ALREADY_STOPPED, "core is already stopped");
        assert_eq!(
            super::MSG_CORE_NOT_STARTED,
            "core have not been started yet"
        );
    }
}
