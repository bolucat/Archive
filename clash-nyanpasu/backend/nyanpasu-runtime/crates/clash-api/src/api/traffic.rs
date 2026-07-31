use std::fmt;

use reqwest::Method;

use crate::{Client, HttpStream, Result, retry::RequestMetadata};

/// A byte count as represented by Mihomo's signed Go `int64` counters.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    serde::Serialize,
    specta::Type,
)]
#[serde(transparent)]
pub struct Bytes(i64);

impl Bytes {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl From<i64> for Bytes {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<Bytes> for i64 {
    fn from(value: Bytes) -> Self {
        value.0
    }
}

impl fmt::Display for Bytes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A byte-per-second rate as represented by Mihomo's signed Go `int64` counters.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    serde::Serialize,
    specta::Type,
)]
#[serde(transparent)]
pub struct BytesPerSecond(i64);

impl BytesPerSecond {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

impl From<i64> for BytesPerSecond {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<BytesPerSecond> for i64 {
    fn from(value: BytesPerSecond) -> Self {
        value.0
    }
}

impl fmt::Display for BytesPerSecond {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// One `/traffic` sample.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(rename_all = "camelCase")]
pub struct Traffic {
    pub up: BytesPerSecond,
    pub down: BytesPerSecond,
    pub up_total: Bytes,
    pub down_total: Bytes,
}

impl Client {
    /// Open Mihomo's newline-delimited `/traffic` HTTP stream.
    pub async fn traffic(&self) -> Result<HttpStream<Traffic>> {
        const OPERATION: &str = "traffic";
        let metadata = RequestMetadata::new(OPERATION, Method::GET, true);
        let response = self.send(metadata, || self.get("/traffic")).await?;
        Ok(HttpStream::from_response(response, OPERATION))
    }

    /// Complete the `/traffic` WebSocket handshake and return the raw socket.
    ///
    /// This method retries only the handshake according to the injected policy.
    /// Once returned, reconnection and frame decoding belong to the caller.
    pub async fn traffic_ws(&self) -> Result<reqwest_websocket::WebSocket> {
        const OPERATION: &str = "traffic_ws";
        let metadata = RequestMetadata::new(OPERATION, Method::GET, true);

        self.websocket(metadata, || self.get("/traffic")).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_uses_all_four_signed_mihomo_fields() {
        let traffic: Traffic =
            serde_json::from_str(r#"{"up":-1,"down":2,"upTotal":3,"downTotal":4}"#).unwrap();

        assert_eq!(traffic.up.get(), -1);
        assert_eq!(traffic.down.get(), 2);
        assert_eq!(traffic.up_total.get(), 3);
        assert_eq!(traffic.down_total.get(), 4);
    }
}
