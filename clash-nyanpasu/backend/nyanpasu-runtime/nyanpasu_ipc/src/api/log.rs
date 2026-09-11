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

pub const LOG_FILES_ENDPOINT: &str = "/v1/logs/files";
pub const LOG_OPEN_ENDPOINT: &str = "/v1/logs/open";
pub const LOG_QUERY_ENDPOINT: &str = "/v1/logs/query";
pub const LOG_CLOSE_ENDPOINT: &str = "/v1/logs/close";
pub const LOG_QUERY_VERSION: u32 = 1;

/// Session ownership is scoped by the authorized local IPC client instance/window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedLogRequest<T> {
    pub owner: String,
    pub request: T,
}
