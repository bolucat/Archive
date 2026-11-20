use crate::api::{R, status::RevisionIdInfo};
use serde::{Deserialize, Serialize};

pub const NETWORK_TRANSPARENT_PROXY_RECONCILE_ENDPOINT: &str =
    "/network/transparent-proxy/reconcile";
pub const NETWORK_TRANSPARENT_PROXY_STATUS_ENDPOINT: &str = "/network/transparent-proxy/status";
pub const NETWORK_TRANSPARENT_PROXY_DIAGNOSTICS_ENDPOINT: &str =
    "/network/transparent-proxy/diagnostics";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum NetworkTransparentProxyProtocol {
    Tcp,
    Udp,
}

/// The capture scope last successfully applied by this service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyAppliedScope {
    pub mode: NetworkTransparentProxyMode,
    pub port: u16,
    pub local: bool,
    pub interfaces: Vec<String>,
    pub ipv6: bool,
    /// Derived from the mode: TPROXY captures TCP and UDP, REDIRECT only TCP.
    pub protocols: Vec<NetworkTransparentProxyProtocol>,
}

/// One kernel object this service manages or expects for the applied capture scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum NetworkTransparentProxyObject {
    NftTable,
    NftOutputChain,
    NftPreroutingChain,
    NftOutputIpv4Capture,
    NftOutputIpv6Capture,
    NftPreroutingIpv4Capture,
    NftPreroutingIpv6Capture,
    PolicyRuleIpv4,
    PolicyRuleIpv6,
    PolicyRouteIpv4,
    PolicyRouteIpv6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum NetworkTransparentProxyObjectState {
    Present,
    Absent,
    Foreign,
    ReadError,
    /// The object is not part of this mode or configured scope (for example
    /// policy routing while REDIRECT is active).
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyObjectObservation {
    pub object: NetworkTransparentProxyObject,
    /// Whether the applied scope requires this object.
    pub expected: bool,
    pub state: NetworkTransparentProxyObjectState,
}

/// The fixed packet marks and policy-routing identity used by transparent capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyMarks {
    pub capture_mark: u32,
    pub capture_mask: u32,
    pub egress_mark: u32,
}

/// The policy-routing rule and table identifiers, displayed for diagnostics only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyRouting {
    pub rule_priority: u32,
    pub table: u32,
}

/// The fixed bypass classes in the generated capture rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyBypass {
    /// Root-owned local traffic is excluded from local capture.
    pub root_uid: bool,
    /// Destinations routed as local are excluded.
    pub local_destination: bool,
    /// Loopback, link-local, private, documentation, multicast and reserved
    /// destinations are excluded.
    pub private_reserved: bool,
    /// Core egress carrying the bypass mark is excluded.
    pub core_egress_mark: bool,
}

/// A read-only snapshot of the service's applied scope and observed kernel objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkTransparentProxyDiagnostics {
    pub supported: bool,
    /// The last applied scope, even when it is stale and awaiting cleanup.
    pub applied: Option<NetworkTransparentProxyAppliedScope>,
    /// The applied scope no longer matches the running core, or owned resources
    /// remain without an applied scope.
    pub stale: bool,
    pub marks: NetworkTransparentProxyMarks,
    pub routing: NetworkTransparentProxyRouting,
    pub bypass: NetworkTransparentProxyBypass,
    pub objects: Vec<NetworkTransparentProxyObjectObservation>,
}

pub type NetworkTransparentProxyReconcileRes<'a> = R<'a, NetworkTransparentProxyStatus>;
pub type NetworkTransparentProxyStatusRes<'a> = R<'a, NetworkTransparentProxyStatus>;
pub type NetworkTransparentProxyDiagnosticsRes<'a> = R<'a, NetworkTransparentProxyDiagnostics>;
