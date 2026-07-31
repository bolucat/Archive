use std::{str::FromStr, time::Duration};

use chrono::{DateTime, FixedOffset};
use indexmap::IndexMap;
use reqwest::{Method, Url};

use crate::{Client, Error, Result, retry::RequestMetadata};

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub struct ProxyName(String);

impl ProxyName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProxyName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProxyName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Display for ProxyName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub struct ProviderName(String);

impl ProviderName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ProviderName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ProviderName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct DelayHistory {
    pub time: DateTime<FixedOffset>,
    pub delay: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct ProxyExtra {
    pub alive: bool,
    pub history: Vec<DelayHistory>,
}

/// Common and group-specific fields emitted by Mihomo's proxy wrappers.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub name: ProxyName,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub history: Vec<DelayHistory>,
    pub extra: IndexMap<String, ProxyExtra>,
    pub alive: bool,
    pub udp: bool,
    pub uot: bool,
    pub xudp: bool,
    pub tfo: bool,
    pub mptcp: bool,
    pub smux: bool,
    pub interface: String,
    #[serde(rename = "routing-mark")]
    pub routing_mark: i64,
    #[serde(rename = "provider-name")]
    pub provider_name: String,
    #[serde(rename = "dialer-proxy")]
    pub dialer_proxy: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub now: Option<ProxyName>,
    #[serde(default)]
    pub all: Option<Vec<ProxyName>>,
    #[serde(default)]
    pub test_url: Option<String>,
    #[serde(default)]
    pub expected_status: Option<String>,
    #[serde(default)]
    pub fixed: Option<ProxyName>,
    #[serde(default)]
    pub hidden: Option<bool>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub empty_fallback: Option<ProxyName>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum ProviderType {
    Proxy,
    Rule,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum VehicleType {
    File,
    #[serde(rename = "HTTP")]
    Http,
    Compatible,
    Inline,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "PascalCase")]
pub struct SubscriptionInfo {
    pub upload: i64,
    pub download: i64,
    pub total: i64,
    pub expire: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProxyProvider {
    pub name: ProviderName,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub vehicle_type: VehicleType,
    pub proxies: Vec<Proxy>,
    pub test_url: String,
    pub expected_status: String,
    #[serde(default)]
    pub updated_at: Option<DateTime<FixedOffset>>,
    #[serde(default)]
    pub subscription_info: Option<SubscriptionInfo>,
}

/// Allowed response status expression such as `200/204/401-429`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, specta::Type)]
#[specta(transparent)]
pub struct ExpectedStatus(String);

impl ExpectedStatus {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        validate_expected_status(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ExpectedStatus {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

/// URL test parameters shared by group, proxy, and provider endpoints.
#[derive(Clone, Debug, PartialEq, Eq, specta::Type)]
pub struct DelayQuery {
    pub url: Url,
    pub timeout: Duration,
    pub expected: Option<ExpectedStatus>,
}

impl DelayQuery {
    pub fn new(url: Url, timeout: Duration) -> Result<Self> {
        if timeout.as_millis() == 0 {
            return Err(Error::InvalidArgument {
                argument: "timeout",
                message: "must be at least one millisecond".to_owned(),
            });
        }
        if timeout.as_millis() > i32::MAX as u128 {
            return Err(Error::InvalidArgument {
                argument: "timeout",
                message: "does not fit Mihomo's group timeout".to_owned(),
            });
        }
        Ok(Self {
            url,
            timeout,
            expected: None,
        })
    }

    pub fn with_expected(mut self, expected: ExpectedStatus) -> Self {
        self.expected = Some(expected);
        self
    }

    fn query_pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = vec![
            ("url", self.url.as_str().to_owned()),
            ("timeout", self.timeout.as_millis().to_string()),
        ];
        if let Some(expected) = &self.expected {
            pairs.push(("expected", expected.as_str().to_owned()));
        }
        pairs
    }

    fn validate_proxy_timeout(&self) -> Result<()> {
        if self.timeout.as_millis() > i16::MAX as u128 {
            return Err(Error::InvalidArgument {
                argument: "timeout",
                message: "proxy delay endpoints accept at most 32767 milliseconds".to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct Delay {
    pub delay: u16,
}

#[derive(serde::Deserialize)]
struct ProxyMap {
    proxies: IndexMap<ProxyName, Proxy>,
}

#[derive(serde::Deserialize)]
struct ProxyList {
    proxies: Vec<Proxy>,
}

#[derive(serde::Deserialize)]
struct ProviderMap {
    providers: IndexMap<ProviderName, ProxyProvider>,
}

#[derive(serde::Serialize)]
struct SelectProxyRequest<'a> {
    name: &'a ProxyName,
}

impl Client {
    pub async fn groups(&self) -> Result<Vec<Proxy>> {
        let result: ProxyList = self
            .send_json(RequestMetadata::new("groups", Method::GET, true), || {
                self.get("/group/")
            })
            .await?;
        Ok(result.proxies)
    }

    pub async fn group(&self, name: &ProxyName) -> Result<Proxy> {
        let url = self.endpoint_with_segments("/group", [name.as_str(), ""])?;
        self.send_json(RequestMetadata::new("group", Method::GET, true), || {
            Ok(self.request_url(Method::GET, url.clone()))
        })
        .await
    }

    pub async fn group_delay(
        &self,
        name: &ProxyName,
        query: &DelayQuery,
    ) -> Result<IndexMap<ProxyName, u16>> {
        let url = self.endpoint_with_segments("/group", [name.as_str(), "delay"])?;
        let query = query.query_pairs();
        self.send_json(
            RequestMetadata::new("group_delay", Method::GET, false),
            || Ok(self.request_url(Method::GET, url.clone()).query(&query)),
        )
        .await
    }

    pub async fn proxies(&self) -> Result<IndexMap<ProxyName, Proxy>> {
        let result: ProxyMap = self
            .send_json(RequestMetadata::new("proxies", Method::GET, true), || {
                self.get("/proxies/")
            })
            .await?;
        Ok(result.proxies)
    }

    pub async fn proxy(&self, name: &ProxyName) -> Result<Proxy> {
        let url = self.endpoint_with_segments("/proxies", [name.as_str(), ""])?;
        self.send_json(RequestMetadata::new("proxy", Method::GET, true), || {
            Ok(self.request_url(Method::GET, url.clone()))
        })
        .await
    }

    pub async fn proxy_delay(&self, name: &ProxyName, query: &DelayQuery) -> Result<Delay> {
        query.validate_proxy_timeout()?;
        let url = self.endpoint_with_segments("/proxies", [name.as_str(), "delay"])?;
        let query = query.query_pairs();
        self.send_json(
            RequestMetadata::new("proxy_delay", Method::GET, false),
            || Ok(self.request_url(Method::GET, url.clone()).query(&query)),
        )
        .await
    }

    pub async fn select_proxy(&self, group: &ProxyName, target: &ProxyName) -> Result<()> {
        let url = self.endpoint_with_segments("/proxies", [group.as_str(), ""])?;
        self.send_empty(
            RequestMetadata::new("select_proxy", Method::PUT, false),
            || {
                Ok(self
                    .request_url(Method::PUT, url.clone())
                    .json(&SelectProxyRequest { name: target }))
            },
        )
        .await
    }

    pub async fn clear_proxy_selection(&self, group: &ProxyName) -> Result<()> {
        let url = self.endpoint_with_segments("/proxies", [group.as_str(), ""])?;
        self.send_empty(
            RequestMetadata::new("clear_proxy_selection", Method::DELETE, false),
            || Ok(self.request_url(Method::DELETE, url.clone())),
        )
        .await
    }

    pub async fn proxy_providers(&self) -> Result<IndexMap<ProviderName, ProxyProvider>> {
        let result: ProviderMap = self
            .send_json(
                RequestMetadata::new("proxy_providers", Method::GET, true),
                || self.get("/providers/proxies/"),
            )
            .await?;
        Ok(result.providers)
    }

    pub async fn proxy_provider(&self, provider: &ProviderName) -> Result<ProxyProvider> {
        let url = self.endpoint_with_segments("/providers/proxies", [provider.as_str(), ""])?;
        self.send_json(
            RequestMetadata::new("proxy_provider", Method::GET, true),
            || Ok(self.request_url(Method::GET, url.clone())),
        )
        .await
    }

    pub async fn update_proxy_provider(&self, provider: &ProviderName) -> Result<()> {
        let url = self.endpoint_with_segments("/providers/proxies", [provider.as_str(), ""])?;
        self.send_empty(
            RequestMetadata::new("update_proxy_provider", Method::PUT, false),
            || Ok(self.request_url(Method::PUT, url.clone())),
        )
        .await
    }

    /// Trigger provider-wide health checking. Mihomo returns 204 with no body.
    pub async fn healthcheck_proxy_provider(&self, provider: &ProviderName) -> Result<()> {
        let url =
            self.endpoint_with_segments("/providers/proxies", [provider.as_str(), "healthcheck"])?;
        self.send_empty(
            RequestMetadata::new("healthcheck_proxy_provider", Method::GET, false),
            || Ok(self.request_url(Method::GET, url.clone())),
        )
        .await
    }

    pub async fn provider_proxy(
        &self,
        provider: &ProviderName,
        proxy: &ProxyName,
    ) -> Result<Proxy> {
        let url = self.endpoint_with_segments(
            "/providers/proxies",
            [provider.as_str(), proxy.as_str(), ""],
        )?;
        self.send_json(
            RequestMetadata::new("provider_proxy", Method::GET, true),
            || Ok(self.request_url(Method::GET, url.clone())),
        )
        .await
    }

    pub async fn provider_proxy_delay(
        &self,
        provider: &ProviderName,
        proxy: &ProxyName,
        query: &DelayQuery,
    ) -> Result<Delay> {
        query.validate_proxy_timeout()?;
        let url = self.endpoint_with_segments(
            "/providers/proxies",
            [provider.as_str(), proxy.as_str(), "healthcheck"],
        )?;
        let query = query.query_pairs();
        self.send_json(
            RequestMetadata::new("provider_proxy_delay", Method::GET, false),
            || Ok(self.request_url(Method::GET, url.clone()).query(&query)),
        )
        .await
    }
}

fn validate_expected_status(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(Error::InvalidArgument {
            argument: "expected",
            message: "must not be empty".to_owned(),
        });
    }

    for item in value.split('/') {
        let valid = if let Some((start, end)) = item.split_once('-') {
            parse_status(start)
                .zip(parse_status(end))
                .is_some_and(|(start, end)| start <= end)
        } else {
            parse_status(item).is_some()
        };
        if !valid {
            return Err(Error::InvalidArgument {
                argument: "expected",
                message: format!("invalid status expression `{value}`"),
            });
        }
    }
    Ok(())
}

fn parse_status(value: &str) -> Option<u16> {
    value.parse::<u16>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_status_validates_ranges() {
        assert!(ExpectedStatus::new("200/204/401-429").is_ok());
        assert!(ExpectedStatus::new("429-401").is_err());
        assert!(ExpectedStatus::new("200/").is_err());
    }

    #[test]
    fn proxy_delay_rejects_values_mihomos_int16_parser_cannot_hold() {
        let query = DelayQuery::new(
            Url::parse("https://example.com").unwrap(),
            Duration::from_secs(40),
        )
        .unwrap();
        assert!(query.validate_proxy_timeout().is_err());
    }
}
