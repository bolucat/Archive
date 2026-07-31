use crate::api::R;

pub const CORE_RECOVER_ENDPOINT: &str = "/core/recover";

/// Clear the quarantine latch left behind by an epoch whose death could not be
/// confirmed. Until it is cleared every lifecycle operation fails with
/// `error_kind = "quarantined"`, and before S8 the only remedy was restarting
/// the whole service (report §1.3).
///
/// No request body, and no payload on success: recovery is idempotent and
/// succeeds when nothing was quarantined, which is what makes it safe to call
/// speculatively. Whether a latch was actually cleared is deliberately not
/// reported — the manager does not expose it (see the S8 plan §12/D3).
pub type CoreRecoverRes<'a> = R<'a, ()>;
