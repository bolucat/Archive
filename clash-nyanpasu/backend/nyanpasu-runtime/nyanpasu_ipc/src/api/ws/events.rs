use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::api::status::{CoreInfos, CoreState};

/// The core log vocabulary, re-exported so a consumer of this stream never has
/// to name the metadata crate to spell the payload it just decoded.
pub use nyanpasu_core_metadata::{
    ClashCoreKind, LogField, LogFrame, LogLevel, LogStream, LogTimestamp,
};

/// The event endpoint. There is no protocol negotiation and no version
/// parameter: the service binary ships with the program that consumes it, so
/// every connection speaks the same stream and any query string is ignored.
pub const EVENT_URI: &str = "/ws/events";

/// The status snapshot is the only large variant, and it is deliberately not
/// boxed: since the log ring started carrying frames rather than events, every
/// `Event` that exists travels on the status ring, where a `CoreStatusChanged`
/// is the common case. Boxing would buy back a few dozen KiB of ring and pay for
/// it with a heap allocation per snapshot, on the path that fans out to every
/// connection.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Event {
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
    /// One core console record, pushed live — the manager's own frame, not a
    /// projection of it. The archive and this stream therefore carry the same
    /// bytes: `raw` is present because under the service's ACL a client cannot
    /// read the JSONL archive, which makes this the only live fidelity source
    /// there is.
    ///
    /// Two clocks, and they mean different things. [`LogFrame::at`] is when the
    /// manager's parser observed the record's root line and is always present,
    /// so it is what a client sorts on; `timestamp` is what the core itself
    /// printed, absent when the header did not parse and `inferred` when the
    /// core printed only a time of day.
    ///
    /// Carried on its own broadcast ring inside the service, because the two
    /// streams fail differently: losing a status frame costs a full snapshot
    /// resend, losing a log line costs nothing. Consequence, stated rather than
    /// hidden: there is **no ordering guarantee between this variant and the
    /// status variants**. They render in different panels and have no causal
    /// relationship.
    ///
    /// Nothing is replayed on connect. The authoritative history is the JSONL
    /// archive the manager writes, whose directory `/status` reports.
    ///
    /// The `Arc` is a service-side fan-out detail and is invisible on the wire:
    /// serde encodes it as the frame itself.
    CoreLog(Arc<LogFrame>),
}

impl Event {
    pub fn new_core_state_changed(state: CoreState) -> Self {
        Self::CoreStateChanged(state)
    }

    pub fn new_core_status_changed(infos: CoreInfos) -> Self {
        Self::CoreStatusChanged(infos)
    }

    pub fn new_core_log(frame: Arc<LogFrame>) -> Self {
        Self::CoreLog(frame)
    }
}
