use crate::api::{
    R,
    status::{ConfigRevisionInfo, RevisionIdInfo},
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::PathBuf};

pub const CORE_APPLY_ENDPOINT: &str = "/core/apply";

/// Apply a config to the running core.
///
/// The core must already be running: apply never starts one, so `/core/start`
/// keeps its explicitness (report §7 R2). The manager classifies the change and
/// picks the cheapest route that can carry it — an in-place `PATCH /configs`, a
/// `PUT /configs` reload, a same-epoch restart with rollback, or a full core
/// switch when the process spec itself changed — which is why there is no
/// separate `/core/switch` operation. A `core_type` different from the running
/// one is therefore legal and means "switch to this core".
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreApplyReq<'n> {
    pub core_type: Cow<'n, nyanpasu_utils::core::CoreType>,
    /// The caller's own config file, and only ever the *source*: the service
    /// commits a canonicalized private copy and the core runs that one.
    pub config_file: Cow<'n, PathBuf>,
    /// Compare-and-swap token. `None` applies unconditionally; `Some` applies
    /// nothing and fails with `error_kind = "revision_conflict"` when the
    /// running revision has moved on. Omitted from the wire when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<RevisionIdInfo>,
}

/// How the manager carried the change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ApplyOutcomeKind {
    /// The config was already in effect; the core was not touched.
    Noop,
    /// Applied in place with `PATCH /configs` and verified leaf by leaf. No
    /// downtime.
    Patched,
    /// Applied in place with `PUT /configs`. No downtime.
    Reloaded,
    /// The core process was replaced within the same epoch.
    Restarted,
    /// The process spec itself changed — a different core, binary, or launch
    /// option — so the old epoch was stopped and a new one started. Distinct
    /// from [`Self::Restarted`], which replaces the process inside one epoch.
    ///
    /// This is a hard switch today: the apply path runs a stop → start with
    /// old-epoch rollback (`manager/apply.rs`, `switch_with_compensation`), not
    /// the manager's graceful zero-downtime switch, which only
    /// `restart()`/`switch()` reach.
    Switched,
    /// The apply failed and the previous revision was restored. **The core is
    /// running the OLD config**, `revision` is the old revision, and
    /// `failed_apply` says why the new one was rejected.
    RolledBack,
}

/// The result of an apply.
///
/// `outcome` is the field to branch on. A rolled-back apply is reported as a
/// *successful call* — HTTP 200, `code: "Ok"` — because the caller has to be
/// told which config is actually running; treating it as an error would make it
/// indistinguishable from "nothing happened" (report §4 P0-C).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreApplyData {
    pub outcome: ApplyOutcomeKind,
    /// The revision the core is running now, `source_hash` included: a caller
    /// holding the source file can confirm the service read the bytes it wrote
    /// without reimplementing the hash (report §4 P0-C bullet 1).
    pub revision: ConfigRevisionInfo,
    /// A degradation the operation survived. Today only the manager's
    /// durability warning: the runtime copy is in place but a directory sync
    /// could not be confirmed, so a crash right now might lose it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// Why the desired config was rejected. Set only for
    /// [`ApplyOutcomeKind::RolledBack`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_apply: Option<String>,
}

pub type CoreApplyRes<'a> = R<'a, CoreApplyData>;
