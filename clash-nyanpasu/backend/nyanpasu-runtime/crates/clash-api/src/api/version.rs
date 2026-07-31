use reqwest::Method;

use crate::{Client, Result, retry::RequestMetadata};

/// Core version information. Clash Premium omits `meta`, so it defaults to
/// `false` for non-mihomo cores.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct Version {
    #[serde(default)]
    pub meta: bool,
    pub version: String,
}

impl Client {
    pub async fn version(&self) -> Result<Version> {
        const OPERATION: &str = "version";
        self.send_json(RequestMetadata::new(OPERATION, Method::GET, true), || {
            self.get("/version")
        })
        .await
    }
}
