/// Exit information of a terminated child. Field semantics match the legacy
/// `core::TerminatedPayload` so downstream migration is a rename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminatedPayload {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// Events delivered on the channel returned by [`crate::process::Command::spawn`].
///
/// Ordering: when `Terminated` is delivered it is the final event on the
/// channel. Delivery is best-effort — a receiver that stops draining during
/// termination may never see `Terminated` (it is dropped after a short stall so
/// the pump can exit). Use [`crate::process::ProcessHandle::wait`] for the
/// authoritative termination signal.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    Stdout(String),
    Stderr(String),
    /// Non-fatal IO/decode error while pumping output. The process may still be alive.
    Error(String),
    Terminated(TerminatedPayload),
}
