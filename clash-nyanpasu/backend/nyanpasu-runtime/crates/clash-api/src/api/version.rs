use reqwest::Method;

use crate::{Client, Result, retry::RequestMetadata};

/// Core version information. Clash Premium omits `meta`, so it defaults to
/// `false` for non-mihomo cores.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct Version {
    #[serde(default)]
    pub meta: bool,
    pub version: String,
    #[serde(default)]
    pub premium: Option<bool>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_accept_clash_rs_and_preserve_premium_flag() {
        let minimal: Version = serde_json::from_str(r#"{"version":"1"}"#).unwrap();
        assert!(!minimal.meta);
        assert_eq!(minimal.premium, None);
        let premium: Version = serde_json::from_str(r#"{"version":"1","premium":true}"#).unwrap();
        assert_eq!(premium.premium, Some(true));
    }
}
