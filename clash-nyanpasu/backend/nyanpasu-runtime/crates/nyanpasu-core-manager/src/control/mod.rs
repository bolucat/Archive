//! Portable control-plane command surface: operation identity, commands, and
//! operation results.
//!
//! Design: `docs/design/2026-08-08-core-manager-control-plane-runtime-backend-design.md`
//! (§9, §16, §25, with the 2026-08-12 amendments). Two deliberate deviations
//! from that draft, decided at implementation time:
//!
//! - Check is not a [`CoreCommand`] variant. Amendment A2 degrades check to an
//!   advisory, read-only call that never enters the mutating queue, so it is a
//!   separate control method rather than an impossible registry state.
//! - [`CoreError::kind`] is `Option`. The R0 protocol rule is that naming a
//!   kind is a statement of fact about the failure and a guessed kind is worse
//!   than an absent one; unclassified failures stay unclassified.

mod executor;

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

use camino::Utf8PathBuf;
use tokio::sync::{Semaphore, broadcast, mpsc, watch};
use zerocopy::{ByteEq, ByteHash, FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{
    error::CoreErrorKind,
    log::LogFrame,
    manager::{ApplyOutcome, CoreManager},
    spec::{CoreSpec, InstanceOptions, InstanceSpec},
    state::{CoreStatus, RevisionId},
};

use executor::{Admission, ExecutorContext, ExecutorWork, Registry};

/// Correlation, idempotency, and event-tracing identity of one control
/// request. Not a lease, a session, or a lock: it never grants ownership and
/// never blocks another operation
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, ByteEq, ByteHash, Immutable)]
pub struct OperationId {
    pub nanos: u64,
    pub pid: u32,
    pub counter: u32,
}

impl OperationId {
    /// Convenience generator for local callers that do not carry their own id.
    ///
    /// Uniqueness comes from (unix nanos, pid, process-local counter). This is
    /// an identity for the bounded operation registry, not a cryptographic
    /// token; remote callers supply their own ids.
    pub fn generate() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            nanos,
            pid: std::process::id(),
            counter: COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }
}

impl std::fmt::Display for OperationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:016x}-{:08x}-{:08x}",
            self.nanos, self.pid, self.counter
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseOperationIdError;

impl std::fmt::Display for ParseOperationIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an operation id must be lowercase hexadecimal in 16-8-8 format")
    }
}

impl std::error::Error for ParseOperationIdError {}

impl std::str::FromStr for OperationId {
    type Err = ParseOperationIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (nanos, rest) = value.split_once('-').ok_or(ParseOperationIdError)?;
        let (pid, counter) = rest.split_once('-').ok_or(ParseOperationIdError)?;
        if nanos.len() != 16
            || pid.len() != 8
            || counter.len() != 8
            || !value
                .bytes()
                .all(|byte| byte == b'-' || byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ParseOperationIdError);
        }
        Ok(Self {
            nanos: u64::from_str_radix(nanos, 16).map_err(|_| ParseOperationIdError)?,
            pid: u32::from_str_radix(pid, 16).map_err(|_| ParseOperationIdError)?,
            counter: u32::from_str_radix(counter, 16).map_err(|_| ParseOperationIdError)?,
        })
    }
}

/// Change-identity digest over a raw payload: stable FNV-1a, hex-encoded. The
/// idempotency registry compares digests, never full payloads; this is not a
/// cryptographic integrity primitive.
pub fn payload_digest(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Portable configuration payload. The control plane never reads caller
/// filesystem paths; a host that has a path performs read → digest → `Inline`
/// at its own boundary (design §16.2). A `Resource` variant is deferred until
/// a config store consumer exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigInput {
    Inline {
        bytes: Vec<u8>,
        /// [`payload_digest`] of `bytes` as the caller computed it, verified
        /// on receipt when present.
        expected_digest: Option<String>,
    },
}

impl ConfigInput {
    pub fn inline(bytes: Vec<u8>) -> Self {
        Self::Inline {
            bytes,
            expected_digest: None,
        }
    }
}

/// The single mutating convergence command (amendment A2): make the runtime
/// match this desired core + config. Start, restart, apply, and switch are
/// classifications the orchestrator derives internally, not caller choices.
#[derive(Debug, Clone)]
pub struct ReconcileRequest {
    pub core: CoreSpec,
    pub config: ConfigInput,
    pub options: InstanceOptions,
    /// Compare-and-swap token: the revision the caller believes is applied.
    /// `None` skips the comparison (unconditional reconcile).
    pub expected_applied: Option<RevisionId>,
}

/// Advisory, read-only validation (amendment A2): concurrency-limited, never
/// queued, and never a precondition for any change.
#[derive(Debug, Clone)]
pub struct CheckRequest {
    pub core: CoreSpec,
    pub config: ConfigInput,
}

/// A mutating control command. Serialized by the control executor; every
/// variant runs as one transaction to a safe terminal state.
#[derive(Debug, Clone)]
pub enum CoreCommand {
    Reconcile(Box<ReconcileRequest>),
    Stop,
    Recover,
    Shutdown,
}

impl CoreCommand {
    /// The identity digest the idempotency registry compares: two envelopes
    /// with the same [`OperationId`] must describe the same work. Config
    /// bytes, the desired core, the CAS token, and the declared config digest
    /// are identity; tuning fields ([`InstanceOptions`]) deliberately are not.
    ///
    /// A corrected `expected_digest` is therefore a *different* envelope: the
    /// two describe different claims about the same bytes, and re-submitting
    /// under the original id answers `OperationConflict` rather than replaying
    /// the first attempt's `InvalidConfig`. Callers that fix a digest must use
    /// a fresh id.
    pub fn payload_digest(&self) -> String {
        use std::fmt::Write;
        match self {
            Self::Reconcile(request) => {
                let mut identity = format!(
                    "reconcile\0{}\0{}\0{}\0",
                    request.core.kind,
                    request.core.binary_path,
                    request.core.version.as_deref().unwrap_or(""),
                );
                for feature in &request.core.features {
                    identity.push_str(feature);
                    identity.push('\0');
                }
                if let Some(expected) = &request.expected_applied {
                    let _ = write!(identity, "{expected}");
                }
                identity.push('\0');
                let mut payload = identity.into_bytes();
                let ConfigInput::Inline {
                    bytes,
                    expected_digest,
                } = &request.config;
                payload.extend_from_slice(bytes);
                payload.extend_from_slice(b"\0digest:");
                payload
                    .extend_from_slice(expected_digest.as_deref().unwrap_or_default().as_bytes());
                payload_digest(&payload)
            }
            Self::Stop => payload_digest(b"stop"),
            Self::Recover => payload_digest(b"recover"),
            Self::Shutdown => payload_digest(b"shutdown"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CoreCommandEnvelope {
    pub operation_id: OperationId,
    pub command: CoreCommand,
}

/// The successful terminal payload of one operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationOutput {
    Reconciled(ApplyOutcome),
    Stopped,
    Recovered,
    ShutDown,
}

/// Portable, cloneable error surface of the control plane (design §25). The
/// domain [`Error`](crate::Error) is converted at the executor boundary so the
/// registry can replay the same terminal result to idempotent re-submits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    /// See the module doc: `None` means unclassified, never "no error".
    pub kind: Option<CoreErrorKind>,
    /// Human-readable and unstable; never a branch condition.
    pub message: String,
    /// Whether resubmitting the same envelope can plausibly succeed. Set by
    /// the producer: the same kind can be retryable in one situation and not
    /// another (an `OperationConflict` from a handoff in progress is; one from
    /// an id reused with a different payload is not).
    pub retryable: bool,
    pub operation_id: Option<OperationId>,
}

impl CoreError {
    pub fn new(kind: CoreErrorKind, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            kind: Some(kind),
            message: message.into(),
            retryable,
            operation_id: None,
        }
    }

    pub fn from_domain(error: &crate::Error, operation_id: Option<OperationId>) -> Self {
        let kind = error.kind();
        Self {
            kind,
            message: error.to_string(),
            retryable: kind.is_some_and(|kind| kind.default_retryable()),
            operation_id,
        }
    }

    pub fn with_operation(mut self, operation_id: OperationId) -> Self {
        self.operation_id = Some(operation_id);
        self
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            Some(kind) => write!(f, "{kind}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for CoreError {}

/// Registry-visible lifecycle of one operation (design §9.4).
#[derive(Debug, Clone)]
pub enum OperationState {
    Queued,
    Running,
    Succeeded(OperationOutput),
    Failed(CoreError),
}

/// Host-boundary configuration of one control plane.
#[derive(Debug, Clone)]
pub struct ControlOptions {
    /// Where portable config bytes are materialized for the path-based
    /// pipeline underneath. Host-owned; never a caller path.
    pub source_dir: Utf8PathBuf,
    /// Working directory for launched cores (data dir with geo assets).
    pub working_dir: Utf8PathBuf,
    /// Mutating-operation queue bound; a full queue answers `QueueFull`.
    pub queue_capacity: usize,
    /// Most-recent-operations registry bound (idempotency + query window).
    pub registry_capacity: usize,
    /// Concurrent advisory checks; further callers wait on the semaphore.
    pub check_concurrency: usize,
}

impl ControlOptions {
    pub fn new(source_dir: Utf8PathBuf, working_dir: Utf8PathBuf) -> Self {
        Self {
            source_dir,
            working_dir,
            queue_capacity: 16,
            registry_capacity: 64,
            check_concurrency: 2,
        }
    }
}

/// How the executor task ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorExit {
    /// A `Shutdown` operation completed and the executor drained.
    Clean,
    /// The task died without shutting down (panic). The host must treat the
    /// whole control plane as fatally broken.
    Died,
}

/// The portable control plane over one [`CoreManager`] (design §9): submit /
/// status / subscribe on the outside, one executor task owning every mutating
/// transaction on the inside. Cheap to clone; all clones share the executor.
#[derive(Clone)]
pub struct CoreControl {
    manager: CoreManager,
    registry: Arc<Registry>,
    work_tx: mpsc::Sender<ExecutorWork>,
    closing: Arc<AtomicBool>,
    check_semaphore: Arc<Semaphore>,
    source_dir: Utf8PathBuf,
    working_dir: Utf8PathBuf,
    done_rx: watch::Receiver<bool>,
}

impl CoreControl {
    /// Wraps a host-built manager and spawns the executor task. The manager
    /// handle stays usable directly, but every mutating call should go through
    /// `submit` from here on — the executor owns transaction serialization.
    pub fn spawn(manager: CoreManager, options: ControlOptions) -> Self {
        let ControlOptions {
            source_dir,
            working_dir,
            queue_capacity,
            registry_capacity,
            check_concurrency,
        } = options;
        let registry = Arc::new(Registry::new(live_registry_capacity(
            registry_capacity,
            queue_capacity,
        )));
        let (work_tx, work_rx) = mpsc::channel(queue_capacity);
        let (done_tx, done_rx) = watch::channel(false);
        let closing = Arc::new(AtomicBool::new(false));
        let context = Arc::new(ExecutorContext {
            manager: manager.clone(),
            registry: registry.clone(),
            source_dir: source_dir.clone(),
            working_dir: working_dir.clone(),
            closing: closing.clone(),
        });
        tokio::spawn(async move {
            // Only a clean exit reports done. A dead executor drops `done_tx`
            // instead, which is what `until_closed` reads as `Died`.
            if executor::run(work_rx, context).await == ExecutorExit::Clean {
                let _ = done_tx.send(true);
            }
        });
        Self {
            manager,
            registry,
            work_tx,
            closing,
            check_semaphore: Arc::new(Semaphore::new(check_concurrency)),
            source_dir,
            working_dir,
            done_rx,
        }
    }

    /// Admission is synchronous: closing latch, idempotency, then the bounded
    /// queue. The returned handle names the operation; dropping it never
    /// cancels the work.
    pub fn submit(&self, envelope: CoreCommandEnvelope) -> Result<OperationHandle, CoreError> {
        let id = envelope.operation_id;
        if self.closing.load(Ordering::Acquire) {
            return Err(CoreError::new(
                CoreErrorKind::ShuttingDown,
                "the control plane is shutting down",
                false,
            )
            .with_operation(id));
        }
        let digest = envelope.command.payload_digest();
        let is_shutdown = matches!(envelope.command, CoreCommand::Shutdown);
        let work = ExecutorWork {
            id,
            command: envelope.command,
        };
        let (admitted, newly_admitted) = match self
            .registry
            .admit(id, &digest, || self.work_tx.try_send(work))?
        {
            Admission::Existing(admitted) => (admitted, false),
            Admission::Registered(admitted) => {
                // Only after the work is queued: a latch set beside a rejected
                // enqueue would refuse every later submit for a shutdown that
                // never happened.
                if is_shutdown {
                    self.closing.store(true, Ordering::Release);
                }
                (admitted, true)
            }
        };
        Ok(OperationHandle {
            id,
            state_rx: admitted.state_rx,
            newly_admitted,
            sequence: admitted.sequence,
        })
    }

    /// Shuts the control plane down through its own executor, and waits.
    ///
    /// The latch goes down *before* the command is queued — the reverse of
    /// `submit(Shutdown)`, and the whole point: work already waiting in the
    /// queue is refused by the executor instead of running behind the
    /// shutdown. A host that stops the core by any other route can watch its
    /// own shutdown put a core back up.
    ///
    /// Queue pressure waits rather than answering `QueueFull`: everything in
    /// front is about to be refused, so a slot frees promptly.
    pub async fn shutdown(&self) -> Result<(), CoreError> {
        self.closing.store(true, Ordering::Release);
        let permit = match self.work_tx.reserve().await {
            Ok(permit) => permit,
            // The executor already exited; there is nothing left to run a
            // shutdown, so report how it went instead of inventing one.
            Err(_) => {
                return match self.until_closed().await {
                    ExecutorExit::Clean => Ok(()),
                    ExecutorExit::Died => Err(CoreError::new(
                        CoreErrorKind::Internal,
                        "the control executor died",
                        false,
                    )),
                };
            }
        };
        let id = OperationId::generate();
        let command = CoreCommand::Shutdown;
        let digest = command.payload_digest();
        let admitted = match self.registry.admit(id, &digest, move || {
            permit.send(ExecutorWork { id, command });
            Ok(())
        })? {
            Admission::Registered(admitted) | Admission::Existing(admitted) => admitted,
        };
        OperationHandle {
            id,
            state_rx: admitted.state_rx,
            newly_admitted: true,
            sequence: admitted.sequence,
        }
        .wait()
        .await
        .map(drop)
    }

    /// Whether the executor is gone: its receiver was dropped, so nothing can
    /// own a transaction any more. A host uses this to tell a failed shutdown
    /// apart from one that had no executor to run it.
    pub fn executor_is_closed(&self) -> bool {
        self.work_tx.is_closed()
    }

    /// Zero-mailbox snapshot read.
    pub fn status(&self) -> CoreStatus {
        self.manager.status()
    }

    pub fn subscribe(&self) -> watch::Receiver<CoreStatus> {
        self.manager.subscribe()
    }

    pub fn subscribe_logs(&self) -> broadcast::Receiver<Arc<LogFrame>> {
        self.manager.subscribe_logs()
    }

    /// The registry's answer for one operation id, or `None` when the entry
    /// was evicted or never existed. Losing an entry is recoverable by
    /// design: re-read status, and let the revision CAS block a double apply.
    pub fn operation(&self, id: OperationId) -> Option<OperationState> {
        self.registry.get(id)
    }

    /// Long-poll surface for RPC hosts: the operation's state after `timeout`,
    /// or its terminal state the moment it reaches one — whichever comes
    /// first. `None` for an unknown or evicted id.
    pub async fn wait_operation(
        &self,
        id: OperationId,
        timeout: std::time::Duration,
    ) -> Option<OperationState> {
        let mut state_rx = self.registry.subscribe(id)?;
        let _ = tokio::time::timeout(timeout, async {
            while !executor::is_terminal(&state_rx.borrow_and_update()) {
                if state_rx.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
        let state = state_rx.borrow().clone();
        Some(state)
    }

    /// Advisory, read-only config validation (amendment A2): bounded
    /// concurrency, never queued, and never a precondition for any change.
    pub async fn check(&self, request: CheckRequest) -> Result<(), CoreError> {
        let _permit = self.check_semaphore.acquire().await.map_err(|_| {
            CoreError::new(
                CoreErrorKind::Internal,
                "the check semaphore is closed",
                false,
            )
        })?;
        let ConfigInput::Inline { bytes, .. } = &request.config;
        let id = OperationId::generate();
        // The id, not just the digest: two concurrent checks of the same bytes
        // must not share a file, or the first one to finish deletes the source
        // the other is still reading.
        let file_name = format!("check-source-{}-{id}.yaml", payload_digest(bytes));
        let config_path =
            executor::materialize(&self.source_dir, &file_name, request.config, id).await?;
        let spec = InstanceSpec {
            core: request.core,
            config_path: config_path.clone(),
            working_dir: self.working_dir.clone(),
            pid_file: None,
            options: InstanceOptions::default(),
        };
        let result = self.manager.check_config(&spec).await;
        // Best-effort cleanup of this call's own source file.
        let _ = tokio::fs::remove_file(&config_path).await;
        result.map_err(|error| CoreError::from_domain(&error, None))
    }

    /// Resolves when the executor task has exited: cleanly after a `Shutdown`
    /// operation, or [`ExecutorExit::Died`] when it panicked. The host must
    /// watch this and turn `Died` into a fatal service state.
    pub async fn until_closed(&self) -> ExecutorExit {
        let mut done_rx = self.done_rx.clone();
        loop {
            if *done_rx.borrow_and_update() {
                return ExecutorExit::Clean;
            }
            if done_rx.changed().await.is_err() {
                return ExecutorExit::Died;
            }
        }
    }
}

/// The registry bound the executor actually runs with: at most
/// `queue_capacity` operations wait while one runs outside the queue, so a
/// smaller configured bound is raised rather than honored. A misconfigured
/// host should not turn into a run-time `QueueFull`; the invariant "every live
/// operation has a registry entry" holds by construction instead.
fn live_registry_capacity(registry_capacity: usize, queue_capacity: usize) -> usize {
    registry_capacity.max(queue_capacity.saturating_add(1))
}

/// A submitted operation. `wait` consumes the handle and resolves with the
/// terminal result; `state` polls without consuming. Dropping the handle
/// leaves the operation running to its safe terminal state.
#[derive(Debug)]
pub struct OperationHandle {
    id: OperationId,
    state_rx: watch::Receiver<OperationState>,
    newly_admitted: bool,
    sequence: u64,
}

impl OperationHandle {
    pub fn id(&self) -> OperationId {
        self.id
    }

    /// Whether this handle registered the operation, as opposed to attaching
    /// to one that was already submitted under the same id and payload. Hosts
    /// that start per-operation work on admission use it to start it once.
    pub fn newly_admitted(&self) -> bool {
        self.newly_admitted
    }

    /// Where this operation sits in the executor's run order, counted from the
    /// control plane's start.
    ///
    /// It exists for hosts that mirror an operation's effect somewhere else and
    /// have to apply those mirrors in the order the transactions committed:
    /// operations run serially, but whatever watches them does not, so two
    /// completions can be observed out of order and a stale one would overwrite
    /// the current value.
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn state(&self) -> OperationState {
        self.state_rx.borrow().clone()
    }

    pub async fn wait(mut self) -> Result<OperationOutput, CoreError> {
        loop {
            match self.state_rx.borrow_and_update().clone() {
                OperationState::Succeeded(output) => return Ok(output),
                OperationState::Failed(error) => return Err(error),
                OperationState::Queued | OperationState::Running => {}
            }
            if self.state_rx.changed().await.is_err() {
                return Err(CoreError::new(
                    CoreErrorKind::Internal,
                    "the control executor terminated before the operation completed",
                    false,
                )
                .with_operation(self.id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn operation_ids_roundtrip_through_hex() {
        let id = OperationId {
            nanos: 0x0001_0a10_7f80_ff42,
            pid: 0x0001_0a10,
            counter: 0x7f80_ff42,
        };
        let text = id.to_string();
        assert_eq!(text, "00010a107f80ff42-00010a10-7f80ff42");
        assert_eq!(OperationId::from_str(&text).unwrap(), id);
    }

    #[test]
    fn malformed_operation_ids_are_rejected() {
        for bad in [
            "",
            "abc",
            "00010a107f80ff4200010a107f80ff42",
            "00010A107f80ff42-00010a10-7f80ff42",
            "00010g107f80ff42-00010a10-7f80ff42",
            "00010a107f80ff4-00010a10-7f80ff42",
            "00010a107f80ff42-00010a1-7f80ff42",
            "00010a107f80ff42-00010a10-7f80ff4",
            "00010a107f80ff42-00010a10-7f80ff42-extra",
        ] {
            assert_eq!(OperationId::from_str(bad), Err(ParseOperationIdError));
        }
    }

    #[test]
    fn generated_operation_ids_differ() {
        assert_ne!(OperationId::generate(), OperationId::generate());
    }

    #[test]
    fn the_payload_digest_is_stable_and_content_sensitive() {
        assert_eq!(payload_digest(b"abc"), payload_digest(b"abc"));
        assert_ne!(payload_digest(b"abc"), payload_digest(b"abd"));
        // Pinned so a silent algorithm change cannot slip past the idempotency
        // registry's stored digests.
        assert_eq!(payload_digest(b""), "cbf29ce484222325");
    }

    #[test]
    fn domain_errors_convert_with_their_kind_and_default_retryability() {
        let error = CoreError::from_domain(&crate::Error::AlreadyRunning, None);
        assert_eq!(error.kind, Some(CoreErrorKind::AlreadyRunning));
        assert!(!error.retryable);

        let unclassified = CoreError::from_domain(
            &crate::Error::Io(std::io::Error::other("boom")),
            Some(OperationId {
                nanos: 7,
                pid: 7,
                counter: 7,
            }),
        );
        assert_eq!(unclassified.kind, None);
        assert!(!unclassified.retryable);
        assert_eq!(
            unclassified.operation_id,
            Some(OperationId {
                nanos: 7,
                pid: 7,
                counter: 7,
            })
        );
    }

    #[test]
    fn the_registry_bound_is_clamped_to_hold_every_live_operation() {
        // A host that configures fewer registry slots than the queue can hold
        // gets the queue bound plus the running operation, not its own number.
        assert_eq!(live_registry_capacity(1, 2), 3);
        assert_eq!(live_registry_capacity(64, 16), 64);
    }

    #[test]
    fn a_corrected_declared_digest_is_a_different_payload() {
        let request = |expected_digest: Option<&str>| ReconcileRequest {
            core: CoreSpec {
                kind: crate::kind::CoreKind::Mihomo,
                binary_path: Utf8PathBuf::from("core"),
                version: None,
                features: Vec::new(),
            },
            config: ConfigInput::Inline {
                bytes: b"mixed-port: 7890\n".to_vec(),
                expected_digest: expected_digest.map(str::to_owned),
            },
            options: InstanceOptions::default(),
            expected_applied: None,
        };
        let wrong = CoreCommand::Reconcile(Box::new(request(Some("0000000000000000"))));
        let right = CoreCommand::Reconcile(Box::new(request(Some("1111111111111111"))));
        let absent = CoreCommand::Reconcile(Box::new(request(None)));
        assert_ne!(wrong.payload_digest(), right.payload_digest());
        assert_ne!(wrong.payload_digest(), absent.payload_digest());
    }

    #[test]
    fn admission_kinds_default_to_retryable() {
        assert!(CoreError::new(CoreErrorKind::QueueFull, "full", true).retryable);
        assert!(CoreErrorKind::QueueFull.default_retryable());
        assert!(CoreErrorKind::BackendUnavailable.default_retryable());
        assert!(!CoreErrorKind::OperationConflict.default_retryable());
        assert!(!CoreErrorKind::ShuttingDown.default_retryable());
    }
}
