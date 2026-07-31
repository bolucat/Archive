use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use reqwest::Method;
use uuid::Uuid;

use crate::{Client, Error, Result, retry::RequestMetadata};

/// Snapshot returned by `GET /connections` and each WebSocket frame.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionsSnapshot {
    pub download_total: i64,
    pub upload_total: i64,
    /// Mihomo serializes this field as `null` when no connections exist.
    pub connections: Option<Vec<Connection>>,
    pub memory: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: Uuid,
    pub metadata: Option<ConnectionMetadata>,
    pub upload: i64,
    pub download: i64,
    pub start: DateTime<FixedOffset>,
    pub chains: Vec<String>,
    pub provider_chains: Vec<String>,
    pub rule: String,
    pub rule_payload: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionNetwork {
    Tcp,
    Udp,
    All,
    Invalid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum ConnectionType {
    #[serde(rename = "HTTP")]
    Http,
    #[serde(rename = "HTTPS")]
    Https,
    #[serde(rename = "Socks4")]
    Socks4,
    #[serde(rename = "Socks5")]
    Socks5,
    #[serde(rename = "ShadowSocks")]
    ShadowSocks,
    Snell,
    #[serde(rename = "Vmess")]
    Vmess,
    #[serde(rename = "Vless")]
    Vless,
    Redir,
    TProxy,
    Trojan,
    Tunnel,
    Tun,
    Tuic,
    Hysteria2,
    #[serde(rename = "AnyTLS")]
    AnyTls,
    Mieru,
    Sudoku,
    TrustTunnel,
    ShadowQuic,
    Inner,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum DnsMode {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "fake-ip")]
    FakeIp,
    #[serde(rename = "redir-host")]
    RedirHost,
    #[serde(rename = "hosts")]
    Hosts,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct ConnectionMetadata {
    pub network: ConnectionNetwork,
    #[serde(rename = "type")]
    pub connection_type: ConnectionType,
    #[serde(rename = "sourceIP")]
    pub source_ip: String,
    #[serde(rename = "destinationIP")]
    pub destination_ip: String,
    #[serde(rename = "sourceGeoIP")]
    pub source_geo_ip: Option<Vec<String>>,
    #[serde(rename = "destinationGeoIP")]
    pub destination_geo_ip: Option<Vec<String>>,
    #[serde(rename = "sourceIPASN")]
    pub source_ip_asn: String,
    #[serde(rename = "destinationIPASN")]
    pub destination_ip_asn: String,
    #[serde(rename = "sourcePort", with = "string_u16")]
    #[specta(type = String)]
    pub source_port: u16,
    #[serde(rename = "destinationPort", with = "string_u16")]
    #[specta(type = String)]
    pub destination_port: u16,
    #[serde(rename = "inboundIP")]
    pub inbound_ip: String,
    #[serde(rename = "inboundPort", with = "string_u16")]
    #[specta(type = String)]
    pub inbound_port: u16,
    #[serde(rename = "inboundName")]
    pub inbound_name: String,
    #[serde(rename = "inboundUser")]
    pub inbound_user: String,
    #[serde(rename = "rematchName")]
    pub rematch_name: String,
    pub host: String,
    #[serde(rename = "dnsMode")]
    pub dns_mode: DnsMode,
    pub uid: u32,
    pub process: String,
    #[serde(rename = "processPath")]
    pub process_path: String,
    #[serde(rename = "specialProxy")]
    pub special_proxy: String,
    #[serde(rename = "specialRules")]
    pub special_rules: String,
    #[serde(rename = "remoteDestination")]
    pub remote_destination: String,
    pub dscp: u8,
    #[serde(rename = "sniffHost")]
    pub sniff_host: String,
}

/// WebSocket sampling interval. Mihomo interprets it as decimal milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, specta::Type)]
pub struct ConnectionStreamQuery {
    interval: Duration,
}

impl ConnectionStreamQuery {
    pub fn new(interval: Duration) -> Result<Self> {
        if interval.as_millis() == 0 {
            return Err(Error::InvalidArgument {
                argument: "interval",
                message: "must be at least one millisecond".to_owned(),
            });
        }
        if interval.as_millis() > isize::MAX as u128 {
            return Err(Error::InvalidArgument {
                argument: "interval",
                message: "does not fit Mihomo's millisecond query".to_owned(),
            });
        }
        Ok(Self { interval })
    }

    pub const fn interval(self) -> Duration {
        self.interval
    }

    fn milliseconds(self) -> u128 {
        self.interval.as_millis()
    }
}

impl Default for ConnectionStreamQuery {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(1),
        }
    }
}

impl Client {
    /// Fetch the one-shot HTTP connections snapshot.
    pub async fn connections(&self) -> Result<ConnectionsSnapshot> {
        self.send_json(
            RequestMetadata::new("connections", Method::GET, true),
            || self.get("/connections"),
        )
        .await
    }

    /// Open the sampled connections WebSocket and return it without wrapping.
    pub async fn connections_ws(
        &self,
        query: ConnectionStreamQuery,
    ) -> Result<reqwest_websocket::WebSocket> {
        let interval = query.milliseconds().to_string();
        self.websocket(
            RequestMetadata::new("connections_ws", Method::GET, true),
            || Ok(self.get("/connections")?.query(&[("interval", &interval)])),
        )
        .await
    }

    /// Close a connection. Mihomo also returns success when the id is absent.
    pub async fn close_connection(&self, id: Uuid) -> Result<()> {
        let url = self.endpoint_with_segments("/connections", [id.to_string()])?;
        self.send_empty(
            RequestMetadata::new("close_connection", Method::DELETE, true),
            || Ok(self.request_url(Method::DELETE, url.clone())),
        )
        .await
    }

    /// Close every current connection. This is intentionally not auto-retried.
    pub async fn close_all_connections(&self) -> Result<()> {
        self.send_empty(
            RequestMetadata::new("close_all_connections", Method::DELETE, false),
            || self.delete("/connections"),
        )
        .await
    }
}

mod string_u16 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub fn serialize<S>(value: &u16, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u16, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_ports_follow_gos_string_encoding() {
        let metadata: ConnectionMetadata = serde_json::from_str(
            r#"{
                "network":"tcp","type":"HTTP",
                "sourceIP":"127.0.0.1","destinationIP":"1.1.1.1",
                "sourceGeoIP":null,"destinationGeoIP":[],
                "sourceIPASN":"","destinationIPASN":"AS13335",
                "sourcePort":"1234","destinationPort":"443",
                "inboundIP":"127.0.0.1","inboundPort":"7890",
                "inboundName":"mixed","inboundUser":"","rematchName":"",
                "host":"example.com","dnsMode":"normal","uid":0,
                "process":"","processPath":"","specialProxy":"",
                "specialRules":"","remoteDestination":"","dscp":0,"sniffHost":""
            }"#,
        )
        .unwrap();

        assert_eq!(metadata.source_port, 1234);
        assert_eq!(metadata.destination_port, 443);
        assert_eq!(metadata.source_geo_ip, None);
    }

    #[test]
    fn zero_connection_interval_is_rejected_before_the_go_ticker_can_panic() {
        assert!(matches!(
            ConnectionStreamQuery::new(Duration::ZERO),
            Err(Error::InvalidArgument {
                argument: "interval",
                ..
            })
        ));
    }
}
