use crate::api::R;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::PathBuf};

pub const CORE_CHECK_ENDPOINT: &str = "/core/check";

/// Dry-run a config against a core binary. Touches no running core and needs
/// none: it invokes the binary's own config check, so a GUI can validate before
/// saving instead of finding out by failing to start (report §4 P1-B).
///
/// Its own type rather than a reuse of
/// [`CoreStartReq`](super::start::CoreStartReq): the two operations happen to
/// take the same two fields today and must stay free to diverge.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreCheckReq<'n> {
    pub core_type: Cow<'n, nyanpasu_utils::core::CoreType>,
    pub config_file: Cow<'n, PathBuf>,
}

/// A rejected config is an error envelope with
/// `error_kind = "config_check_failed"` and the core's own diagnostic in `msg`,
/// not a `200` carrying `ok: false` — every other operation in this protocol
/// reports failure through the envelope, and a second convention for one
/// endpoint would fragment it.
pub type CoreCheckRes<'a> = R<'a, ()>;
