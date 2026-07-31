use crate::api::R;

pub const CORE_STOP_ENDPOINT: &str = "/core/stop";

pub type CoreStopRes<'a> = R<'a, ()>;
