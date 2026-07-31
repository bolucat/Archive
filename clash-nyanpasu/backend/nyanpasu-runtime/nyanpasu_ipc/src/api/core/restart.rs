use crate::api::R;

pub const CORE_RESTART_ENDPOINT: &str = "/core/restart";

pub type CoreRestartRes<'a> = R<'a, ()>;
