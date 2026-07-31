use reqwest::Method;

use crate::{Client, Result, retry::RequestMetadata};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct Hello {
    pub hello: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct StatusResponse {
    pub status: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct UpgradeOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    pub force: bool,
}

impl Client {
    pub async fn hello(&self) -> Result<Hello> {
        self.send_json(RequestMetadata::new("hello", Method::GET, true), || {
            self.get("/")
        })
        .await
    }

    /// Ask a debug-enabled Mihomo controller to return unused memory to the OS.
    pub async fn collect_garbage(&self) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("collect_garbage", Method::PUT, false),
            || self.put("/debug/gc"),
        )
        .await
    }

    pub async fn flush_fake_ip_cache(&self) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("flush_fake_ip_cache", Method::POST, false),
            || self.post("/cache/fakeip/flush"),
        )
        .await
    }

    pub async fn flush_dns_cache(&self) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("flush_dns_cache", Method::POST, false),
            || self.post("/cache/dns/flush"),
        )
        .await
    }

    pub async fn restart(&self) -> Result<StatusResponse> {
        self.send_json(RequestMetadata::new("restart", Method::POST, false), || {
            self.post("/restart")
        })
        .await
    }

    pub async fn upgrade(&self, options: &UpgradeOptions) -> Result<StatusResponse> {
        self.send_json(RequestMetadata::new("upgrade", Method::POST, false), || {
            Ok(self.post("/upgrade")?.query(options))
        })
        .await
    }

    pub async fn upgrade_ui(&self) -> Result<StatusResponse> {
        self.send_json(
            RequestMetadata::new("upgrade_ui", Method::POST, false),
            || self.post("/upgrade/ui"),
        )
        .await
    }

    pub async fn upgrade_geo_databases(&self) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("upgrade_geo_databases", Method::POST, false),
            || self.post("/upgrade/geo"),
        )
        .await
    }
}
