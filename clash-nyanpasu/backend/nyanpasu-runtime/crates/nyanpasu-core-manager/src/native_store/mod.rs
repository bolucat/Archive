//! Host-injected ownership of native core data, separate from epoch artifacts.
mod fs;
mod paths;
pub use fs::{FsNativeStore, StoreOwner};
pub use paths::rewrite_paths;

use crate::CoreKind;
use camino::{Utf8Path, Utf8PathBuf};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("native store is inaccessible: {0}")]
    Access(#[from] std::io::Error),
    #[error("native store is in use: {0}")]
    Busy(Utf8PathBuf),
    #[error("unsafe native store path: {0}")]
    Unsafe(Utf8PathBuf),
    #[error("native store format or migration is ambiguous: {0}")]
    Format(Utf8PathBuf),
    #[error("native store owner is not configured; update or reinstall the service")]
    OwnerMissing,
    #[error("native store requires recovery of its previous process: {0}")]
    Recovery(Utf8PathBuf),
}

/// Filesystem/privilege boundary. Hosts supply the trusted data root and owner;
/// neither is accepted from a core-control wire request.
pub trait NativeStore: Send + Sync + 'static {
    fn data_dir(&self) -> &Utf8Path;
    fn home(&self, kind: CoreKind) -> Utf8PathBuf {
        self.data_dir().join("native-store/v1").join(kind.as_ref())
    }
    fn acquire(&self, kind: CoreKind) -> Result<Arc<dyn StoreLease>, StoreError>;
    fn check_home(&self) -> Result<tempfile::TempDir, StoreError>;
}

/// Held by the instance and its supervisor until death is confirmed. Releasing
/// an OS lock alone does not prove death after a manager crash: the dirty marker
/// is deliberately retained unless the owner calls `confirm_stopped`.
pub trait StoreLease: Send + Sync + 'static {
    fn prepare_spawn(&self) -> Result<(), StoreError>;
    fn record_started(&self, pid_file: &Utf8Path) -> Result<(), StoreError>;
    fn confirm_stopped(&self) -> Result<(), StoreError>;
}

pub(crate) fn cache_failure(kind: CoreKind, message: &str) -> bool {
    match kind {
        CoreKind::ClashRust => [
            "failed to read cache file:",
            "failed to parse cache file:",
            "failed to write cache file:",
            "failed to serialize cache file:",
        ]
        .iter()
        .any(|prefix| message.contains(prefix)),
        CoreKind::Meow => {
            message.contains("selector store:")
                && (message.contains("failed") || message.contains("malformed"))
        }
        _ => {
            message.contains("[CacheFile]")
                && ["can't open", "failed:", "error:"]
                    .iter()
                    .any(|part| message.contains(part))
        }
    }
}

/// Capture the persisted selection before a control request can change it.
/// Unknown or absent identity cannot authorize importing a bbolt database.
pub fn legacy_kind(config: &std::path::Path) -> Result<Option<CoreKind>, StoreError> {
    let bytes = match std::fs::read(config) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let document: serde_yaml_ng::Mapping = serde_yaml_ng::from_slice(&bytes)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(
        match document
            .get("core")
            .or_else(|| document.get("clash_core"))
            .and_then(serde_yaml_ng::Value::as_str)
        {
            Some("mihomo" | "mihomo-alpha" | "clash-meta") => Some(CoreKind::Mihomo),
            Some("clash-rs" | "clash-rs-alpha") => Some(CoreKind::ClashRust),
            Some("clash" | "clash-premium") => Some(CoreKind::ClashPremium),
            Some("meow") => Some(CoreKind::Meow),
            _ => None,
        },
    )
}
