//! The canonical shape of one normalized core console record.
//!
//! Lives here rather than in the core manager because three layers need the
//! same type: the parser that produces it, the JSONL archive that flattens it
//! onto disk, and the IPC event that pushes it to a client. Every representation
//! that used to project this shape has been retired.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ClashCoreKind;

/// Normalized severity. Go's `fatal` and `panic` both terminate the process, so
/// they collapse into `Fatal`; the original spelling survives in
/// [`LogFrame::raw`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Type, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

/// Which console stream a record arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Type, Serialize, Deserialize)]
pub struct LogTimestamp {
    /// Exactly as the core printed it.
    pub raw: String,
    /// Unix milliseconds, when the printed instant resolves to one.
    pub unix_ms: Option<i64>,
    /// The date or the zone offset came from the observation instant because the
    /// core did not print one (clash premium and clash-rs).
    pub inferred: bool,
}

/// One structured field the core printed beside its message.
#[derive(Debug, Clone, PartialEq, Eq, Type, Serialize, Deserialize)]
pub struct LogField {
    pub key: String,
    pub value: String,
}

/// One normalized core console record.
///
/// Bounded on every free-text field — but that is a *provenance* invariant, held
/// by the parser that builds these, not a property of the type. Nothing stops a
/// decoded or hand-built frame from carrying an arbitrarily long message.
///
/// The field order is the on-disk JSONL order: the archive flattens this struct
/// straight into its envelope.
#[derive(Debug, Clone, PartialEq, Eq, Type, Serialize, Deserialize)]
pub struct LogFrame {
    /// Unix milliseconds at which the parser observed the record's **root**
    /// line. Always present, which is what makes a stream of frames sortable: a
    /// line whose header did not parse has no clock of its own, and clash
    /// premium's and clash-rs's are inferred. A multi-line record keeps the
    /// instant of the line that opened it.
    pub at: i64,
    pub epoch: u64,
    pub kind: ClashCoreKind,
    pub stream: LogStream,
    pub level: LogLevel,
    pub timestamp: Option<LogTimestamp>,
    /// clash premium's `[Tag]` prefix (a message convention, not a field), meow's
    /// tracing target, or clash-rs's `file:line`.
    pub target: Option<String>,
    pub message: String,
    /// Filled only where field boundaries are decidable, which today means
    /// mihomo's quoted logfmt.
    pub fields: Vec<LogField>,
    /// The whole logical record with ANSI removed, continuations included.
    pub raw: String,
    /// Content was dropped at the parser boundary, either because continuations
    /// stopped fitting or because a single value exceeded its cap.
    pub truncated: bool,
}
