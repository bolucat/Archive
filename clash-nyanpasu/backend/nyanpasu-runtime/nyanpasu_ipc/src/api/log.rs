use crate::api::R;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub const LOGS_RETRIEVE_ENDPOINT: &str = "/logs/retrieve";
pub const LOGS_INSPECT_ENDPOINT: &str = "/logs/inspect";

// TODO: more health check fields
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogsResBody<'a> {
    pub logs: Vec<Cow<'a, str>>,
}

pub type LogsRes<'a> = R<'a, LogsResBody<'a>>;
