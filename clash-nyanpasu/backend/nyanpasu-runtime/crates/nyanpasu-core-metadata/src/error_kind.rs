//! Stable machine-readable classifications for core-manager failures.
//!
//! Lives here rather than in the core manager because three layers need the
//! same type: the manager that classifies a failure, the IPC envelope that
//! carries it, and the client that interprets it. Every wire spelling is
//! defined here.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Type, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreErrorKind {
    /// The operation needs a running core and there is none.
    NotStarted,
    /// The operation needs a stopped core and one is running.
    AlreadyRunning,
    /// `expected_revision` did not match the running revision. Nothing was
    /// applied; re-read `/status` for the current one and retry.
    RevisionConflict,
    /// An epoch whose death could not be confirmed has latched the manager.
    /// Every lifecycle operation is refused until a `Recover` submission clears it.
    Quarantined,
    /// The core itself rejected the config in a dry run.
    ConfigCheckFailed,
    ConfigNotFound,
    BinaryNotFound,
    /// The config could not be parsed or canonicalized.
    InvalidConfig,
    /// The config declares no external controller, so the core cannot be
    /// health-probed.
    ControllerMissing,
    /// The apply failed and the previous revision was restored.
    ApplyFailed,
    /// The apply failed and so did the rollback: no epoch is running.
    ApplyRollbackFailed,
    /// A core process could not be proven dead; the manager is now quarantined.
    StopUnconfirmed,
    /// The control plane is shutting down and admits no new operations.
    ShuttingDown,
    /// The bounded operation queue is full; retry after in-flight work drains.
    QueueFull,
    /// The `OperationId` was already used with a different payload, or the
    /// operation cannot run concurrently with one that owns the endpoint
    /// (for example a host handoff in progress).
    OperationConflict,
    /// The control endpoint cannot be reached: transport failure, daemon not
    /// running, or the endpoint is reconnecting. Retryable by definition.
    BackendUnavailable,
    /// The control plane itself failed — an executor died or a reply channel
    /// broke. Not retryable; the host must treat this as fatal.
    Internal,
}

impl CoreErrorKind {
    /// Every kind, in wire-declaration order. The single place a new kind has to
    /// be listed besides the enum itself; `from_wire` and the golden test both
    /// walk it.
    pub const ALL: &'static [Self] = &[
        Self::NotStarted,
        Self::AlreadyRunning,
        Self::RevisionConflict,
        Self::Quarantined,
        Self::ConfigCheckFailed,
        Self::ConfigNotFound,
        Self::BinaryNotFound,
        Self::InvalidConfig,
        Self::ControllerMissing,
        Self::ApplyFailed,
        Self::ApplyRollbackFailed,
        Self::StopUnconfirmed,
        Self::ShuttingDown,
        Self::QueueFull,
        Self::OperationConflict,
        Self::BackendUnavailable,
        Self::Internal,
    ];

    /// The wire string. `serde` derives the same spelling from
    /// `rename_all = "snake_case"`; the two are pinned equal by this module's
    /// tests, and this one exists because an envelope needs a `&'static str`.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::AlreadyRunning => "already_running",
            Self::RevisionConflict => "revision_conflict",
            Self::Quarantined => "quarantined",
            Self::ConfigCheckFailed => "config_check_failed",
            Self::ConfigNotFound => "config_not_found",
            Self::BinaryNotFound => "binary_not_found",
            Self::InvalidConfig => "invalid_config",
            Self::ControllerMissing => "controller_missing",
            Self::ApplyFailed => "apply_failed",
            Self::ApplyRollbackFailed => "apply_rollback_failed",
            Self::StopUnconfirmed => "stop_unconfirmed",
            Self::ShuttingDown => "shutting_down",
            Self::QueueFull => "queue_full",
            Self::OperationConflict => "operation_conflict",
            Self::BackendUnavailable => "backend_unavailable",
            Self::Internal => "internal",
        }
    }

    /// Whether a failure of this kind is retryable regardless of what produced
    /// it. Everything else defaults to non-retryable and the producer overrides
    /// where it knows better, which is why this is a default and not the
    /// answer: the retryability that reaches the wire is the producer's.
    pub const fn default_retryable(&self) -> bool {
        matches!(self, Self::QueueFull | Self::BackendUnavailable)
    }

    /// The kind a wire string names, or `None` when this build does not know it.
    ///
    /// `None` is not an error: a newer service may classify a failure this build
    /// has no variant for, and the raw string stays available to the caller.
    pub fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.as_str() == value)
    }
}

impl std::fmt::Display for CoreErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_serializes_to_its_wire_string() {
        for kind in CoreErrorKind::ALL {
            assert_eq!(
                serde_json::to_string(kind).unwrap(),
                format!("\"{}\"", kind.as_str())
            );
        }

        assert_eq!(CoreErrorKind::NotStarted.as_str(), "not_started");
        assert_eq!(CoreErrorKind::AlreadyRunning.as_str(), "already_running");
        assert_eq!(
            CoreErrorKind::RevisionConflict.as_str(),
            "revision_conflict"
        );
        assert_eq!(CoreErrorKind::Quarantined.as_str(), "quarantined");
        assert_eq!(
            CoreErrorKind::ConfigCheckFailed.as_str(),
            "config_check_failed"
        );
        assert_eq!(CoreErrorKind::ConfigNotFound.as_str(), "config_not_found");
        assert_eq!(CoreErrorKind::BinaryNotFound.as_str(), "binary_not_found");
        assert_eq!(CoreErrorKind::InvalidConfig.as_str(), "invalid_config");
        assert_eq!(
            CoreErrorKind::ControllerMissing.as_str(),
            "controller_missing"
        );
        assert_eq!(CoreErrorKind::ApplyFailed.as_str(), "apply_failed");
        assert_eq!(
            CoreErrorKind::ApplyRollbackFailed.as_str(),
            "apply_rollback_failed"
        );
        assert_eq!(CoreErrorKind::StopUnconfirmed.as_str(), "stop_unconfirmed");
        assert_eq!(CoreErrorKind::ShuttingDown.as_str(), "shutting_down");
        assert_eq!(CoreErrorKind::QueueFull.as_str(), "queue_full");
        assert_eq!(
            CoreErrorKind::OperationConflict.as_str(),
            "operation_conflict"
        );
        assert_eq!(
            CoreErrorKind::BackendUnavailable.as_str(),
            "backend_unavailable"
        );
        assert_eq!(CoreErrorKind::Internal.as_str(), "internal");
    }

    #[test]
    fn every_wire_string_parses_back() {
        for kind in CoreErrorKind::ALL {
            assert_eq!(CoreErrorKind::from_wire(kind.as_str()), Some(*kind));
            assert_eq!(
                serde_json::from_str::<CoreErrorKind>(&format!("\"{}\"", kind.as_str())).unwrap(),
                *kind
            );
        }
    }

    #[test]
    fn an_unknown_wire_string_has_no_kind() {
        assert!(CoreErrorKind::from_wire("a_future_kind").is_none());
    }
}
