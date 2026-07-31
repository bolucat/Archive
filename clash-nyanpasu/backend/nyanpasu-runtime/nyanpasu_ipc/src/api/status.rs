use crate::api::R;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::PathBuf};

pub const STATUS_ENDPOINT: &str = "/status";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum CoreState {
    Running,
    Stopped(Option<String>),
}

impl Default for CoreState {
    fn default() -> Self {
        Self::Stopped(None)
    }
}

/// The core's control endpoint, as the manager resolved it.
///
/// A deliberately separate type from `clash_api::Host`: clash-api is an
/// internal dependency of the core manager and must not leak into the wire
/// dependency tree. The controller secret is never carried here — it comes
/// from the caller's own config, and the IPC transport's only gate is the
/// socket ACL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum CoreControllerInfo {
    NamedPipe(PathBuf),
    UnixSocket(PathBuf),
    /// Normalized base URL with any credentials removed.
    Http(String),
}

/// Health observation state for the active core.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum CoreHealthState {
    Starting,
    Healthy,
    Unhealthy,
}

/// The manager's health observation for the active core. Absent while the
/// core is stopping or stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreHealthInfo {
    pub state: CoreHealthState,
    /// Unix milliseconds of the last health-state transition.
    pub changed_at: i64,
    pub consecutive_failures: u32,
    /// Last probe failure detail, capped by the manager at 512 bytes.
    pub last_error: Option<String>,
    /// Unix milliseconds of the last successful probe.
    pub last_success_at: Option<i64>,
}

/// Identity of the config the running core actually adopted.
///
/// `source_hash` is computed over the caller's own file and `effective_hash`
/// over the canonical form the core is running, both FNV-1a over canonical
/// YAML. A caller that holds the source file can therefore confirm the
/// service read the same bytes it wrote, without reimplementing the hash.
/// The manager's private runtime copy path is deliberately not exposed: it
/// lives in a 0o700 directory the caller cannot read, so it would only
/// mislead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ConfigRevisionInfo {
    pub epoch: u64,
    pub generation: u64,
    pub source_hash: String,
    pub effective_hash: String,
}

/// The compare-and-swap identity of a config revision.
///
/// Deliberately a subset of [`ConfigRevisionInfo`]: the manager's CAS compares
/// epoch, generation and `effective_hash` only, so carrying `source_hash` here
/// would imply it takes part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RevisionIdInfo {
    pub epoch: u64,
    pub generation: u64,
    pub effective_hash: String,
}

impl ConfigRevisionInfo {
    /// The CAS token for this revision, for feeding a `/status` snapshot
    /// straight back into `POST /core/apply`'s `expected_revision`.
    ///
    /// Which fields make up the identity is protocol knowledge, not caller
    /// convenience — hence a method here rather than a struct literal at every
    /// call site.
    pub fn id(&self) -> RevisionIdInfo {
        RevisionIdInfo {
            epoch: self.epoch,
            generation: self.generation,
            effective_hash: self.effective_hash.clone(),
        }
    }
}

/// The core's full lifecycle state.
///
/// [`CoreState`] is a two-valued projection kept for wire compatibility: it
/// reports `Starting` and `Restarting` as `Stopped(None)`, so a crash loop is
/// indistinguishable from a real stop. This is the faithful view; prefer it
/// when present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum CoreStateDetail {
    Stopped { reason: Option<String> },
    Starting { epoch: u64 },
    Running { epoch: u64, pid: u32 },
    Restarting { epoch: u64, attempt: u32 },
    Switching { from: Option<u64>, to: u64 },
    Stopping { epoch: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreInfos {
    pub r#type: Option<nyanpasu_utils::core::CoreType>,
    pub state: CoreState,
    pub state_changed_at: i64,
    pub config_path: Option<PathBuf>,
    /// Omitted rather than null when absent, so a payload carrying none of
    /// the fields below is byte-identical to the pre-S7 wire format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<CoreControllerInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<CoreHealthInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<ConfigRevisionInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<CoreStateDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RuntimeInfos<'a> {
    pub service_data_dir: Cow<'a, PathBuf>,
    pub service_config_dir: Cow<'a, PathBuf>,
    pub nyanpasu_config_dir: Cow<'a, PathBuf>,
    pub nyanpasu_data_dir: Cow<'a, PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct StatusResBody<'a> {
    pub version: Cow<'a, str>,
    pub core_infos: CoreInfos,
    pub runtime_infos: RuntimeInfos<'a>,
}

pub type StatusRes<'a> = R<'a, StatusResBody<'a>>;
