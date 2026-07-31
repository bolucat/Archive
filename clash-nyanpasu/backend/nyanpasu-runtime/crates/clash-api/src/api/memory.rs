use reqwest::Method;

use crate::{Client, HttpStream, Result, retry::RequestMetadata};

/// One `/memory` sample.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
pub struct Memory {
    #[serde(rename = "inuse")]
    pub in_use: u64,
    #[serde(rename = "oslimit")]
    pub os_limit: u64,
}

impl Client {
    /// Open Mihomo's newline-delimited `/memory` HTTP stream.
    pub async fn memory(&self) -> Result<HttpStream<Memory>> {
        const OPERATION: &str = "memory";
        let response = self
            .send(RequestMetadata::new(OPERATION, Method::GET, true), || {
                self.get("/memory")
            })
            .await?;
        Ok(HttpStream::from_response(response, OPERATION))
    }

    /// Complete the `/memory` handshake and return the raw WebSocket.
    pub async fn memory_ws(&self) -> Result<reqwest_websocket::WebSocket> {
        self.websocket(RequestMetadata::new("memory_ws", Method::GET, true), || {
            self.get("/memory")
        })
        .await
    }
}
