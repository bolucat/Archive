//! The control executor: one task that owns every mutating operation from
//! admission to a safe terminal state, isolated from caller cancellation
//! (design §10). Callers hold [`super::OperationHandle`]s; dropping one never
//! cancels the work it names.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use camino::Utf8PathBuf;
use tokio::sync::{mpsc, watch};

use crate::{
    error::{CoreErrorKind, Error},
    manager::{ApplyOutcome, CoreManager},
    spec::InstanceSpec,
};

use super::{
    ConfigInput, CoreCommand, CoreError, OperationId, OperationOutput, OperationState,
    ReconcileRequest, payload_digest,
};

pub(super) struct ExecutorWork {
    pub(super) id: OperationId,
    pub(super) command: CoreCommand,
}

/// Bounded most-recent-operations registry: the idempotency table and the
/// query surface for lost-response recovery. Losing an entry never affects the
/// runtime — a client that misses falls back to reading `CoreStatus`, and the
/// revision CAS blocks double application (design §9.4).
pub(super) struct Registry {
    inner: parking_lot::Mutex<RegistryInner>,
    capacity: usize,
}

struct RegistryInner {
    operations: HashMap<OperationId, RegisteredOperation>,
    order: VecDeque<OperationId>,
    next_sequence: u64,
}

struct RegisteredOperation {
    digest: String,
    sequence: u64,
    state_tx: watch::Sender<OperationState>,
}

/// One admitted operation's place in the queue, and the channel to watch it on.
pub(super) struct Admitted {
    pub(super) sequence: u64,
    pub(super) state_rx: watch::Receiver<OperationState>,
}

pub(super) enum Admission {
    /// New registration; the caller must enqueue the work.
    Registered(Admitted),
    /// Same id + same digest: the original operation, wherever it is.
    Existing(Admitted),
}

impl Registry {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            inner: parking_lot::Mutex::new(RegistryInner {
                operations: HashMap::new(),
                order: VecDeque::new(),
                next_sequence: 0,
            }),
            capacity,
        }
    }

    /// Registration and enqueue in one critical section.
    ///
    /// `enqueue` runs while the registry lock is held, so a concurrent submit
    /// of the same id either does not see the entry at all or sees one that is
    /// already queued. There is no window in which a caller can attach to a
    /// registered operation that will never reach a terminal state — which is
    /// why `enqueue` must be non-blocking (`try_send` / a reserved permit) and
    /// must never call back into the registry.
    pub(super) fn admit(
        &self,
        id: OperationId,
        digest: &str,
        enqueue: impl FnOnce() -> Result<(), mpsc::error::TrySendError<ExecutorWork>>,
    ) -> Result<Admission, CoreError> {
        let mut inner = self.inner.lock();
        if let Some(existing) = inner.operations.get(&id) {
            if existing.digest == digest {
                return Ok(Admission::Existing(Admitted {
                    sequence: existing.sequence,
                    state_rx: existing.state_tx.subscribe(),
                }));
            }
            return Err(CoreError::new(
                CoreErrorKind::OperationConflict,
                format!("operation {id} was already submitted with a different payload"),
                false,
            )
            .with_operation(id));
        }
        // Evict the oldest *terminal* entries only. In-flight operations are
        // never evicted: `CoreControl::spawn` clamps this capacity to the queue
        // bound plus the one operation running outside the queue, so every live
        // operation fits by construction.
        while inner.operations.len() >= self.capacity {
            let Some(position) = inner.order.iter().position(|id| {
                inner
                    .operations
                    .get(id)
                    .is_some_and(|operation| is_terminal(&operation.state_tx.borrow()))
            }) else {
                break;
            };
            let evicted = inner.order.remove(position).expect("position is in range");
            inner.operations.remove(&evicted);
        }
        let (state_tx, state_rx) = watch::channel(OperationState::Queued);
        // Assigned here, not by the caller: this is the same critical section
        // the enqueue happens in, so the sequence is the order the executor
        // will actually run these in. A caller that stamps its own would be
        // stamping submit order, which two concurrent submits can invert.
        let sequence = inner.next_sequence;
        inner.next_sequence += 1;
        inner.operations.insert(
            id,
            RegisteredOperation {
                digest: digest.to_owned(),
                sequence,
                state_tx,
            },
        );
        inner.order.push_back(id);
        if let Err(error) = enqueue() {
            inner.operations.remove(&id);
            inner.order.pop_back();
            let error = match error {
                mpsc::error::TrySendError::Full(_) => CoreError::new(
                    CoreErrorKind::QueueFull,
                    "the operation queue is full",
                    true,
                ),
                mpsc::error::TrySendError::Closed(_) => CoreError::new(
                    CoreErrorKind::Internal,
                    "the control executor is gone",
                    false,
                ),
            };
            return Err(error.with_operation(id));
        }
        Ok(Admission::Registered(Admitted { sequence, state_rx }))
    }

    pub(super) fn set(&self, id: OperationId, state: OperationState) {
        let inner = self.inner.lock();
        if let Some(operation) = inner.operations.get(&id) {
            // `send` is not `send_replace`: with no live receiver it returns
            // the value back as an error and leaves the stored one untouched.
            // Every caller that drops its handle -- the wire's fire-and-forget
            // submit, for one -- would then leave this operation `Queued`
            // forever, and the next long-poll would subscribe to a state that
            // stopped advancing.
            operation.state_tx.send_replace(state);
        }
    }

    pub(super) fn get(&self, id: OperationId) -> Option<OperationState> {
        let inner = self.inner.lock();
        inner
            .operations
            .get(&id)
            .map(|operation| operation.state_tx.borrow().clone())
    }

    pub(super) fn subscribe(&self, id: OperationId) -> Option<watch::Receiver<OperationState>> {
        let inner = self.inner.lock();
        inner
            .operations
            .get(&id)
            .map(|operation| operation.state_tx.subscribe())
    }
}

pub(super) fn is_terminal(state: &OperationState) -> bool {
    matches!(
        state,
        OperationState::Succeeded(_) | OperationState::Failed(_)
    )
}

pub(super) struct ExecutorContext {
    pub(super) manager: CoreManager,
    pub(super) registry: Arc<Registry>,
    pub(super) source_dir: Utf8PathBuf,
    pub(super) working_dir: Utf8PathBuf,
    /// Shared with [`super::CoreControl`]: the executor is the only writer
    /// after `shutdown` latches it, and the latch is what makes queued work
    /// refuse rather than run behind a shutdown.
    pub(super) closing: Arc<AtomicBool>,
}

pub(super) async fn run(
    mut rx: mpsc::Receiver<ExecutorWork>,
    context: Arc<ExecutorContext>,
) -> super::ExecutorExit {
    while let Some(work) = rx.recv().await {
        let ExecutorWork { id, command } = work;
        let is_shutdown = matches!(command, CoreCommand::Shutdown);
        // Admission latched closing before this operation was dequeued: it was
        // queued for a control plane that no longer accepts work, so it is
        // refused here rather than executed behind the shutdown.
        if !is_shutdown && context.closing.load(Ordering::Acquire) {
            context.registry.set(
                id,
                OperationState::Failed(
                    CoreError::new(
                        CoreErrorKind::ShuttingDown,
                        "the control plane is shutting down",
                        false,
                    )
                    .with_operation(id),
                ),
            );
            continue;
        }
        context.registry.set(id, OperationState::Running);
        // One task per operation, so a panic inside a transaction becomes a
        // `JoinError` here instead of unwinding the executor and leaving every
        // in-flight and queued operation without a terminal state. The executor
        // still awaits it, so operations stay serialized.
        let operation_context = context.clone();
        let outcome =
            tokio::spawn(async move { execute(&operation_context, id, command).await }).await;
        let state = match outcome {
            Ok(Ok(output)) => OperationState::Succeeded(output),
            Ok(Err(error)) => OperationState::Failed(error),
            Err(join) => {
                // The guard over `Ctrl` was released while unwinding, so the
                // orchestrator's state may be half-updated. `Died` is fatal by
                // design: the executor terminalizes what it can and stops.
                context.registry.set(
                    id,
                    OperationState::Failed(
                        CoreError::new(
                            CoreErrorKind::Internal,
                            format!("the control operation panicked: {join}"),
                            false,
                        )
                        .with_operation(id),
                    ),
                );
                context.closing.store(true, Ordering::Release);
                // A queued Shutdown did not happen either: nothing ran it.
                drain(&mut rx, &context, |work| {
                    OperationState::Failed(
                        CoreError::new(CoreErrorKind::Internal, "the control executor died", false)
                            .with_operation(work.id),
                    )
                })
                .await;
                return super::ExecutorExit::Died;
            }
        };
        context.registry.set(id, state);
        if is_shutdown {
            break;
        }
    }
    // Everything still queued was admitted before the shutdown drained the
    // loop. A queued Shutdown *was* accomplished by the one that ran; anything
    // else was not.
    drain(&mut rx, &context, |work| match work.command {
        CoreCommand::Shutdown => OperationState::Succeeded(OperationOutput::ShutDown),
        _ => OperationState::Failed(
            CoreError::new(
                CoreErrorKind::ShuttingDown,
                "the control plane shut down before this queued operation ran",
                false,
            )
            .with_operation(work.id),
        ),
    })
    .await;
    super::ExecutorExit::Clean
}

/// Closes the queue and gives every operation still in it a terminal state.
///
/// `recv` and not `try_recv`: closing the receiver stops new senders but leaves
/// a permit already handed out by [`mpsc::Sender::reserve`] valid, and
/// `try_recv` reports the buffer empty while such a permit is still in flight.
/// Awaiting instead waits for every outstanding permit to be used or dropped,
/// which is what makes "every admitted operation reaches a terminal state" true
/// rather than merely usually true.
async fn drain(
    rx: &mut mpsc::Receiver<ExecutorWork>,
    context: &ExecutorContext,
    terminal: impl Fn(&ExecutorWork) -> OperationState,
) {
    rx.close();
    while let Some(work) = rx.recv().await {
        context.registry.set(work.id, terminal(&work));
    }
}

async fn execute(
    context: &ExecutorContext,
    id: OperationId,
    command: CoreCommand,
) -> Result<OperationOutput, CoreError> {
    let domain = |error: &Error| CoreError::from_domain(error, Some(id));
    match command {
        CoreCommand::Reconcile(request) => reconcile(context, id, *request)
            .await
            .map(OperationOutput::Reconciled),
        CoreCommand::Stop => context
            .manager
            .stop()
            .await
            .map(|()| OperationOutput::Stopped)
            .map_err(|error| domain(&error)),
        CoreCommand::Recover => context
            .manager
            .recover_quarantine()
            .await
            .map(|()| OperationOutput::Recovered)
            .map_err(|error| domain(&error)),
        CoreCommand::Shutdown => context
            .manager
            .shutdown()
            .await
            .map(|()| OperationOutput::ShutDown)
            .map_err(|error| domain(&error)),
    }
}

async fn reconcile(
    context: &ExecutorContext,
    id: OperationId,
    request: ReconcileRequest,
) -> Result<ApplyOutcome, CoreError> {
    let ReconcileRequest {
        core,
        config,
        options,
        expected_applied,
    } = request;
    let config_path = materialize(
        &context.source_dir,
        // One fixed name: mutating operations are serialized, and after the
        // transaction the effective config lives in the runtime store, so the
        // source never accumulates.
        "reconcile-source.yaml",
        config,
        id,
    )
    .await?;
    let spec = InstanceSpec {
        core,
        config_path,
        working_dir: context.working_dir.clone(),
        pid_file: None,
        options,
    };
    context
        .manager
        .reconcile(spec, expected_applied)
        .await
        .map_err(|error| CoreError::from_domain(&error, Some(id)))
}

/// Writes portable config bytes to a host file for the path-based pipeline
/// below the control plane. The digest is verified before anything else
/// happens — a mismatch is a clean abort.
pub(super) async fn materialize(
    source_dir: &camino::Utf8Path,
    file_name: &str,
    config: ConfigInput,
    id: OperationId,
) -> Result<Utf8PathBuf, CoreError> {
    let ConfigInput::Inline {
        bytes,
        expected_digest,
    } = config;
    if let Some(declared) = expected_digest {
        let computed = payload_digest(&bytes);
        if declared != computed {
            return Err(CoreError::new(
                CoreErrorKind::InvalidConfig,
                format!("config digest mismatch: declared {declared}, computed {computed}"),
                false,
            )
            .with_operation(id));
        }
    }
    let io = |error: std::io::Error| CoreError::from_domain(&Error::Io(error), Some(id));
    tokio::fs::create_dir_all(source_dir).await.map_err(io)?;
    let path = source_dir.join(file_name);
    tokio::fs::write(&path, &bytes).await.map_err(io)?;
    Ok(path)
}
