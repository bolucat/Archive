use crate::{Direction, Filter, Level};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum LogError {
    #[error("log source unavailable")]
    Unavailable,
    #[error("service does not support log queries")]
    Unsupported,
    #[error("log session expired")]
    SessionExpired,
    #[error("log file changed; reset cursor")]
    CursorReset,
    #[error("log file no longer exists")]
    FileGone,
    #[error("log resource limit reached")]
    Limit,
    #[error("invalid log request")]
    InvalidRequest,
}
pub type LogResult<T> = Result<T, LogError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogFileInfo {
    pub id: String,
    pub name: String,
    pub bytes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct OpenLogs {
    pub request_id: String,
    pub file: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogSession {
    pub id: String,
    pub lease_ms: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogCursor {
    pub generation: String,
    pub offset: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct QueryLogs {
    pub session: String,
    pub filter: Filter,
    pub direction: Direction,
    pub cursor: Option<LogCursor>,
    pub limit: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogRow {
    pub id: String,
    pub timestamp: Option<String>,
    pub level: Level,
    pub target: String,
    pub message: String,
    pub raw: String,
    pub unparsed: bool,
    pub truncated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogPage {
    pub rows: Vec<LogRow>,
    pub cursor: LogCursor,
    pub head: LogCursor,
    pub start: String,
    pub file: String,
    pub building: bool,
    pub more: bool,
    pub partial: bool,
    pub malformed: String,
    pub truncated: String,
    pub indexed_bytes: String,
    pub file_bytes: String,
}
