pub mod api;
pub mod client;
pub mod error;
pub mod retry;
pub mod stream;

pub use api::{
    BrutalOptions, Bytes, BytesPerSecond, ConfigPatch, Connection, ConnectionMetadata,
    ConnectionNetwork, ConnectionStreamQuery, ConnectionType, ConnectionsSnapshot, Delay,
    DelayHistory, DelayQuery, DnsMode, DnsQuery, DnsQuestion, DnsRecord, DnsRecordType,
    DnsResponse, ExpectedStatus, FindProcessMode, GeoUrls, Hello, LogEntry, LogField, LogLevel,
    LogQuery, Memory, MuxOptions, ProviderName, ProviderType, Proxy, ProxyExtra, ProxyName,
    ProxyProvider, Rule, RuleExtra, RuleFormat, RulePatch, RuleProvider, RuleProviderBehavior,
    RuleProviderName, RuntimeConfig, RuntimeTuicServer, RuntimeTun, StatusResponse, StorageKey,
    StructuredLogEntry, StructuredLogLevel, SubscriptionInfo, Traffic, TuicServerPatch, TunPatch,
    TunStack, TunnelMode, UpdateConfigOptions, UpdateConfigRequest, UpgradeOptions, VehicleType,
    Version,
};
pub use client::{Client, ClientBuilder, ControllerEndpoint, Host, Secret};
pub use error::{Error, ErrorBody, Result};
pub use indexmap::IndexMap;
pub use retry::{ExponentialRetry, NoRetry, RequestMetadata, RetryPolicy};
pub use stream::HttpStream;

// TODO(ws-typed-stream): consider an opt-in typed frame adapter after raw
// WebSocket callers have migrated. The endpoint methods intentionally return
// `reqwest_websocket::WebSocket` directly in this implementation.
