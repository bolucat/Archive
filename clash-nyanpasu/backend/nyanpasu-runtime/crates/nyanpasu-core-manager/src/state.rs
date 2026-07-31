//! Instance and manager state machines and the published status snapshot.

use camino::Utf8PathBuf;

use crate::{Feature, RuntimeFeature, kind::CoreKind};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Starting,
    Healthy,
    Unhealthy,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HealthStatus {
    pub state: HealthState,
    /// Unix milliseconds of the last health-state transition.
    pub changed_at: i64,
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    pub last_success_at: Option<i64>,
}

impl std::fmt::Debug for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthStatus")
            .field("state", &self.state)
            .field("changed_at", &self.changed_at)
            .field("consecutive_failures", &self.consecutive_failures)
            .field(
                "last_error",
                &self.last_error.as_ref().map(|_| "<redacted>"),
            )
            .field("last_success_at", &self.last_success_at)
            .finish()
    }
}

impl HealthStatus {
    pub(crate) fn starting() -> Self {
        Self {
            state: HealthState::Starting,
            changed_at: now_ms(),
            consecutive_failures: 0,
            last_error: None,
            last_success_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceState {
    /// Spawned; the readiness success threshold has not been reached yet.
    Starting,
    Running {
        pid: u32,
    },
    /// Crashed; the supervisor is backing off, respawning, or re-probing.
    Restarting {
        attempt: u32,
    },
    Stopping,
    Stopped(StopReason),
}

impl InstanceState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceStatus {
    pub state: InstanceState,
    pub health: Option<HealthStatus>,
}

impl InstanceStatus {
    pub(crate) fn initial() -> Self {
        Self {
            state: InstanceState::Starting,
            health: Some(HealthStatus::starting()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The process exited cleanly (code 0); the supervisor does not restart it.
    Finished,
    User,
    Error(String),
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopReason::Finished => f.write_str("core exited"),
            StopReason::User => f.write_str("stopped by user"),
            StopReason::Error(message) => f.write_str(message),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreState {
    Stopped {
        reason: Option<StopReason>,
    },
    Starting {
        epoch: u64,
    },
    Running {
        epoch: u64,
        pid: u32,
    },
    Restarting {
        epoch: u64,
        attempt: u32,
    },
    /// A hard or graceful switch is in flight.
    Switching {
        from: Option<u64>,
        to: u64,
    },
    Stopping {
        epoch: u64,
    },
}

#[derive(Debug, Clone)]
pub struct SpecSummary {
    pub kind: CoreKind,
    pub config_path: Utf8PathBuf,
    /// Capabilities the active core build supports, resolved from its version.
    pub capabilities: Vec<Feature>,
    /// Functionality the manager actually enabled for the active epoch.
    pub runtime_features: Vec<RuntimeFeature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRevision {
    pub epoch: u64,
    pub generation: u64,
    pub source_hash: String,
    pub effective_hash: String,
    pub runtime_path: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionId {
    pub epoch: u64,
    pub generation: u64,
    pub effective_hash: String,
}

impl ConfigRevision {
    pub fn id(&self) -> RevisionId {
        RevisionId {
            epoch: self.epoch,
            generation: self.generation,
            effective_hash: self.effective_hash.clone(),
        }
    }
}

impl std::fmt::Display for RevisionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "epoch {} generation {} ({})",
            self.epoch, self.generation, self.effective_hash
        )
    }
}

/// Snapshot published on the manager's watch channel.
#[derive(Debug, Clone)]
pub struct CoreStatus {
    pub state: CoreState,
    /// Unix milliseconds of the last state transition (feeds IPC `state_changed_at`).
    pub changed_at: i64,
    pub health: Option<HealthStatus>,
    pub spec: Option<SpecSummary>,
    /// The primary controller channel used by the current active instance.
    pub controller: Option<clash_api::Host>,
    pub revision: Option<ConfigRevision>,
}

impl CoreStatus {
    pub(crate) fn initial() -> Self {
        Self {
            state: CoreState::Stopped { reason: None },
            changed_at: now_ms(),
            health: None,
            spec: None,
            controller: None,
            revision: None,
        }
    }
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
