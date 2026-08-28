//! Single-epoch core instance: process supervision + health-probed state machine.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use nyanpasu_utils::process::{
    Command, EpochPidFile, OrphanReapOutcome, ProcessError, ProcessEvent, ReadinessProbe,
    Supervisor, SupervisorEvent, TerminatedPayload, reap_epoch_pid_file,
};
use tokio::{
    sync::{broadcast, mpsc, oneshot, watch},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error,
    health::{
        HealthTracker, TrackerState,
        driver::{ProbeDriver, ProbeObservation},
    },
    kind::{self, CLICOLOR_FORCE_ENV_NAME, MIHOMO_SAFE_PATHS_ENV_NAME},
    log::{
        LOG_CHANNEL_CAPACITY, LogFrame, LogParser, LogStream, ParsedFrames, error_summary,
        format_tail,
    },
    probe::{ControllerVersionProbe, ProbeHandle, ProbePhase, ProbeResult},
    spec::{InstanceOptions, InstanceSpec, ResolvedController},
    state::{HealthState, HealthStatus, InstanceState, InstanceStatus, StopReason, now_ms},
};

const LOG_TAIL_FRAMES: usize = 32;

/// One epoch of a running core. The spec is immutable; a config change means a
/// new `Instance` with a new epoch (created by `CoreManager`).
pub struct Instance {
    epoch: u64,
    spec: Arc<InstanceSpec>,
    controller: Arc<ResolvedController>,
    state_rx: watch::Receiver<InstanceStatus>,
    shared: Arc<Shared>,
}

struct Shared {
    state_tx: watch::Sender<InstanceStatus>,
    user_stop: AtomicBool,
    probe_timeout: AtomicBool,
    parser: parking_lot::Mutex<LogParser>,
    log_tail: parking_lot::Mutex<VecDeque<Arc<LogFrame>>>,
    log_tx: broadcast::Sender<Arc<LogFrame>>,
    cancel: CancellationToken,
    probe_cancel: CancellationToken,
    probe_request_tx: mpsc::UnboundedSender<ProbeNowRequest>,
    supervisor: tokio::sync::Mutex<Option<Supervisor>>,
    monitor: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Shared {
    fn publish(&self, state: InstanceState) {
        self.state_tx.send_modify(|status| {
            status.state = state.clone();
            if matches!(state, InstanceState::Stopping | InstanceState::Stopped(_)) {
                status.health = None;
            }
        });
    }

    fn publish_status(&self, status: InstanceStatus) {
        let _ = self.state_tx.send(status);
    }

    /// The parser lock is released before logging, buffering and broadcasting:
    /// this runs inline on the supervision loop.
    fn parse(&self, stream: LogStream, line: String) -> ParsedFrames {
        self.parser.lock().push(stream, line)
    }

    fn finish_log_record(&self) -> ParsedFrames {
        self.parser.lock().finish()
    }

    /// Buffer the frame for diagnostics and hand it to the subscribers.
    ///
    /// This runs inline on the supervision loop, in the same `select!` that
    /// decides readiness, so it does exactly two cheap things. It used to emit a
    /// tracing event as well; every consumer that fed — the JSONL archive, and
    /// in the service the bridge that mirrors frames into tracing — is a
    /// broadcast subscriber, so doing it here too was a second copy of every
    /// line paid for on the hot path.
    fn publish_log_frame(&self, frame: LogFrame) {
        // Shared once, here: the tail and every subscriber hold the same
        // allocation, so fan-out costs a refcount rather than a 16 KiB clone.
        let frame = Arc::new(frame);
        let mut tail = self.log_tail.lock();
        if tail.len() == LOG_TAIL_FRAMES {
            tail.pop_front();
        }
        tail.push_back(Arc::clone(&frame));
        drop(tail);
        let _ = self.log_tx.send(frame);
    }

    /// `Terminated` is best-effort, so every terminal path releases the pending
    /// record itself: a fatal block that never saw its terminating event still
    /// reaches the tail and the subscribers.
    fn flush_log_record(&self) {
        for frame in self.finish_log_record().into_iter().flatten() {
            self.publish_log_frame(frame);
        }
    }

    fn diagnostic_frames(&self) -> Vec<Arc<LogFrame>> {
        self.flush_log_record();
        self.log_tail.lock().iter().cloned().collect()
    }

    fn diagnostics(&self) -> String {
        format_tail(&self.diagnostic_frames())
    }

    /// The lifecycle text already carries the raw tail, so it stands in when the
    /// core logged nothing above `Info`.
    fn failure_summary(&self, lifecycle: &str) -> String {
        error_summary(&self.diagnostic_frames()).unwrap_or_else(|| lifecycle.to_owned())
    }
}

struct ProbeNowRequest {
    response: oneshot::Sender<ProbeResult>,
}

pub struct InstanceBuilder {
    spec: InstanceSpec,
    epoch: u64,
    controller: ResolvedController,
    parent: CancellationToken,
    readiness_probe: Option<ProbeHandle>,
    liveness_probe: Option<ProbeHandle>,
    liveness_with_readiness: bool,
    log_tx: Option<broadcast::Sender<Arc<LogFrame>>>,
}

impl Instance {
    pub fn builder(
        spec: InstanceSpec,
        epoch: u64,
        controller: ResolvedController,
        parent: CancellationToken,
    ) -> InstanceBuilder {
        InstanceBuilder {
            spec,
            epoch,
            controller,
            parent,
            readiness_probe: None,
            liveness_probe: None,
            liveness_with_readiness: false,
            log_tx: None,
        }
    }

    pub async fn spawn(
        spec: InstanceSpec,
        epoch: u64,
        controller: ResolvedController,
        parent: CancellationToken,
    ) -> Result<Instance, Error> {
        Self::builder(spec, epoch, controller, parent).spawn().await
    }

    async fn spawn_configured(builder: InstanceBuilder) -> Result<Instance, Error> {
        let InstanceBuilder {
            spec,
            epoch,
            controller,
            parent,
            readiness_probe,
            liveness_probe,
            liveness_with_readiness,
            log_tx,
        } = builder;
        if tokio::fs::metadata(&spec.config_path).await.is_err() {
            return Err(Error::ConfigNotFound(spec.config_path.clone()));
        }
        if tokio::fs::metadata(&spec.core.binary_path).await.is_err() {
            return Err(Error::BinaryNotFound(spec.core.binary_path.clone()));
        }
        kind::run_args(spec.core.kind, &spec.working_dir, &spec.config_path)?;

        let readiness_probe = match readiness_probe {
            Some(probe) => probe,
            None => ProbeHandle::new(
                "controller-version",
                ControllerVersionProbe::new(&controller)?,
            ),
        };
        let liveness_probe = if liveness_with_readiness {
            Some(readiness_probe.clone())
        } else {
            liveness_probe
        };
        let initial_deadline = Instant::now() + spec.options.startup_timeout;
        let spec = Arc::new(spec);
        let controller = Arc::new(controller);
        let (state_tx, state_rx) = watch::channel(InstanceStatus::initial());
        let cancel = parent.child_token();
        let probe_cancel = CancellationToken::new();
        let (probe_request_tx, probe_request_rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            state_tx,
            user_stop: AtomicBool::new(false),
            probe_timeout: AtomicBool::new(false),
            parser: parking_lot::Mutex::new(LogParser::new(spec.core.kind, epoch)),
            log_tail: parking_lot::Mutex::new(VecDeque::with_capacity(LOG_TAIL_FRAMES)),
            log_tx: log_tx.unwrap_or_else(|| broadcast::channel(LOG_CHANNEL_CAPACITY).0),
            cancel: cancel.clone(),
            probe_cancel,
            probe_request_tx,
            supervisor: tokio::sync::Mutex::new(None),
            monitor: tokio::sync::Mutex::new(None),
        });

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let supervisor = Supervisor::builder({
            let spec = spec.clone();
            let controller = controller.clone();
            move || build_command(&spec, epoch, &controller)
        })
        .restart_policy(spec.options.restart_policy)
        .backoff(spec.options.backoff)
        .readiness(ReadinessProbe::Acknowledged)
        .cancel_token(cancel.clone())
        .on_event(move |event| {
            let _ = event_tx.send(event);
        })
        .on_process_event({
            let shared = shared.clone();
            move |event| {
                let frames = match event {
                    ProcessEvent::Stdout(line) => shared.parse(LogStream::Stdout, line),
                    ProcessEvent::Stderr(line) => shared.parse(LogStream::Stderr, line),
                    ProcessEvent::Terminated(_) => shared.finish_log_record(),
                    ProcessEvent::Error(error) => {
                        tracing::warn!(target: "core", "output pump: {error}");
                        [None, None]
                    }
                    _ => [None, None],
                };
                for frame in frames.into_iter().flatten() {
                    shared.publish_log_frame(frame);
                }
            }
        })
        .spawn()
        .await?;
        *shared.supervisor.lock().await = Some(supervisor);

        let monitor = tokio::spawn(monitor_loop(
            event_rx,
            shared.clone(),
            epoch,
            spec.options.clone(),
            controller.clone(),
            readiness_probe,
            liveness_probe,
            initial_deadline,
            probe_request_rx,
        ));
        *shared.monitor.lock().await = Some(monitor);

        Ok(Instance {
            epoch,
            spec,
            controller,
            state_rx,
            shared,
        })
    }

    pub fn state(&self) -> watch::Receiver<InstanceStatus> {
        self.state_rx.clone()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<Arc<LogFrame>> {
        self.shared.log_tx.subscribe()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn spec(&self) -> &InstanceSpec {
        &self.spec
    }

    pub fn controller(&self) -> &ResolvedController {
        &self.controller
    }

    pub fn pid(&self) -> Option<u32> {
        match &self.state_rx.borrow().state {
            InstanceState::Running { pid } => Some(*pid),
            _ => None,
        }
    }

    /// Resolves once the initial start is confirmed (`Running`) or failed
    /// (`Stopped`). The startup timeout is enforced by the monitor task.
    pub async fn wait_ready(&self) -> Result<(), Error> {
        let mut rx = self.state_rx.clone();
        loop {
            let state = rx.borrow_and_update().state.clone();
            match state {
                InstanceState::Running { .. } => return Ok(()),
                InstanceState::Stopped(reason) => {
                    let text = match reason {
                        StopReason::Error(text) => text,
                        other => format!("stopped before ready: {other:?}"),
                    };
                    let stderr_tail = self.shared.failure_summary(&text);
                    return Err(if self.shared.probe_timeout.load(Ordering::SeqCst) {
                        Error::StartupTimeout { stderr_tail }
                    } else {
                        Error::StartupFailed { stderr_tail }
                    });
                }
                _ => {}
            }
            if rx.changed().await.is_err() {
                return Err(Error::StartupFailed {
                    stderr_tail: self.shared.diagnostics(),
                });
            }
        }
    }

    /// Runs one serialized health check using the instance's configured probe.
    pub async fn probe_now(&self, phase: ProbePhase) -> ProbeResult {
        if !matches!(phase, ProbePhase::Reconcile) {
            return ProbeResult::Unhealthy {
                detail: Some("only reconciliation probes can be requested directly".into()),
            };
        }
        let (response, result) = oneshot::channel();
        if self
            .shared
            .probe_request_tx
            .send(ProbeNowRequest { response })
            .is_err()
        {
            return ProbeResult::Unhealthy {
                detail: Some("instance probe monitor is not running".into()),
            };
        }
        result.await.unwrap_or_else(|_| ProbeResult::Unhealthy {
            detail: Some("instance probe driver stopped".into()),
        })
    }

    /// Compatibility wrapper for callers that do not provide a manager stop
    /// deadline.
    pub async fn stop(self) -> Result<(), Error> {
        self.stop_and_confirm_dead(std::time::Duration::from_secs(10))
            .await
    }

    /// Stops supervision and proves the epoch process is dead before returning.
    /// If normal supervisor termination is uncertain, the structured epoch pid
    /// record is the only authority used for the fallback kill.
    pub async fn stop_and_confirm_dead(self, timeout: std::time::Duration) -> Result<(), Error> {
        if self.state_rx.borrow().state.is_terminal() {
            self.reap_epoch_record_if_present().await.map_err(|error| {
                Error::StopUnconfirmed(format!(
                    "terminal instance identity verification failed: {error}"
                ))
            })?;
            return Ok(());
        }
        self.shared.user_stop.store(true, Ordering::SeqCst);
        self.shared.probe_cancel.cancel();
        self.shared.publish(InstanceState::Stopping);
        let supervisor = self.shared.supervisor.lock().await.take();
        let stop_result = match supervisor {
            Some(supervisor) => match tokio::time::timeout(timeout, supervisor.stop()).await {
                Ok(Ok(())) | Ok(Err(ProcessError::AlreadyExited)) => Ok(()),
                Ok(Err(error)) => Err(format!("supervisor stop failed: {error}")),
                Err(_) => Err(format!("supervisor stop exceeded {timeout:?}")),
            },
            None => Ok(()),
        };

        let mut monitor_confirmed = false;
        if let Some(mut monitor) = self.shared.monitor.lock().await.take() {
            match tokio::time::timeout(timeout, &mut monitor).await {
                Ok(_) => monitor_confirmed = true,
                Err(_) => {
                    monitor.abort();
                    let _ = monitor.await;
                }
            }
        }
        let terminal = self.state_rx.borrow().state.is_terminal();
        if stop_result.is_ok() && (monitor_confirmed || terminal) {
            self.reap_epoch_record_if_present().await.map_err(|error| {
                Error::StopUnconfirmed(format!(
                    "stopped instance identity verification failed: {error}"
                ))
            })?;
            return Ok(());
        }
        let stop_error = stop_result
            .err()
            .unwrap_or_else(|| "instance monitor did not confirm termination".to_owned());

        let Some(pid_file) = epoch_pid_path(&self.spec, self.epoch) else {
            return Err(Error::StopUnconfirmed(stop_error));
        };
        let runtime_dir = self.spec.config_path.parent().ok_or_else(|| {
            Error::StopUnconfirmed("runtime config has no parent directory".into())
        })?;
        let reaped = reap_epoch_pid_file(pid_file.as_std_path(), runtime_dir.as_std_path())
            .await
            .map_err(|error| {
                Error::StopUnconfirmed(format!(
                    "{stop_error}; epoch identity reaper failed: {error}"
                ))
            })?;
        if matches!(
            reaped,
            OrphanReapOutcome::AlreadyExited | OrphanReapOutcome::Killed
        ) || (matches!(reaped, OrphanReapOutcome::NotFound) && terminal)
        {
            if !terminal {
                self.shared
                    .publish(InstanceState::Stopped(StopReason::User));
            }
            return Ok(());
        }
        Err(Error::StopUnconfirmed(stop_error))
    }

    async fn reap_epoch_record_if_present(&self) -> Result<(), Error> {
        let Some(pid_file) = epoch_pid_path(&self.spec, self.epoch) else {
            return Ok(());
        };
        if !tokio::fs::try_exists(pid_file).await? {
            return Ok(());
        }
        let runtime_dir = self.spec.config_path.parent().ok_or_else(|| {
            Error::StopUnconfirmed("runtime config has no parent directory".into())
        })?;
        let _ = reap_epoch_pid_file(pid_file.as_std_path(), runtime_dir.as_std_path()).await?;
        Ok(())
    }
}

impl InstanceBuilder {
    pub fn readiness_probe(mut self, probe: ProbeHandle) -> Self {
        self.readiness_probe = Some(probe);
        self
    }

    pub fn liveness_probe(mut self, probe: ProbeHandle) -> Self {
        self.liveness_probe = Some(probe);
        self.liveness_with_readiness = false;
        self
    }

    pub fn liveness_with_readiness_probe(mut self) -> Self {
        self.liveness_probe = None;
        self.liveness_with_readiness = true;
        self
    }

    /// Lets the manager keep one log channel across epochs so subscriptions
    /// survive restarts and core switches.
    pub(crate) fn log_sender(mut self, log_tx: broadcast::Sender<Arc<LogFrame>>) -> Self {
        self.log_tx = Some(log_tx);
        self
    }

    pub async fn spawn(self) -> Result<Instance, Error> {
        Instance::spawn_configured(self).await
    }
}

/// Dropping an `Instance` without `stop()` cancels supervision so the core
/// process tree is killed instead of orphaned (the monitor and supervisor
/// tasks hold `Arc<Shared>`, so drop alone would never reach them).
impl Drop for Instance {
    fn drop(&mut self) {
        self.shared.probe_cancel.cancel();
        self.shared.cancel.cancel();
    }
}

fn build_command(spec: &InstanceSpec, epoch: u64, controller: &ResolvedController) -> Command {
    let mut args = kind::run_args(spec.core.kind, &spec.working_dir, &spec.config_path)
        .expect("kind validated in Instance::spawn");
    args.extend(kind::controller_args(spec.core.kind, &controller.host));
    let config_dir = spec
        .config_path
        .parent()
        .unwrap_or(spec.config_path.as_path());
    let mut command = Command::new(spec.core.binary_path.as_str())
        .args(args)
        .env(
            MIHOMO_SAFE_PATHS_ENV_NAME,
            kind::mihomo_safe_paths(&spec.working_dir, config_dir),
        )
        .env(CLICOLOR_FORCE_ENV_NAME, "0")
        .current_dir(spec.working_dir.as_str());
    if let Some(pid_file) = &spec.pid_file {
        command = if epoch_pid_path(spec, epoch).is_some() {
            command.epoch_pid_file(EpochPidFile::new(
                pid_file.as_std_path(),
                epoch,
                spec.config_path.as_std_path(),
            ))
        } else {
            command.pid_file(pid_file.as_std_path())
        };
    }
    command
}

fn epoch_pid_path(spec: &InstanceSpec, epoch: u64) -> Option<&camino::Utf8Path> {
    let pid_file = spec.pid_file.as_deref()?;
    let expected_pid = format!("core-{epoch}.pid");
    let expected_config = format!("config-{epoch}.yaml");
    (pid_file.file_name() == Some(expected_pid.as_str())
        && spec.config_path.file_name() == Some(expected_config.as_str())
        && pid_file.parent() == spec.config_path.parent())
    .then_some(pid_file)
}

struct RunState {
    run_id: u64,
    pid: u32,
    ack_attempted: bool,
    ready: bool,
    tracker: HealthTracker,
}

#[allow(clippy::too_many_arguments)]
async fn monitor_loop(
    mut events: mpsc::UnboundedReceiver<SupervisorEvent>,
    shared: Arc<Shared>,
    epoch: u64,
    options: InstanceOptions,
    controller: Arc<ResolvedController>,
    readiness_probe: ProbeHandle,
    liveness_probe: Option<ProbeHandle>,
    initial_deadline: Instant,
    mut probe_requests: mpsc::UnboundedReceiver<ProbeNowRequest>,
) {
    let (observation_tx, mut observations) = mpsc::unbounded_channel();
    let mut ever_ready = false;
    let mut timeout_fired = false;
    let mut probes_cancelled = false;
    let mut next_run_id = 0_u64;
    let mut current: Option<RunState> = None;
    let mut driver: Option<ProbeDriver> = None;
    let mut respawn_deadline: Option<Instant> = None;
    let mut last_exit: Option<TerminatedPayload> = None;

    loop {
        let respawn_deadline_for_select = respawn_deadline.unwrap_or(initial_deadline);
        tokio::select! {
            biased;
            _ = shared.probe_cancel.cancelled(), if !probes_cancelled => {
                probes_cancelled = true;
                stop_probe_driver(&mut driver).await;
                current = None;
                respawn_deadline = None;
            }
            maybe = events.recv() => match maybe {
                Some(SupervisorEvent::Started { pid }) => {
                    stop_probe_driver(&mut driver).await;
                    next_run_id = next_run_id.saturating_add(1);
                    let started_at = std::time::Instant::now();
                    let previous_health = shared.state_tx.borrow().health.clone();
                    let lifecycle = shared.state_tx.borrow().state.clone();
                    shared.publish_status(InstanceStatus {
                        state: lifecycle,
                        health: Some(reset_starting_health(previous_health.as_ref())),
                    });
                    current = Some(RunState {
                        run_id: next_run_id,
                        pid,
                        ack_attempted: false,
                        ready: false,
                        tracker: HealthTracker::new(options.health.clone(), started_at),
                    });
                    respawn_deadline = ever_ready
                        .then(|| Instant::now() + options.startup_timeout);
                    if !probes_cancelled {
                        driver = Some(ProbeDriver::start(
                            epoch,
                            next_run_id,
                            pid,
                            controller.clone(),
                            readiness_probe.clone(),
                            liveness_probe.clone(),
                            options.health.clone(),
                            observation_tx.clone(),
                        ));
                    }
                }
                Some(SupervisorEvent::Restarting { attempt, .. }) => {
                    stop_probe_driver(&mut driver).await;
                    current = None;
                    respawn_deadline = None;
                    let previous_health = shared.state_tx.borrow().health.clone();
                    shared.publish_status(InstanceStatus {
                        state: InstanceState::Restarting { attempt },
                        health: Some(reset_starting_health(previous_health.as_ref())),
                    });
                }
                Some(SupervisorEvent::Exited(payload)) => {
                    stop_probe_driver(&mut driver).await;
                    current = None;
                    respawn_deadline = None;
                    last_exit = Some(payload);
                }
                Some(SupervisorEvent::GaveUp) => {
                    stop_probe_driver(&mut driver).await;
                    shared.publish_status(InstanceStatus {
                        state: InstanceState::Stopped(StopReason::Error(format!(
                        "core kept crashing; restart budget exhausted\n{}",
                        shared.diagnostics()
                        ))),
                        health: None,
                    });
                    return;
                }
                Some(SupervisorEvent::Stopped) => {
                    stop_probe_driver(&mut driver).await;
                    publish_terminal(&shared, last_exit.as_ref());
                    return;
                }
                Some(_) => {} // `Ready` (alive-after) only resets the restart budget
                None => {
                    publish_terminal(&shared, last_exit.as_ref());
                    return;
                }
            },
            _ = tokio::time::sleep_until(initial_deadline), if !ever_ready && !timeout_fired => {
                // Total limit for the initial start, crash-retries included.
                let became_ready = drain_probe_observations(
                    &mut observations,
                    &mut current,
                    &mut ever_ready,
                    &mut respawn_deadline,
                    initial_deadline,
                    &shared,
                    driver.as_ref(),
                    epoch,
                ).await;
                if !became_ready && !ever_ready {
                    timeout_fired = true;
                    current = None;
                    respawn_deadline = None;
                    stop_probe_driver(&mut driver).await;
                    shared.probe_timeout.store(true, Ordering::SeqCst);
                    shared.probe_cancel.cancel();
                    shared.cancel.cancel(); // the supervisor kills the tree, then emits Stopped
                }
            }
            _ = tokio::time::sleep_until(respawn_deadline_for_select), if respawn_deadline.is_some() => {
                let became_ready = drain_probe_observations(
                    &mut observations,
                    &mut current,
                    &mut ever_ready,
                    &mut respawn_deadline,
                    initial_deadline,
                    &shared,
                    driver.as_ref(),
                    epoch,
                ).await;
                if !became_ready && respawn_deadline.is_some() {
                    current = None;
                    respawn_deadline = None;
                    stop_probe_driver(&mut driver).await;
                    shared.probe_timeout.store(true, Ordering::SeqCst);
                    shared.probe_cancel.cancel();
                    shared.cancel.cancel();
                }
            }
            request = probe_requests.recv() => match request {
                Some(request) if current.as_ref().is_some_and(|run| run.ready) => {
                    if let Some(driver) = &driver {
                        driver.reconcile(request.response);
                    } else {
                        let _ = request.response.send(ProbeResult::Unhealthy {
                            detail: Some("instance probe driver is not running".into()),
                        });
                    }
                }
                Some(request) => {
                    let _ = request.response.send(ProbeResult::Unhealthy {
                        detail: Some("instance is not running".into()),
                    });
                }
                None => {}
            },
            observation = observations.recv() => {
                let Some(observation) = observation else { continue };
                apply_probe_observation(
                    observation,
                    &mut current,
                    &mut ever_ready,
                    &mut respawn_deadline,
                    initial_deadline,
                    &shared,
                    driver.as_ref(),
                    epoch,
                ).await;
            }
        }
    }
}

fn observation_applies(observation: &ProbeObservation, run: &RunState) -> bool {
    observation.run_id == run.run_id && observation.pid == run.pid
}

#[allow(clippy::too_many_arguments)]
async fn apply_probe_observation(
    observation: ProbeObservation,
    current: &mut Option<RunState>,
    ever_ready: &mut bool,
    respawn_deadline: &mut Option<Instant>,
    initial_deadline: Instant,
    shared: &Shared,
    driver: Option<&ProbeDriver>,
    epoch: u64,
) -> bool {
    let Some(run) = current.as_mut() else {
        return false;
    };
    if !observation_applies(&observation, run) {
        return false;
    }
    let beyond_initial_deadline =
        !*ever_ready && observation.completed_at > initial_deadline.into_std();
    let beyond_respawn_deadline =
        respawn_deadline.is_some_and(|deadline| observation.completed_at > deadline.into_std());
    if beyond_initial_deadline || beyond_respawn_deadline {
        return false;
    }

    tracing::trace!(
        epoch,
        run_id = observation.run_id,
        pid = observation.pid,
        phase = ?observation.phase,
        "applying health probe observation"
    );
    let update = run
        .tracker
        .observe(observation.completed_at, &observation.result);
    let should_ack = !run.ack_attempted && update.state == TrackerState::Healthy;
    if should_ack {
        run.ack_attempted = true;
        let pid = run.pid;
        let supervisor = shared.supervisor.lock().await;
        let acknowledged = match supervisor.as_ref() {
            Some(supervisor) => supervisor.acknowledge_ready(pid).await,
            None => false,
        };
        drop(supervisor);
        if acknowledged {
            *ever_ready = true;
            *respawn_deadline = None;
            if let Some(run) = current.as_mut() {
                run.ready = true;
            }
            if let Some(driver) = driver {
                driver.use_liveness();
            }
            let previous = shared.state_tx.borrow().health.clone();
            shared.publish_status(InstanceStatus {
                state: InstanceState::Running { pid },
                health: Some(health_status(previous.as_ref(), &update, &observation)),
            });
        }
        return acknowledged;
    }

    let previous = shared.state_tx.borrow().health.clone();
    let lifecycle = shared.state_tx.borrow().state.clone();
    shared.publish_status(InstanceStatus {
        state: lifecycle,
        health: Some(health_status(previous.as_ref(), &update, &observation)),
    });
    false
}

#[allow(clippy::too_many_arguments)]
async fn drain_probe_observations(
    observations: &mut mpsc::UnboundedReceiver<ProbeObservation>,
    current: &mut Option<RunState>,
    ever_ready: &mut bool,
    respawn_deadline: &mut Option<Instant>,
    initial_deadline: Instant,
    shared: &Shared,
    driver: Option<&ProbeDriver>,
    epoch: u64,
) -> bool {
    while let Ok(observation) = observations.try_recv() {
        let became_ready = apply_probe_observation(
            observation,
            current,
            ever_ready,
            respawn_deadline,
            initial_deadline,
            shared,
            driver,
            epoch,
        )
        .await;
        if became_ready {
            return true;
        }
    }
    false
}

async fn stop_probe_driver(driver: &mut Option<ProbeDriver>) {
    if let Some(driver) = driver.take() {
        driver.stop().await;
    }
}

fn reset_starting_health(previous: Option<&HealthStatus>) -> HealthStatus {
    HealthStatus {
        state: HealthState::Starting,
        changed_at: previous
            .filter(|status| status.state == HealthState::Starting)
            .map_or_else(now_ms, |status| status.changed_at),
        consecutive_failures: 0,
        last_error: None,
        last_success_at: None,
    }
}

fn health_status(
    previous: Option<&HealthStatus>,
    update: &crate::health::TrackerUpdate,
    observation: &ProbeObservation,
) -> HealthStatus {
    let state = match update.state {
        TrackerState::Starting => HealthState::Starting,
        TrackerState::Healthy => HealthState::Healthy,
        TrackerState::Unhealthy => HealthState::Unhealthy,
    };
    let changed_at = previous
        .filter(|status| status.state == state)
        .map_or(observation.completed_at_ms, |status| status.changed_at);
    let last_success_at = if observation.result.is_healthy() {
        Some(observation.completed_at_ms)
    } else {
        previous.and_then(|status| status.last_success_at)
    };
    HealthStatus {
        state,
        changed_at,
        consecutive_failures: update.consecutive_failures,
        last_error: update.last_error.clone(),
        last_success_at,
    }
}

fn publish_terminal(shared: &Shared, last_exit: Option<&TerminatedPayload>) {
    // The clean-exit and user-stop arms never read the diagnostics, so without
    // this a held fatal record would never reach the log subscribers.
    shared.flush_log_record();
    let reason = if shared.user_stop.load(Ordering::SeqCst) {
        StopReason::User
    } else if shared.probe_timeout.load(Ordering::SeqCst) {
        StopReason::Error(format!("health probe timed out\n{}", shared.diagnostics()))
    } else if last_exit.is_some_and(|payload| payload.code == Some(0)) {
        StopReason::Finished
    } else {
        StopReason::Error(format!(
            "core exited unexpectedly ({last_exit:?})\n{}",
            shared.diagnostics()
        ))
    };
    let _ = shared.state_tx.send(InstanceStatus {
        state: InstanceState::Stopped(reason),
        health: None,
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn run_state(run_id: u64, pid: u32, started_at: std::time::Instant) -> RunState {
        RunState {
            run_id,
            pid,
            ack_attempted: false,
            ready: false,
            tracker: HealthTracker::new(crate::health::HealthPolicy::default(), started_at),
        }
    }

    fn healthy_observation(
        run_id: u64,
        pid: u32,
        completed_at: std::time::Instant,
    ) -> ProbeObservation {
        ProbeObservation {
            run_id,
            pid,
            phase: ProbePhase::Readiness,
            completed_at,
            completed_at_ms: now_ms(),
            result: ProbeResult::Healthy,
        }
    }

    #[test]
    fn observation_requires_matching_run_id_and_pid() {
        let now = std::time::Instant::now();
        let run = run_state(7, 42, now);

        assert!(observation_applies(&healthy_observation(7, 42, now), &run));
        assert!(!observation_applies(&healthy_observation(6, 42, now), &run));
        assert!(!observation_applies(&healthy_observation(7, 41, now), &run));
        assert!(!observation_applies(&healthy_observation(6, 41, now), &run));
    }

    #[test]
    fn terminal_publication_releases_a_held_record() {
        let (state_tx, _state_rx) = watch::channel(InstanceStatus::initial());
        let (probe_request_tx, _probe_request_rx) = mpsc::unbounded_channel();
        let (log_tx, mut logs) = broadcast::channel(LOG_CHANNEL_CAPACITY);
        let shared = Shared {
            state_tx,
            user_stop: AtomicBool::new(true),
            probe_timeout: AtomicBool::new(false),
            parser: parking_lot::Mutex::new(LogParser::new(kind::CoreKind::Mihomo, 1)),
            log_tail: parking_lot::Mutex::new(VecDeque::new()),
            log_tx,
            cancel: CancellationToken::new(),
            probe_cancel: CancellationToken::new(),
            probe_request_tx,
            supervisor: tokio::sync::Mutex::new(None),
            monitor: tokio::sync::Mutex::new(None),
        };
        // A fatal record is held back waiting for continuation lines.
        for frame in shared
            .parse(
                LogStream::Stdout,
                r#"time="2026-07-29T00:17:26+08:00" level=fatal msg="final record""#.to_owned(),
            )
            .into_iter()
            .flatten()
        {
            shared.publish_log_frame(frame);
        }
        assert!(logs.try_recv().is_err());

        // A user stop reads no diagnostics, so the flush has to happen anyway.
        publish_terminal(&shared, None);
        let emitted = logs.try_recv().expect("held record");
        assert_eq!(emitted.message, "final record");
        // One allocation reaches both the tail and the subscriber.
        let buffered = shared
            .log_tail
            .lock()
            .back()
            .cloned()
            .expect("the held record also lands in the diagnostic tail");
        assert!(Arc::ptr_eq(&emitted, &buffered));
        assert!(matches!(
            state_rx_state(&shared),
            InstanceState::Stopped(StopReason::User)
        ));
    }

    fn state_rx_state(shared: &Shared) -> InstanceState {
        shared.state_tx.borrow().state.clone()
    }

    #[test]
    #[ignore = "spawned as the managed child by the deadline-drain test"]
    fn deadline_drain_test_child() {
        std::thread::sleep(Duration::from_secs(60));
    }

    #[tokio::test]
    async fn deadline_drain_applies_queued_boundary_observation_before_timeout() {
        let test_binary = std::env::current_exe().expect("test executable");
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let supervisor = Supervisor::builder(move || {
            Command::new(&test_binary)
                .args([
                    "--exact",
                    "instance::tests::deadline_drain_test_child",
                    "--ignored",
                ])
                .kill_grace(Duration::from_millis(20))
        })
        .readiness(ReadinessProbe::Acknowledged)
        .on_event(move |event| {
            let _ = event_tx.send(event);
        })
        .spawn()
        .await
        .expect("spawn test child");
        let pid = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(SupervisorEvent::Started { pid }) = event_rx.recv().await {
                    break pid;
                }
            }
        })
        .await
        .expect("test child did not start");

        let (state_tx, state_rx) = watch::channel(InstanceStatus::initial());
        let (probe_request_tx, _probe_request_rx) = mpsc::unbounded_channel();
        let shared = Shared {
            state_tx,
            user_stop: AtomicBool::new(false),
            probe_timeout: AtomicBool::new(false),
            parser: parking_lot::Mutex::new(LogParser::new(kind::CoreKind::Mihomo, 1)),
            log_tail: parking_lot::Mutex::new(VecDeque::new()),
            log_tx: broadcast::channel(LOG_CHANNEL_CAPACITY).0,
            cancel: CancellationToken::new(),
            probe_cancel: CancellationToken::new(),
            probe_request_tx,
            supervisor: tokio::sync::Mutex::new(Some(supervisor)),
            monitor: tokio::sync::Mutex::new(None),
        };
        let initial_deadline = Instant::now() + Duration::from_millis(10);
        let deadline = initial_deadline.into_std();
        let mut current = Some(run_state(1, pid, std::time::Instant::now()));
        let mut ever_ready = false;
        let mut respawn_deadline = None;
        let (observation_tx, mut observations) = mpsc::unbounded_channel();
        observation_tx
            .send(healthy_observation(
                1,
                pid,
                deadline + Duration::from_nanos(1),
            ))
            .unwrap();
        observation_tx
            .send(healthy_observation(1, pid, deadline))
            .unwrap();
        tokio::time::sleep_until(initial_deadline + Duration::from_millis(1)).await;

        assert!(
            drain_probe_observations(
                &mut observations,
                &mut current,
                &mut ever_ready,
                &mut respawn_deadline,
                initial_deadline,
                &shared,
                None,
                1,
            )
            .await
        );
        assert!(ever_ready);
        assert!(current.as_ref().is_some_and(|run| run.ready));
        assert!(matches!(
            state_rx.borrow().state,
            InstanceState::Running { pid: running_pid } if running_pid == pid
        ));

        let supervisor = shared
            .supervisor
            .lock()
            .await
            .take()
            .expect("supervisor retained");
        tokio::time::timeout(Duration::from_secs(5), supervisor.stop())
            .await
            .expect("stop test child")
            .expect("supervisor stop");
    }
}
