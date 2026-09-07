use reqwest::Method;

use crate::{Client, HttpStream, Result, retry::RequestMetadata};

/// One `/memory` sample.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
pub struct Memory {
    #[serde(
        rename = "inuse",
        deserialize_with = "crate::stream::deserialize_number"
    )]
    pub in_use: u64,
    #[serde(
        default,
        rename = "oslimit",
        deserialize_with = "crate::stream::deserialize_number"
    )]
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

    /// Open the typed `/memory` WebSocket.
    pub async fn memory_ws(&self) -> Result<crate::WebSocketStream<Memory>> {
        self.websocket(RequestMetadata::new("memory_ws", Method::GET, true), || {
            self.get("/memory")
        })
        .await
        .map(|socket| crate::WebSocketStream::new(socket, "memory_ws"))
    }
}
