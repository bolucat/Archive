use std::time::Duration;

use educe::Educe;
use serde::{Deserialize, Serialize};
use wind_core::StackPrefer;

/// Preset or custom DNS resolver for outbound traffic.
///
/// * `System` preserves the legacy behaviour (`tokio::net::lookup_host`, which
///   in turn uses the OS `getaddrinfo`).
/// * The named presets use Hickory DNS with the published resolver IPs for each
///   provider. `*Tls` variants speak DoT, `*Https` variants speak DoH.
/// * `Custom` reads [`DnsConfig::servers`].
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Educe)]
#[serde(rename_all = "kebab-case")]
#[educe(Default)]
pub enum DnsMode {
	// `System` is the default: it preserves the legacy behaviour documented on
	// `DnsConfig` ("Defaults to the OS resolver"). The default previously sat on
	// `CloudflareTls`, which silently rerouted every outbound lookup through
	// Cloudflare DoT for users who omitted the `[dns]` section -- a behaviour and
	// privacy change that also breaks environments where port 853 is blocked.
	#[educe(Default)]
	System,
	Cloudflare,
	CloudflareTls,
	CloudflareHttps,
	Google,
	GoogleTls,
	GoogleHttps,
	Quad9,
	Quad9Tls,
	Quad9Https,
	Custom,
}

/// DNS resolver configuration.
///
/// Defaults to the OS resolver, matching the pre-existing behaviour. When a
/// non-`System` mode is selected, outbound FQDN lookups are routed through a
/// Hickory DNS resolver built from these settings.
#[derive(Debug, Clone, Deserialize, Serialize, Educe)]
// `#[serde(default)]` on the container is required in addition to `educe`:
// educe only affects `Default::default()`, not serde. Without it, writing e.g.
// `[dns]\nmode = "google"` (omitting `stack_prefer`) fails with
// "missing field `stack_prefer`". The container default fills any omitted field
// from `Default::default()` while `deny_unknown_fields` still rejects typos.
#[serde(default, deny_unknown_fields)]
#[educe(Default)]
pub struct DnsConfig {
	/// Resolver mode. See [`DnsMode`].
	pub mode: DnsMode,

	/// Servers for `mode = "custom"`. Each entry is either a bare address or a
	/// URL-style specifier:
	///   * `1.1.1.1` or `1.1.1.1:53`        — UDP on port 53
	///   * `udp://1.1.1.1[:port]`
	///   * `tcp://1.1.1.1[:port]`
	///   * `tls://1.1.1.1[:port][#sni]`     — DoT (port defaults to 853)
	///   * `https://1.1.1.1[:port][#sni]`   — DoH (port defaults to 443)
	///
	/// IPv6 literals must be bracketed when a port is present, e.g.
	/// `tls://[2606:4700:4700::1111]:853#cloudflare-dns.com`.
	#[serde(default)]
	pub servers: Vec<String>,

	/// Per-query timeout. Defaults to the Hickory library default.
	#[serde(default, with = "humantime_serde::option")]
	pub timeout: Option<Duration>,

	/// Retry attempts per query. Defaults to the Hickory library default.
	#[serde(default)]
	pub attempts: Option<usize>,

	/// IP stack preference. Default: v4first (A first, then AAAA).
	#[educe(Default(expression = StackPrefer::V4first))]
	pub stack_prefer: StackPrefer,
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn partial_table_fills_missing_fields_from_default() {
		// Omitting `stack_prefer` (and everything but `mode`) must not fail with
		// "missing field"; the container `#[serde(default)]` fills the rest.
		let cfg: DnsConfig = toml::from_str("mode = \"google\"").unwrap();
		assert_eq!(cfg.mode, DnsMode::Google);
		assert_eq!(cfg.stack_prefer, StackPrefer::V4first);
		assert!(cfg.servers.is_empty());
	}

	#[test]
	fn empty_table_is_all_defaults() {
		let cfg: DnsConfig = toml::from_str("").unwrap();
		assert_eq!(cfg.mode, DnsMode::default());
		assert_eq!(cfg.stack_prefer, StackPrefer::V4first);
	}

	#[test]
	fn default_mode_is_system_matching_docs() {
		// The documented default is the OS resolver (backward compatibility);
		// it must not silently route lookups through Cloudflare DoT.
		assert_eq!(DnsMode::default(), DnsMode::System);
		assert_eq!(DnsConfig::default().mode, DnsMode::System);
	}

	#[test]
	fn unknown_field_is_still_rejected() {
		assert!(toml::from_str::<DnsConfig>("mode = \"system\"\nbogus = 1").is_err());
	}
}
