use std::net::IpAddr;

use indexmap::IndexMap;
use ipnet::IpNet;
use reqwest::Method;

use crate::{Client, LogLevel, Result, retry::RequestMetadata};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum TunnelMode {
    Global,
    Rule,
    Direct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum FindProcessMode {
    Strict,
    Always,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum TunStack {
    #[serde(rename = "gVisor")]
    Gvisor,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "mixed")]
    Mixed,
    #[serde(other)]
    Unknown,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct GeoUrls {
    pub geo_ip: String,
    pub mmdb: String,
    pub asn: String,
    pub geo_site: String,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default)]
pub struct BrutalOptions {
    pub enabled: bool,
    pub up: String,
    pub down: String,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default)]
pub struct MuxOptions {
    pub padding: bool,
    pub brutal: BrutalOptions,
}

/// Full TUN object returned by `GET /configs`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuntimeTun {
    pub enable: bool,
    pub device: String,
    pub stack: TunStack,
    pub dns_hijack: Vec<String>,
    pub auto_route: bool,
    pub auto_detect_interface: bool,
    pub mtu: u32,
    pub gso: bool,
    pub gso_max_size: u32,
    #[specta(type = Vec<String>)]
    pub inet4_address: Vec<IpNet>,
    #[specta(type = Vec<String>)]
    pub inet6_address: Vec<IpNet>,
    pub iproute2_table_index: i64,
    pub iproute2_rule_index: i64,
    pub auto_redirect: bool,
    pub auto_redirect_input_mark: u32,
    pub auto_redirect_output_mark: u32,
    pub auto_redirect_iproute2_fallback_rule_index: i64,
    pub loopback_address: Vec<IpAddr>,
    pub strict_route: bool,
    #[specta(type = Vec<String>)]
    pub route_address: Vec<IpNet>,
    pub route_address_set: Vec<String>,
    #[specta(type = Vec<String>)]
    pub route_exclude_address: Vec<IpNet>,
    pub route_exclude_address_set: Vec<String>,
    pub include_interface: Vec<String>,
    pub exclude_interface: Vec<String>,
    pub include_uid: Vec<u32>,
    pub include_uid_range: Vec<String>,
    pub exclude_uid: Vec<u32>,
    pub exclude_uid_range: Vec<String>,
    pub exclude_src_port: Vec<u16>,
    pub exclude_src_port_range: Vec<String>,
    pub exclude_dst_port: Vec<u16>,
    pub exclude_dst_port_range: Vec<String>,
    pub include_android_user: Vec<i64>,
    pub include_package: Vec<String>,
    pub exclude_package: Vec<String>,
    pub include_mac_address: Vec<String>,
    pub exclude_mac_address: Vec<String>,
    pub endpoint_independent_nat: bool,
    pub udp_timeout: i64,
    pub icmp_timeout: i64,
    pub disable_icmp_forwarding: bool,
    pub file_descriptor: i64,
    #[specta(type = Vec<String>)]
    pub inet4_route_address: Vec<IpNet>,
    #[specta(type = Vec<String>)]
    pub inet6_route_address: Vec<IpNet>,
    #[specta(type = Vec<String>)]
    pub inet4_route_exclude_address: Vec<IpNet>,
    #[specta(type = Vec<String>)]
    pub inet6_route_exclude_address: Vec<IpNet>,
    pub recvmsgx: bool,
    pub sendmsgx: bool,
}

impl Default for RuntimeTun {
    fn default() -> Self {
        Self {
            enable: false,
            device: String::new(),
            stack: TunStack::Unknown,
            dns_hijack: Vec::new(),
            auto_route: false,
            auto_detect_interface: false,
            mtu: 0,
            gso: false,
            gso_max_size: 0,
            inet4_address: Vec::new(),
            inet6_address: Vec::new(),
            iproute2_table_index: 0,
            iproute2_rule_index: 0,
            auto_redirect: false,
            auto_redirect_input_mark: 0,
            auto_redirect_output_mark: 0,
            auto_redirect_iproute2_fallback_rule_index: 0,
            loopback_address: Vec::new(),
            strict_route: false,
            route_address: Vec::new(),
            route_address_set: Vec::new(),
            route_exclude_address: Vec::new(),
            route_exclude_address_set: Vec::new(),
            include_interface: Vec::new(),
            exclude_interface: Vec::new(),
            include_uid: Vec::new(),
            include_uid_range: Vec::new(),
            exclude_uid: Vec::new(),
            exclude_uid_range: Vec::new(),
            exclude_src_port: Vec::new(),
            exclude_src_port_range: Vec::new(),
            exclude_dst_port: Vec::new(),
            exclude_dst_port_range: Vec::new(),
            include_android_user: Vec::new(),
            include_package: Vec::new(),
            exclude_package: Vec::new(),
            include_mac_address: Vec::new(),
            exclude_mac_address: Vec::new(),
            endpoint_independent_nat: false,
            udp_timeout: 0,
            icmp_timeout: 0,
            disable_icmp_forwarding: false,
            file_descriptor: 0,
            inet4_route_address: Vec::new(),
            inet6_route_address: Vec::new(),
            inet4_route_exclude_address: Vec::new(),
            inet6_route_exclude_address: Vec::new(),
            recvmsgx: false,
            sendmsgx: false,
        }
    }
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct RuntimeTuicServer {
    pub enable: bool,
    pub listen: String,
    pub token: Vec<String>,
    pub users: IndexMap<String, String>,
    pub certificate: String,
    pub private_key: String,
    pub client_auth_type: String,
    pub client_auth_cert: String,
    pub ech_key: String,
    pub congestion_controller: String,
    pub max_idle_time: i64,
    pub authentication_timeout: i64,
    pub alpn: Vec<String>,
    pub max_udp_relay_packet_size: i64,
    pub max_datagram_frame_size: i64,
    pub cwnd: i64,
    pub bbr_profile: String,
    pub mux_option: MuxOptions,
}

/// Runtime view returned by Mihomo's `GET /configs`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub struct RuntimeConfig {
    pub port: i64,
    pub socks_port: i64,
    pub redir_port: i64,
    pub tproxy_port: i64,
    pub mixed_port: i64,
    pub tun: RuntimeTun,
    pub tuic_server: RuntimeTuicServer,
    pub ss_config: String,
    pub vmess_config: String,
    #[serde(default)]
    pub tcptun_config: Option<String>,
    #[serde(default)]
    pub udptun_config: Option<String>,
    pub authentication: Option<Vec<String>>,
    #[specta(type = Option<Vec<String>>)]
    pub skip_auth_prefixes: Option<Vec<IpNet>>,
    #[specta(type = Option<Vec<String>>)]
    pub lan_allowed_ips: Option<Vec<IpNet>>,
    #[specta(type = Option<Vec<String>>)]
    pub lan_disallowed_ips: Option<Vec<IpNet>>,
    pub allow_lan: bool,
    pub bind_address: String,
    pub inbound_tfo: bool,
    pub inbound_mptcp: bool,
    pub mode: TunnelMode,
    pub unified_delay: bool,
    pub log_level: LogLevel,
    pub ipv6: bool,
    pub interface_name: String,
    pub routing_mark: i64,
    pub geox_url: GeoUrls,
    pub geo_auto_update: bool,
    pub geo_update_interval: i64,
    pub geodata_mode: bool,
    pub geodata_loader: String,
    pub geosite_matcher: String,
    pub tcp_concurrent: bool,
    pub find_process_mode: FindProcessMode,
    pub sniffing: bool,
    pub global_ua: String,
    pub etag_support: bool,
    pub keep_alive_idle: i64,
    pub keep_alive_interval: i64,
    pub disable_keep_alive: bool,
}

/// Body accepted by `PUT /configs`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct UpdateConfigRequest {
    pub path: String,
    pub payload: String,
}

impl UpdateConfigRequest {
    pub fn from_path(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            payload: String::new(),
        }
    }

    pub fn from_payload(payload: impl Into<String>) -> Self {
        Self {
            path: String::new(),
            payload: payload.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct UpdateConfigOptions {
    pub force: bool,
}

/// Body accepted by `PATCH /configs`. Every outer field maps to a Go pointer.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct ConfigPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socks_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redir_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tproxy_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixed_port: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tun: Option<TunPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tuic_server: Option<TuicServerPatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ss_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vmess_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcptun_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udptun_config: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_lan: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub skip_auth_prefixes: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub lan_allowed_ips: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub lan_disallowed_ips: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bind_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<TunnelMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tcp_concurrent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub find_process_mode: Option<FindProcessMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_name: Option<String>,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct TunPatch {
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<TunStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_hijack: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_route: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_detect_interface: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gso: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gso_max_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub inet6_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iproute2_table_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iproute2_rule_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_redirect: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_redirect_input_mark: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_redirect_output_mark: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_redirect_iproute2_fallback_rule_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loopback_address: Option<Vec<IpAddr>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict_route: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub route_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_address_set: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub route_exclude_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_exclude_address_set: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_interface: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_interface: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_uid: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_uid_range: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_uid: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_uid_range: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_android_user: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_package: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_package: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_mac_address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_mac_address: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_independent_nat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub udp_timeout: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icmp_timeout: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_descriptor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub inet4_route_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub inet6_route_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub inet4_route_exclude_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(type = Option<Vec<String>>)]
    pub inet6_route_exclude_address: Option<Vec<IpNet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recvmsgx: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sendmsgx: Option<bool>,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type,
)]
#[serde(default, rename_all = "kebab-case")]
pub struct TuicServerPatch {
    pub enable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listen: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub users: Option<IndexMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion_controller: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_idle_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication_timeout: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_udp_relay_packet_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwnd: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbr_profile: Option<String>,
}

impl Client {
    pub async fn configs(&self) -> Result<RuntimeConfig> {
        self.send_json(RequestMetadata::new("configs", Method::GET, true), || {
            self.get("/configs")
        })
        .await
    }

    pub async fn update_config(
        &self,
        request: &UpdateConfigRequest,
        options: UpdateConfigOptions,
    ) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("update_config", Method::PUT, false),
            || Ok(self.put("/configs")?.query(&options).json(request)),
        )
        .await
    }

    pub async fn patch_config(&self, patch: &ConfigPatch) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("patch_config", Method::PATCH, false),
            || Ok(self.patch("/configs")?.json(patch)),
        )
        .await
    }

    pub async fn update_geo_databases(&self) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("update_geo_databases", Method::POST, false),
            || self.post("/configs/geo"),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_patch_serializes_as_an_empty_object() {
        assert_eq!(
            serde_json::to_string(&ConfigPatch::default()).unwrap(),
            "{}"
        );
    }

    #[test]
    fn providing_tun_patch_also_sends_its_non_optional_enable_flag() {
        let patch = ConfigPatch {
            tun: Some(TunPatch::default()),
            ..ConfigPatch::default()
        };
        assert_eq!(
            serde_json::to_value(patch).unwrap(),
            serde_json::json!({"tun": {"enable": false}})
        );
    }

    #[test]
    fn runtime_config_without_optional_tunnel_configs_is_backward_compatible() {
        let value = serde_json::json!({
            "port": 0, "socks-port": 0, "redir-port": 0, "tproxy-port": 0,
            "mixed-port": 0, "tun": {}, "tuic-server": {}, "ss-config": "",
            "vmess-config": "", "authentication": null, "skip-auth-prefixes": null,
            "lan-allowed-ips": null, "lan-disallowed-ips": null, "allow-lan": false,
            "bind-address": "*", "inbound-tfo": false, "inbound-mptcp": false,
            "mode": "rule", "unified-delay": false, "log-level": "info", "ipv6": false,
            "interface-name": "", "routing-mark": 0, "geox-url": {},
            "geo-auto-update": false, "geo-update-interval": 0, "geodata-mode": false,
            "geodata-loader": "", "geosite-matcher": "", "tcp-concurrent": false,
            "find-process-mode": "off", "sniffing": false, "global-ua": "",
            "etag-support": false, "keep-alive-idle": 0, "keep-alive-interval": 0,
            "disable-keep-alive": false
        });
        let config: RuntimeConfig = serde_json::from_value(value).unwrap();
        assert_eq!(config.tcptun_config, None);
        assert_eq!(config.udptun_config, None);
    }
}
