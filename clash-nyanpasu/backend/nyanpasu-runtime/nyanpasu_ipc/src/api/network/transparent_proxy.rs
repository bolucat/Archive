use crate::api::{R, status::RevisionIdInfo};
use serde::{Deserialize, Serialize};

pub const NETWORK_TRANSPARENT_PROXY_RECONCILE_ENDPOINT: &str =
    "/network/transparent-proxy/reconcile";
pub const NETWORK_TRANSPARENT_PROXY_STATUS_ENDPOINT: &str = "/network/transparent-proxy/status";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum NetworkTransparentProxyMode {
    Disabled,
    Redir,
    Tproxy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyRequest {
    pub mode: NetworkTransparentProxyMode,
    pub port: u16,
    pub local: bool,
    pub interfaces: Vec<String>,
    pub ipv6: bool,
    pub expected_revision: RevisionIdInfo,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyStatus {
    pub supported: bool,
    pub active: bool,
    pub mode: Option<NetworkTransparentProxyMode>,
    pub revision: Option<RevisionIdInfo>,
    pub error: Option<String>,
}

pub type NetworkTransparentProxyReconcileRes<'a> = R<'a, NetworkTransparentProxyStatus>;
pub type NetworkTransparentProxyStatusRes<'a> = R<'a, NetworkTransparentProxyStatus>;
