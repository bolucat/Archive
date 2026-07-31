use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio_util::sync::CancellationToken;

use super::{
    command::Command,
    error::ProcessError,
    event::{ProcessEvent, TerminatedPayload},
    handle::ProcessHandle,
};

type Factory = Arc<dyn Fn() -> Command + Send + Sync>;
type EventHook = Arc<dyn Fn(SupervisorEvent) + Send + Sync>;
type ProcessEventHook = Arc<dyn Fn(ProcessEvent) + Send + Sync>;

/// Controls whether and how often failed children are restarted.
///
/// A child that crashes after its readiness window has reset the attempt budget,
/// matching legacy `recover_core` behavior. A storm guard or sliding-window
/// policy may be added in the future for that failure pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Never,
    OnFailure { max_restarts: u32 },
}

/// Bounds abnormal child exits even when readiness repeatedly resets the
/// consecutive restart attempt count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartStormPolicy {
    max_failures: u32,
    window: Duration,
}

impl RestartStormPolicy {
    pub fn new(max_failures: u32, window: Duration) -> Self {
        Self {
            max_failures: max_failures.max(1),
            window: window.max(Duration::from_millis(1)),
        }
    }
}

impl Default for RestartStormPolicy {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(5 * 60))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
    jitter: bool,
}

fn time_entropy() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    // SplitMix64 finalizer decorrelates same-tick calls and spreads their bits.
    let mut z = nanos
        .wrapping_add(counter.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Backoff {
    pub fn exponential(initial: Duration, max: Duration) -> Self {
        Self {
            initial,
            max,
            jitter: false,
        }
    }

    pub fn with_jitter(mut self) -> Self {
        self.jitter = true;
        self
    }

    pub(crate) fn delay_for(&self, attempt: u32) -> Duration {
        let base = self
            .initial
            .saturating_mul(2u32.saturating_pow(attempt.min(30)))
            .min(self.max);
        if !self.jitter {
            return base;
        }
        // Cheap deterministic-free jitter in [-25%, +25%] without a rand dependency.
        let base_ns = base.as_nanos().max(1) as u64;
        let span = base_ns / 2; // total width 50% => ±25%
        let offset = time_entropy() % span.max(1);
        Duration::from_nanos(base_ns - span / 2 + offset)
    }
}

/// Defines when a running child is considered ready.
///
/// Passing readiness resets the consecutive restart attempt budget, but does
/// not clear the independent restart-storm window.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum ReadinessProbe {
    /// Emit `Ready` if the child is still alive after this delay
    /// (successor of the legacy 1.5s `DelayCheckpointPass`, design §5.3).
    AliveAfter(Duration),
    /// Readiness is acknowledged explicitly through
    /// [`Supervisor::acknowledge_ready`].
    Acknowledged,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    Started { pid: u32 },
    Ready,
    Exited(TerminatedPayload),
    Restarting { attempt: u32, delay: Duration },
    GaveUp,
    Stopped,
}

pub struct SupervisorBuilder {
    factory: Factory,
    policy: RestartPolicy,
    backoff: Backoff,
    readiness: ReadinessProbe,
    storm_policy: RestartStormPolicy,
    on_event: Option<EventHook>,
    on_process_event: Option<ProcessEventHook>,
    cancel_token: Option<CancellationToken>,
}

/// Handle to a running supervision loop.
///
/// Dropping a `Supervisor` cancels supervision asynchronously: the loop kills
/// the current child and exits, but no caller awaits that cleanup. Call
/// [`Supervisor::stop`] for deterministic shutdown.
pub struct Supervisor {
    token: CancellationToken,
    current: Arc<tokio::sync::Mutex<Option<ProcessHandle>>>,
    ready_tx: tokio::sync::mpsc::UnboundedSender<u32>,
    ready_pending: Arc<AtomicU32>,
    task: Option<tokio::task::JoinHandle<()>>,
}

async fn stop_process(handle: ProcessHandle) -> Result<(), ProcessError> {
    if handle.graceful_kill().await.is_err() {
        match handle.kill().await {
            Err(ProcessError::AlreadyExited) => Ok(()),
            result => result,
        }
    } else {
        Ok(())
    }
}

impl Supervisor {
    pub fn builder<F>(factory: F) -> SupervisorBuilder
    where
        F: Fn() -> Command + Send + Sync + 'static,
    {
        SupervisorBuilder {
            factory: Arc::new(factory),
            policy: RestartPolicy::OnFailure { max_restarts: 5 },
            backoff: Backoff::exponential(Duration::from_secs(1), Duration::from_secs(30))
                .with_jitter(),
            readiness: ReadinessProbe::AliveAfter(Duration::from_millis(1500)),
            storm_policy: RestartStormPolicy::default(),
            on_event: None,
            on_process_event: None,
            cancel_token: None,
        }
    }

    /// Marks the current child ready if `pid` still identifies that child.
    /// Stale or already-terminated PIDs are ignored.
    pub async fn acknowledge_ready(&self, pid: u32) -> bool {
        let is_current = self
            .current
            .lock()
            .await
            .as_ref()
            .is_some_and(|handle| handle.pid() == pid && handle.terminated.borrow().is_none());
        if !is_current
            || self
                .ready_pending
                .compare_exchange(pid, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return false;
        }
        if self.ready_tx.send(pid).is_ok() {
            true
        } else {
            self.ready_pending.store(pid, Ordering::SeqCst);
            false
        }
    }

    /// Cancels restarts, gracefully kills the current child, and waits for the
    /// supervision loop to end.
    pub async fn stop(mut self) -> Result<(), ProcessError> {
        self.token.cancel();
        let current = self.current.lock().await.take();
        let stop_result = match current {
            Some(handle) => stop_process(handle).await,
            None => Ok(()),
        };
        self.task
            .take()
            .expect("supervisor task")
            .await
            .map_err(|error| ProcessError::Engine(format!("supervisor task failed: {error}")))?;
        stop_result
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

impl SupervisorBuilder {
    pub fn restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn backoff(mut self, backoff: Backoff) -> Self {
        self.backoff = backoff;
        self
    }

    pub fn readiness(mut self, readiness: ReadinessProbe) -> Self {
        self.readiness = readiness;
        self
    }

    pub fn restart_storm_policy(mut self, policy: RestartStormPolicy) -> Self {
        self.storm_policy = policy;
        self
    }

    /// Registers a supervisor-event hook.
    ///
    /// The hook runs inline on the supervision loop, so it must be cheap and
    /// non-blocking. A slow hook stalls event draining and delays reacting to
    /// cancellation.
    pub fn on_event(mut self, hook: impl Fn(SupervisorEvent) + Send + Sync + 'static) -> Self {
        self.on_event = Some(Arc::new(hook));
        self
    }

    /// Registers a child-process-event hook.
    ///
    /// The hook runs inline on the supervision loop, so it must be cheap and
    /// non-blocking. A slow hook stalls event draining and delays reacting to
    /// cancellation.
    pub fn on_process_event(mut self, hook: impl Fn(ProcessEvent) + Send + Sync + 'static) -> Self {
        self.on_process_event = Some(Arc::new(hook));
        self
    }

    pub fn cancel_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// Starts the supervision loop.
    ///
    /// The first spawn happens before this method returns, so its failure is
    /// returned directly. Each successful spawn emits [`SupervisorEvent::Started`]
    /// and forwards all child [`ProcessEvent`] values to the process hook. A child
    /// passing the configured readiness probe emits [`SupervisorEvent::Ready`]
    /// and resets the consecutive restart attempt. Each exit emits [`SupervisorEvent::Exited`];
    /// exit code zero ends the loop without a restart. Other exits and later spawn
    /// failures consume the restart budget and emit [`SupervisorEvent::Restarting`]
    /// after the configured backoff, or [`SupervisorEvent::GaveUp`] when exhausted.
    /// Cancellation interrupts readiness or backoff, prevents further restarts,
    /// gracefully stops the current child, and ends with [`SupervisorEvent::Stopped`].
    pub async fn spawn(self) -> Result<Supervisor, ProcessError> {
        if self
            .cancel_token
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(ProcessError::Engine(
                "supervisor started with cancelled token".into(),
            ));
        }
        let token = self.cancel_token.unwrap_or_default().child_token();
        let current: Arc<tokio::sync::Mutex<Option<ProcessHandle>>> = Arc::default();
        let (ready_tx, mut ready_rx) = tokio::sync::mpsc::unbounded_channel();
        let ready_pending = Arc::new(AtomicU32::new(0));
        let emit = {
            let hook = self.on_event.clone();
            move |event: SupervisorEvent| {
                if let Some(hook) = &hook {
                    hook(event);
                }
            }
        };

        let (first_handle, first_rx) = (self.factory)().spawn().await?;
        let first_pid = first_handle.pid();
        if matches!(self.readiness, ReadinessProbe::Acknowledged) {
            ready_pending.store(first_pid, Ordering::SeqCst);
        }
        emit(SupervisorEvent::Started { pid: first_pid });
        *current.lock().await = Some(first_handle);

        let factory = self.factory;
        let policy = self.policy;
        let backoff = self.backoff;
        let readiness = self.readiness;
        let storm_policy = self.storm_policy;
        let on_process_event = self.on_process_event;
        let token_ = token.clone();
        let current_ = current.clone();
        let ready_pending_ = ready_pending.clone();

        let task = tokio::spawn(async move {
            let mut attempt = 0;
            let mut next_rx = Some(first_rx);
            let mut abnormal_exits = VecDeque::new();

            loop {
                if let Some(mut rx) = next_rx.take() {
                    let (ready_at, acknowledged) = match readiness {
                        ReadinessProbe::AliveAfter(delay) => {
                            (tokio::time::Instant::now() + delay, false)
                        }
                        ReadinessProbe::Acknowledged => (tokio::time::Instant::now(), true),
                    };
                    let mut readiness_pending = true;
                    let mut cancelled = false;
                    let mut kill_task = None;

                    let payload = loop {
                        tokio::select! {
                            biased;
                            _ = token_.cancelled(), if !cancelled => {
                                cancelled = true;
                                readiness_pending = false;
                                if let Some(handle) = current_.lock().await.take() {
                                    kill_task = Some(tokio::spawn(async move {
                                        let _ = stop_process(handle).await;
                                    }));
                                }
                            }
                            _ = tokio::time::sleep_until(ready_at), if readiness_pending && !acknowledged => {
                                readiness_pending = false;
                                let child_is_alive = current_
                                    .lock()
                                    .await
                                    .as_ref()
                                    .is_some_and(|handle| handle.terminated.borrow().is_none());
                                if child_is_alive && !token_.is_cancelled() {
                                    attempt = 0;
                                    emit(SupervisorEvent::Ready);
                                }
                            }
                            maybe_pid = ready_rx.recv(), if readiness_pending && acknowledged => {
                                let Some(pid) = maybe_pid else {
                                    readiness_pending = false;
                                    continue;
                                };
                                let child_is_current_and_alive = current_
                                    .lock()
                                    .await
                                    .as_ref()
                                    .is_some_and(|handle| {
                                        handle.pid() == pid
                                            && handle.terminated.borrow().is_none()
                                    });
                                if child_is_current_and_alive && !token_.is_cancelled() {
                                    readiness_pending = false;
                                    attempt = 0;
                                    emit(SupervisorEvent::Ready);
                                }
                            }
                            maybe_event = rx.recv() => match maybe_event {
                                Some(event) => {
                                    if let Some(hook) = &on_process_event {
                                        hook(event.clone());
                                    }
                                    if let ProcessEvent::Terminated(payload) = event {
                                        break payload;
                                    }
                                }
                                None => break TerminatedPayload {
                                    code: None,
                                    signal: None,
                                },
                            },
                        }
                    };

                    if let Some(task) = kill_task {
                        let _ = task.await;
                    }
                    current_.lock().await.take();
                    ready_pending_.store(0, Ordering::SeqCst);
                    let clean_exit = payload.code == Some(0);
                    emit(SupervisorEvent::Exited(payload));

                    if cancelled || token_.is_cancelled() {
                        emit(SupervisorEvent::Stopped);
                        return;
                    }
                    if clean_exit || matches!(policy, RestartPolicy::Never) {
                        return;
                    }

                    let now = tokio::time::Instant::now();
                    abnormal_exits.push_back(now);
                    while abnormal_exits
                        .front()
                        .is_some_and(|at| now.duration_since(*at) > storm_policy.window)
                    {
                        abnormal_exits.pop_front();
                    }
                    if abnormal_exits.len() >= storm_policy.max_failures as usize {
                        emit(SupervisorEvent::GaveUp);
                        return;
                    }
                } else if token_.is_cancelled() {
                    emit(SupervisorEvent::Stopped);
                    return;
                }

                attempt += 1;
                let RestartPolicy::OnFailure { max_restarts } = policy else {
                    return;
                };
                if attempt > max_restarts {
                    emit(SupervisorEvent::GaveUp);
                    return;
                }

                let delay = backoff.delay_for(attempt - 1);
                emit(SupervisorEvent::Restarting { attempt, delay });
                tokio::select! {
                    biased;
                    _ = token_.cancelled() => {
                        emit(SupervisorEvent::Stopped);
                        return;
                    }
                    _ = tokio::time::sleep(delay) => {}
                }

                if token_.is_cancelled() {
                    emit(SupervisorEvent::Stopped);
                    return;
                }

                match (factory)().spawn().await {
                    Ok((handle, rx)) => {
                        let pid = handle.pid();
                        *current_.lock().await = Some(handle);
                        next_rx = Some(rx);
                        if !token_.is_cancelled() {
                            if matches!(readiness, ReadinessProbe::Acknowledged) {
                                ready_pending_.store(pid, Ordering::SeqCst);
                            }
                            emit(SupervisorEvent::Started { pid });
                        }
                    }
                    Err(error) => {
                        tracing::error!("supervisor respawn failed: {error}");
                    }
                }
            }
        });

        Ok(Supervisor {
            token,
            current,
            ready_tx,
            ready_pending,
            task: Some(task),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn backoff_doubles_and_caps() {
        let b = Backoff::exponential(Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(b.delay_for(0), Duration::from_secs(1));
        assert_eq!(b.delay_for(1), Duration::from_secs(2));
        assert_eq!(b.delay_for(4), Duration::from_secs(16));
        assert_eq!(b.delay_for(10), Duration::from_secs(30)); // capped

        let long = Backoff::exponential(Duration::from_millis(1), Duration::from_secs(600));
        assert_eq!(long.delay_for(25), Duration::from_secs(600));
    }

    #[test]
    fn jitter_stays_within_25_percent() {
        let b = Backoff::exponential(Duration::from_secs(4), Duration::from_secs(60)).with_jitter();
        let samples: Vec<_> = (0..1000).map(|_| b.delay_for(0)).collect();

        for d in &samples {
            assert!(
                *d >= Duration::from_secs(3) && *d <= Duration::from_secs(5),
                "{d:?}"
            );
        }
        assert!(samples.iter().min().unwrap() < &Duration::from_secs(4));
        assert!(samples.iter().max().unwrap() > &Duration::from_secs(4));
    }
}
