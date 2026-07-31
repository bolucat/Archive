use camino::Utf8PathBuf;

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
