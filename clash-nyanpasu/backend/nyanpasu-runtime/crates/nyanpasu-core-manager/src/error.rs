use camino::Utf8PathBuf;
pub use nyanpasu_core_metadata::CoreErrorKind;

use crate::{kind::CoreKind, state::RevisionId};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("core is already running")]
    AlreadyRunning,
    #[error("core is not running")]
    NotStarted,
    #[error("config file not found: {0}")]
    ConfigNotFound(Utf8PathBuf),
    #[error("core binary not found: {0}")]
    BinaryNotFound(Utf8PathBuf),
    #[error("failed to probe version from core binary `{binary_path}`: {detail}")]
    CoreVersionProbeFailed {
        binary_path: Utf8PathBuf,
        detail: String,
    },
    #[error("no external controller configured; the version health probe needs one")]
    ControllerMissing,
    #[error(
        "core `{kind}` version `{version}` does not support the required local IPC transport; use LocalIpcPolicy::Prefer or LocalIpcPolicy::Disable"
    )]
    RequiredLocalIpcUnsupported { kind: CoreKind, version: String },
    #[error("config check failed: {0}")]
    ConfigCheckFailed(String),
    #[error("invalid runtime config: {0}")]
    InvalidConfig(String),
    #[error("invalid manager options: {0}")]
    InvalidManagerOptions(String),
    #[error("invalid health policy: {0}")]
    InvalidHealthPolicy(String),
    #[error("unsafe runtime artifact: {0}")]
    UnsafeRuntimeArtifact(Utf8PathBuf),
    #[error("runtime directory is already owned by another manager: {0}")]
    RuntimeDirectoryOwned(Utf8PathBuf),
    #[error("core process death could not be confirmed: {0}")]
    StopUnconfirmed(String),
    #[error("manager is quarantined by uncertain epoch {epoch}: {reason}")]
    ManagerQuarantined { epoch: u64, reason: String },
    #[error("config revision conflict: expected {expected}, actual {actual:?}")]
    RevisionConflict {
        expected: RevisionId,
        actual: Option<RevisionId>,
    },
    #[error("config apply failed: {0}")]
    ApplyFailed(String),
    #[error("config apply failed ({apply}); rollback also failed ({rollback})")]
    ApplyRollbackFailed { apply: String, rollback: String },
    #[error("{source}; runtime durability warning: {warning}")]
    DurabilityUncertain {
        #[source]
        source: Box<Error>,
        warning: String,
    },
    #[error(
        "core did not become healthy before the startup timeout; diagnostic log tail:\n{stderr_tail}"
    )]
    StartupTimeout { stderr_tail: String },
    #[error("core failed to start; diagnostic log tail:\n{stderr_tail}")]
    StartupFailed { stderr_tail: String },
    #[error(transparent)]
    Process(#[from] nyanpasu_utils::process::ProcessError),
    #[error(transparent)]
    Api(#[from] clash_api::Error),
    #[error("failed to process config YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// The machine-readable classification a caller can branch on, or `None`
    /// when this failure has none.
    ///
    /// `None` means "not classified", never "no error": naming a kind is a
    /// statement of fact about the failure, and a guessed one is worse than an
    /// absent one. The wire omits the field entirely for `None`.
    ///
    /// The match is exhaustive on purpose. `Error` is `#[non_exhaustive]` only
    /// to *downstream* crates, so this — the defining crate — is the one place
    /// a new variant can be made to fail the build instead of silently
    /// answering `None`. Do not collapse the unclassified arms into a wildcard.
    pub fn kind(&self) -> Option<CoreErrorKind> {
        match self {
            Self::AlreadyRunning => Some(CoreErrorKind::AlreadyRunning),
            Self::NotStarted => Some(CoreErrorKind::NotStarted),
            Self::ConfigNotFound(_) => Some(CoreErrorKind::ConfigNotFound),
            Self::BinaryNotFound(_) => Some(CoreErrorKind::BinaryNotFound),
            Self::ControllerMissing => Some(CoreErrorKind::ControllerMissing),
            Self::ConfigCheckFailed(_) => Some(CoreErrorKind::ConfigCheckFailed),
            Self::InvalidConfig(_) | Self::Yaml(_) => Some(CoreErrorKind::InvalidConfig),
            Self::StopUnconfirmed(_) => Some(CoreErrorKind::StopUnconfirmed),
            Self::ManagerQuarantined { .. } => Some(CoreErrorKind::Quarantined),
            Self::RevisionConflict { .. } => Some(CoreErrorKind::RevisionConflict),
            Self::ApplyFailed(_) => Some(CoreErrorKind::ApplyFailed),
            Self::ApplyRollbackFailed { .. } => Some(CoreErrorKind::ApplyRollbackFailed),
            // The durability wrapper is a warning around a real failure; report
            // the failure's kind so a caller can still branch on it.
            Self::DurabilityUncertain { source, .. } => source.kind(),
            // Unclassified, listed one by one so a new variant is a compile
            // error here. Mapping these is the protocol report's P3.
            Self::CoreVersionProbeFailed { .. }
            | Self::RequiredLocalIpcUnsupported { .. }
            | Self::InvalidManagerOptions(_)
            | Self::InvalidHealthPolicy(_)
            | Self::UnsafeRuntimeArtifact(_)
            | Self::RuntimeDirectoryOwned(_)
            | Self::StartupTimeout { .. }
            | Self::StartupFailed { .. }
            | Self::Process(_)
            | Self::Api(_)
            | Self::Io(_) => None,
        }
    }
}

impl From<nyanpasu_utils::io::atomic_fs::AtomicFsError> for Error {
    fn from(error: nyanpasu_utils::io::atomic_fs::AtomicFsError) -> Self {
        use nyanpasu_utils::io::atomic_fs::AtomicFsError;
        match error {
            AtomicFsError::UnsafePath(path) => Error::UnsafeRuntimeArtifact(utf8_lossy(path)),
            AtomicFsError::Contended(path) => Error::RuntimeDirectoryOwned(utf8_lossy(path)),
            AtomicFsError::Io(io_error) => Error::Io(io_error),
        }
    }
}

fn utf8_lossy(path: std::path::PathBuf) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path)
        .unwrap_or_else(|path| Utf8PathBuf::from(path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manager_errors_carry_their_wire_kind() {
        let cases: [(Error, Option<CoreErrorKind>); 8] = [
            (Error::NotStarted, Some(CoreErrorKind::NotStarted)),
            (Error::AlreadyRunning, Some(CoreErrorKind::AlreadyRunning)),
            (
                Error::RevisionConflict {
                    expected: RevisionId {
                        epoch: 3,
                        generation: 7,
                        effective_hash: "fedcba9876543210".to_owned(),
                    },
                    actual: None,
                },
                Some(CoreErrorKind::RevisionConflict),
            ),
            (
                Error::ManagerQuarantined {
                    epoch: 3,
                    reason: "death unconfirmed".to_owned(),
                },
                Some(CoreErrorKind::Quarantined),
            ),
            (
                Error::ConfigCheckFailed("unknown field".to_owned()),
                Some(CoreErrorKind::ConfigCheckFailed),
            ),
            (
                Error::ControllerMissing,
                Some(CoreErrorKind::ControllerMissing),
            ),
            (
                Error::DurabilityUncertain {
                    source: Box::new(Error::ApplyFailed("boom".to_owned())),
                    warning: "sync failed".to_owned(),
                },
                Some(CoreErrorKind::ApplyFailed),
            ),
            (
                Error::StartupFailed {
                    stderr_tail: String::new(),
                },
                None,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error.kind(), expected, "{error}");
        }
    }

    #[test]
    fn the_durability_wrapper_reports_its_source_kind() {
        let error = Error::DurabilityUncertain {
            source: Box::new(Error::ApplyFailed("boom".to_owned())),
            warning: "sync failed".to_owned(),
        };
        assert_eq!(error.kind(), Some(CoreErrorKind::ApplyFailed));
    }

    #[test]
    fn a_nested_durability_wrapper_still_reaches_the_source() {
        let error = Error::DurabilityUncertain {
            source: Box::new(Error::DurabilityUncertain {
                source: Box::new(Error::RevisionConflict {
                    expected: RevisionId {
                        epoch: 3,
                        generation: 7,
                        effective_hash: "fedcba9876543210".to_owned(),
                    },
                    actual: None,
                }),
                warning: "inner sync failed".to_owned(),
            }),
            warning: "outer sync failed".to_owned(),
        };
        assert_eq!(error.kind(), Some(CoreErrorKind::RevisionConflict));
    }

    #[test]
    fn an_unclassified_failure_has_no_kind() {
        assert!(Error::Io(std::io::Error::other("boom")).kind().is_none());
        assert!(
            Error::StartupTimeout {
                stderr_tail: String::new(),
            }
            .kind()
            .is_none()
        );
    }
}
