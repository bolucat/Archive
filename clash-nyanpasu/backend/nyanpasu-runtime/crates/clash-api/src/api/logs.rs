use chrono::NaiveTime;
use reqwest::Method;

use crate::{Client, HttpStream, Result, retry::RequestMetadata};

/// Minimum severity accepted by Mihomo's `/logs` subscription.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    #[default]
    Info,
    Warning,
    Error,
    Silent,
}

/// Query shared by regular and structured log subscriptions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct LogQuery {
    pub level: LogLevel,
}

impl LogQuery {
    pub const fn new(level: LogLevel) -> Self {
        Self { level }
    }
}

/// A standard Mihomo log event.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct LogEntry {
    #[serde(rename = "type")]
    pub level: LogLevel,
    pub payload: String,
}

/// Severity spelling used by `format=structured` (`warn`, not `warning`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum StructuredLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct LogField {
    pub key: String,
    pub value: String,
}

/// A `format=structured` Mihomo log event.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct StructuredLogEntry {
    pub time: NaiveTime,
    pub level: StructuredLogLevel,
    pub message: String,
    pub fields: Vec<LogField>,
}

impl Client {
    pub async fn logs(&self, query: LogQuery) -> Result<HttpStream<LogEntry>> {
        const OPERATION: &str = "logs";
        let response = self
            .send(RequestMetadata::new(OPERATION, Method::GET, true), || {
                Ok(self.get("/logs")?.query(&query))
            })
            .await?;
        Ok(HttpStream::from_response(response, OPERATION))
    }

    pub async fn logs_ws(&self, query: LogQuery) -> Result<reqwest_websocket::WebSocket> {
        self.websocket(RequestMetadata::new("logs_ws", Method::GET, true), || {
            Ok(self.get("/logs")?.query(&query))
        })
        .await
    }

    pub async fn structured_logs(&self, query: LogQuery) -> Result<HttpStream<StructuredLogEntry>> {
        const OPERATION: &str = "structured_logs";
        let response = self
            .send(RequestMetadata::new(OPERATION, Method::GET, true), || {
                Ok(self
                    .get("/logs")?
                    .query(&query)
                    .query(&[("format", "structured")]))
            })
            .await?;
        Ok(HttpStream::from_response(response, OPERATION))
    }

    pub async fn structured_logs_ws(
        &self,
        query: LogQuery,
    ) -> Result<reqwest_websocket::WebSocket> {
        self.websocket(
            RequestMetadata::new("structured_logs_ws", Method::GET, true),
            || {
                Ok(self
                    .get("/logs")?
                    .query(&query)
                    .query(&[("format", "structured")]))
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_log_uses_mihomos_time_and_warn_spelling() {
        let entry: StructuredLogEntry =
            serde_json::from_str(r#"{"time":"12:34:56","level":"warn","message":"x","fields":[]}"#)
                .unwrap();

        assert_eq!(entry.time, NaiveTime::from_hms_opt(12, 34, 56).unwrap());
        assert_eq!(entry.level, StructuredLogLevel::Warn);
    }
}
