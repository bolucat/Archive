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
    pub memory: Option<u64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_chains: Option<Vec<String>>,
    pub rule: String,
    pub rule_payload: String,
    #[serde(flatten)]
    pub extra: indexmap::IndexMap<String, serde_json::Value>,
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
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct ConnectionMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<crate::ConfigEnum<ConnectionNetwork>>,
    #[serde(rename = "type")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_type: Option<crate::ConfigEnum<ConnectionType>>,
    #[serde(rename = "sourceIP")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,
    #[serde(rename = "destinationIP")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_ip: Option<String>,
    #[serde(rename = "sourceGeoIP")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_geo_ip: Option<Vec<String>>,
    #[serde(rename = "destinationGeoIP")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_geo_ip: Option<Vec<String>>,
    #[serde(rename = "sourceIPASN")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ip_asn: Option<String>,
    #[serde(rename = "destinationIPASN")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_ip_asn: Option<String>,
    #[serde(rename = "sourcePort", with = "optional_port")]
    #[specta(type = Option<String>)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_port: Option<u16>,
    #[serde(rename = "destinationPort", with = "optional_port")]
    #[specta(type = Option<String>)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_port: Option<u16>,
    #[serde(rename = "inboundIP")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound_ip: Option<String>,
    #[serde(rename = "inboundPort", with = "optional_port")]
    #[specta(type = Option<String>)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound_port: Option<u16>,
    #[serde(rename = "inboundName")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound_name: Option<String>,
    #[serde(rename = "inboundUser")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound_user: Option<String>,
    #[serde(rename = "rematchName")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rematch_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(rename = "dnsMode")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_mode: Option<crate::ConfigEnum<DnsMode>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(rename = "processPath")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_path: Option<String>,
    #[serde(rename = "specialProxy")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_proxy: Option<String>,
    #[serde(rename = "specialRules")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_rules: Option<String>,
    #[serde(rename = "remoteDestination")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_destination: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dscp: Option<u8>,
    #[serde(rename = "sniffHost")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sniff_host: Option<String>,
    #[serde(flatten)]
    pub extra: indexmap::IndexMap<String, serde_json::Value>,
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

    /// Open the sampled, typed connections WebSocket.
    pub async fn connections_ws(
        &self,
        query: ConnectionStreamQuery,
    ) -> Result<crate::WebSocketStream<ConnectionsSnapshot>> {
        let interval = query.milliseconds().to_string();
        self.websocket(
            RequestMetadata::new("connections_ws", Method::GET, true),
            || Ok(self.get("/connections")?.query(&[("interval", &interval)])),
        )
        .await
        .map(|socket| crate::WebSocketStream::new(socket, "connections_ws"))
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

mod optional_port {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
    pub fn serialize<S: Serializer>(value: &Option<u16>, serializer: S) -> Result<S::Ok, S::Error> {
        value.map(|port| port.to_string()).serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u16>, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Port {
            Text(String),
            Number(u16),
        }
        Option::<Port>::deserialize(deserializer)?
            .map(|port| match port {
                Port::Text(text) => text.parse().map_err(D::Error::custom),
                Port::Number(port) => Ok(port),
            })
            .transpose()
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

        assert_eq!(metadata.source_port, Some(1234));
        assert_eq!(metadata.destination_port, Some(443));
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
