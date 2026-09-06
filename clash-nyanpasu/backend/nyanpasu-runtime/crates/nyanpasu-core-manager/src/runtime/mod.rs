//! The runtime-backend boundary: how the orchestrator launches, observes, and
//! provably stops a core runtime without naming the execution mechanism.
//!
//! Design §12 (first phase): one aggregated backend trait, no separate
//! `CoreDriver` split, and no requirement that the orchestrator itself be
//! backend-agnostic yet — `InstanceSpec` still carries process-shaped fields.
//! The boundary exists so the control plane can be exercised against a fake
//! runtime, and so the process mechanics have one front door.

pub(crate) mod process;

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use tokio::sync::{broadcast, watch};

use crate::{
    epoch::Epoch,
    error::Error,
    log::LogFrame,
    probe::{ProbePhase, ProbeResult},
    spec::{InstanceSpec, ResolvedController},
    state::InstanceStatus,
};

/// Manually boxed futures keep both traits dyn-compatible; native async trait
/// methods are not.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One live core runtime owned by the orchestrator. Every method mirrors the
/// surface the orchestrator actually consumes; nothing here promises a PID or
/// a process — `pid` is `None` for runtimes that have none.
///
/// `epoch`, `spec` and `controller` report the [`RuntimeLaunchRequest`] this
/// runtime was launched from, unchanged. The orchestrator keeps its own copy
/// of that request as the epoch's plan and relies on the two agreeing when it
/// relaunches the same epoch on rollback.
pub trait RuntimeInstance: Send + Sync {
    fn epoch(&self) -> Epoch;

    fn spec(&self) -> &InstanceSpec;

    fn controller(&self) -> &ResolvedController;

    fn pid(&self) -> Option<u32>;

    /// Live status feed; the orchestrator forwards it into the published
    /// `CoreStatus`.
    fn state(&self) -> watch::Receiver<InstanceStatus>;

    /// Resolves when the readiness threshold is reached, or with the launch
    /// failure.
    fn wait_ready<'a>(&'a self) -> BoxFuture<'a, Result<(), Error>>;

    /// One immediate health probe outside the periodic schedule.
    fn probe_now<'a>(&'a self, phase: ProbePhase) -> BoxFuture<'a, ProbeResult>;

    /// Stop and *prove* death within `timeout`. `Err(StopUnconfirmed)` is the
    /// only honest answer when proof is unavailable, and the caller must
    /// quarantine — a timeout is never "probably stopped" (design §7.4).
    fn stop_and_confirm_dead(
        self: Box<Self>,
        timeout: Duration,
    ) -> BoxFuture<'static, Result<(), Error>>;
}

/// Everything a launch needs beyond the backend's own construction-time
/// dependencies. `log_tx` is the manager-owned broadcast channel that outlives
/// every epoch.
pub struct RuntimeLaunchRequest {
    pub effective_spec: InstanceSpec,
    pub epoch: Epoch,
    pub controller: ResolvedController,
    pub log_tx: broadcast::Sender<Arc<LogFrame>>,
}

/// Launches runtimes and validates configs for one execution mechanism.
pub trait RuntimeBackend: Send + Sync {
    fn launch(
        &self,
        request: RuntimeLaunchRequest,
    ) -> BoxFuture<'_, Result<Box<dyn RuntimeInstance>, Error>>;

    /// Dry-run validation of a staged config against the spec's core. Read
    /// only; never touches the active runtime.
    fn check_config<'a>(&'a self, spec: &'a InstanceSpec) -> BoxFuture<'a, Result<(), Error>>;
}
