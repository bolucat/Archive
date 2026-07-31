use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use camino::Utf8PathBuf;
use enumset::{EnumSet, EnumSetType};
pub use nyanpasu_core_metadata::Feature;
use nyanpasu_core_metadata::{CoreVersion, FeatureSupport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    Error,
    spec::{CoreSpec, LocalIpcPolicy},
};

/// Functionality the manager actually enabled for an epoch.
///
/// [`Feature`] answers "does this core build implement a capability"; this set
/// answers "did the manager turn it on". The two disagree exactly where policy
/// overrides ability: a core that supports local IPC runs without it under
/// [`LocalIpcPolicy::Disable`], and a `Prefer` fallback shows up here as a
/// missing [`RuntimeFeature::LocalIpc`] rather than as an inference from
/// policy plus controller host.
#[derive(Debug, EnumSetType, Type, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeFeature {
    /// The epoch's control channel is a manager-owned local IPC endpoint.
    LocalIpc,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct VersionCacheKey {
    binary_path: Utf8PathBuf,
    modified: SystemTime,
}

#[derive(Default)]
pub(crate) struct VersionCache {
    entries: tokio::sync::Mutex<HashMap<VersionCacheKey, String>>,
}

pub(crate) struct ResolvedFeatures {
    /// Capabilities the core build supports, per `nyanpasu-core-metadata`.
    pub capabilities: EnumSet<Feature>,
    /// What the manager enabled after weighing capabilities against policy.
    pub runtime: EnumSet<RuntimeFeature>,
    pub version: Option<String>,
}

impl VersionCache {
    async fn resolve(&self, spec: &CoreSpec) -> Result<String, Error> {
        if let Some(version) = &spec.version {
            return Ok(version.clone());
        }
        let key = VersionCacheKey {
            binary_path: spec.binary_path.clone(),
            modified: binary_modified(spec).await?,
        };

        // Keep the lock across the short probe so concurrent resolutions in
        // one manager issue only one `-v` call for this binary.
        let mut entries = self.entries.lock().await;
        if let Some(version) = entries.get(&key) {
            return Ok(version.clone());
        }
        let version = probe_version(&spec.binary_path).await?;
        if binary_modified(spec).await? != key.modified {
            return Err(Error::CoreVersionProbeFailed {
                binary_path: spec.binary_path.clone(),
                detail:
                    "core binary changed during version probe; retry with the replacement binary"
                        .into(),
            });
        }
        entries.retain(|old, _| old.binary_path != key.binary_path);
        entries.insert(key, version.clone());
        Ok(version)
    }
}

async fn binary_modified(spec: &CoreSpec) -> Result<SystemTime, Error> {
    tokio::fs::metadata(&spec.binary_path)
        .await
        .map_err(|_| Error::BinaryNotFound(spec.binary_path.clone()))?
        .modified()
        .map_err(|error| Error::CoreVersionProbeFailed {
            binary_path: spec.binary_path.clone(),
            detail: error.to_string(),
        })
}

async fn probe_version(binary_path: &camino::Utf8Path) -> Result<String, Error> {
    let output = nyanpasu_utils::process::Command::new(binary_path.as_str())
        .arg("-v")
        .timeout(Duration::from_secs(5))
        .output()
        .await
        .map_err(|error| Error::CoreVersionProbeFailed {
            binary_path: binary_path.to_owned(),
            detail: error.to_string(),
        })?;
    if !output.success() {
        return Err(Error::CoreVersionProbeFailed {
            binary_path: binary_path.to_owned(),
            detail: format!("process exited with code {:?}", output.code),
        });
    }
    let stdout = output.stdout.trim();
    let stderr = output.stderr.trim();
    let version = if !stdout.is_empty() { stdout } else { stderr };
    if version.is_empty() {
        return Err(Error::CoreVersionProbeFailed {
            binary_path: binary_path.to_owned(),
            detail: "version command produced no output".into(),
        });
    }
    Ok(version.to_owned())
}

pub(crate) async fn resolve_features(
    cache: &VersionCache,
    core: &CoreSpec,
    policy: LocalIpcPolicy,
) -> Result<ResolvedFeatures, Error> {
    let (capabilities, version) = if core.kind.potential_features().is_empty() {
        (EnumSet::new(), core.version.clone())
    } else {
        let version = cache.resolve(core).await?;
        (
            core.kind.features(Some(&CoreVersion::parse(&version))),
            Some(version),
        )
    };
    let runtime = resolve_runtime(policy, core, capabilities, version.as_deref())?;
    Ok(ResolvedFeatures {
        capabilities,
        runtime,
        version,
    })
}

/// Decides the runtime set from capabilities and policy, so `Force` fails here
/// — before any config is staged, checked, or spawned — when the core cannot
/// satisfy the platform transport.
fn resolve_runtime(
    policy: LocalIpcPolicy,
    core: &CoreSpec,
    capabilities: EnumSet<Feature>,
    resolved_version: Option<&str>,
) -> Result<EnumSet<RuntimeFeature>, Error> {
    let local_supported = capabilities.contains(crate::config::LOCAL_TRANSPORT_FEATURE);
    match (policy, local_supported) {
        (LocalIpcPolicy::Force, false) => Err(Error::RequiredLocalIpcUnsupported {
            kind: core.kind,
            version: resolved_version
                .or(core.version.as_deref())
                .unwrap_or("unknown")
                .to_owned(),
        }),
        (LocalIpcPolicy::Force | LocalIpcPolicy::Prefer, true) => {
            Ok(EnumSet::only(RuntimeFeature::LocalIpc))
        }
        (LocalIpcPolicy::Prefer, false) | (LocalIpcPolicy::Disable, _) => Ok(EnumSet::new()),
    }
}
