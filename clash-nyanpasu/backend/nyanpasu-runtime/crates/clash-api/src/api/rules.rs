use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use indexmap::IndexMap;
use reqwest::Method;

use crate::{Client, ProviderType, Result, VehicleType, retry::RequestMetadata};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct Rule {
    pub index: i64,
    #[serde(rename = "type")]
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub size: i64,
    #[serde(default)]
    pub extra: Option<RuleExtra>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleExtra {
    pub disabled: bool,
    pub hit_count: u64,
    pub hit_at: DateTime<FixedOffset>,
    pub miss_count: u64,
    pub miss_at: DateTime<FixedOffset>,
}

/// Batch of rule indices to enable (`false`) or disable (`true`).
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub struct RulePatch(BTreeMap<usize, bool>);

impl RulePatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_disabled(&mut self, index: usize, disabled: bool) -> &mut Self {
        self.0.insert(index, disabled);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(transparent)]
pub struct RuleProviderName(String);

impl RuleProviderName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RuleProviderName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for RuleProviderName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum RuleProviderBehavior {
    Domain,
    #[serde(rename = "IPCIDR")]
    IpCidr,
    Classical,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum RuleFormat {
    YamlRule,
    TextRule,
    MrsRule,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleProvider {
    pub behavior: RuleProviderBehavior,
    pub format: RuleFormat,
    pub name: RuleProviderName,
    pub rule_count: i64,
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    pub vehicle_type: VehicleType,
    pub updated_at: DateTime<FixedOffset>,
    #[serde(default)]
    pub payload: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct RuleList {
    rules: Vec<Rule>,
}

#[derive(serde::Deserialize)]
struct RuleProviderMap {
    providers: IndexMap<RuleProviderName, RuleProvider>,
}

impl Client {
    pub async fn rules(&self) -> Result<Vec<Rule>> {
        let result: RuleList = self
            .send_json(RequestMetadata::new("rules", Method::GET, true), || {
                self.get("/rules/")
            })
            .await?;
        Ok(result.rules)
    }

    pub async fn patch_rules(&self, patch: &RulePatch) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("patch_rules", Method::PATCH, false),
            || Ok(self.patch("/rules/disable")?.json(patch)),
        )
        .await
    }

    pub async fn rule_providers(&self) -> Result<IndexMap<RuleProviderName, RuleProvider>> {
        let result: RuleProviderMap = self
            .send_json(
                RequestMetadata::new("rule_providers", Method::GET, true),
                || self.get("/providers/rules/"),
            )
            .await?;
        Ok(result.providers)
    }

    pub async fn update_rule_provider(&self, provider: &RuleProviderName) -> Result<()> {
        let url = self.endpoint_with_segments("/providers/rules", [provider.as_str(), ""])?;
        self.send_empty(
            RequestMetadata::new("update_rule_provider", Method::PUT, false),
            || Ok(self.request_url(Method::PUT, url.clone())),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_patch_serializes_indices_as_json_object_keys() {
        let mut patch = RulePatch::new();
        patch.set_disabled(12, true).set_disabled(3, false);
        assert_eq!(
            serde_json::to_value(patch).unwrap(),
            serde_json::json!({"3": false, "12": true})
        );
    }
}
