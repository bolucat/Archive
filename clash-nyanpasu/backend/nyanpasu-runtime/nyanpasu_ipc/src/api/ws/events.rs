use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::api::status::{CoreInfos, CoreState};

/// The event endpoint. There is no protocol negotiation and no version
/// parameter: the service binary ships with the program that consumes it, so
/// every connection speaks the same stream and any query string is ignored.
pub const EVENT_URI: &str = "/ws/events";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct TraceLog {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub target: String,
    pub fields: IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Event {
    Log(TraceLog),
    /// The lossy state, kept exactly as it has always been: `Starting` and
    /// `Restarting` are reported as `Stopped(None)`, so a crash loop is
    /// indistinguishable from a stop. Kept because the GUI still consumes it,
    /// and emitted beside every [`Self::CoreStatusChanged`];
    /// [`Self::CoreStatusChanged`] is where the faithful view lives.
    CoreStateChanged(CoreState),
    /// The full status snapshot — the same [`CoreInfos`] `/status` returns,
    /// including the faithful `detail`. Sent to every connection: once when the
    /// socket opens, once after a dropped-event recovery, and on every manager
    /// transition. Push *is* snapshot: the payload is byte-identical to
    /// `/status`'s `core_infos`, so a client feeds it into the same state it
    /// already keeps for `/status`. Treat it as idempotent — a reconnect or a
    /// lag recovery can repeat one.
    CoreStatusChanged(CoreInfos),
}

impl Event {
    pub fn new_log(log: TraceLog) -> Self {
        Self::Log(log)
    }

    pub fn new_core_state_changed(state: CoreState) -> Self {
        Self::CoreStateChanged(state)
    }

    pub fn new_core_status_changed(infos: CoreInfos) -> Self {
        Self::CoreStatusChanged(infos)
    }
}
