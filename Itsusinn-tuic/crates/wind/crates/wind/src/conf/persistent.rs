use std::{collections::HashMap, net::SocketAddr, path::PathBuf};

use figment::{
	Figment,
	providers::{Env, Format, Toml, Yaml},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Root configuration for Wind.
///
/// # Example (YAML)
///
/// ```yaml
/// inbounds:
///   - type: socks
///     tag: socks-in
///     listen_addr: "127.0.0.1:6666"
///
/// outbounds:
///   - type: tuic
///     tag: tuic-out
///     server_addr: "127.0.0.1:9443"
///     uuid: "c1e6dbe2-..."
///     password: "test_passwd"
/// ```
// Every struct in this module that is deserialized from an operator-written
// file is `deny_unknown_fields`. Without it a misspelled key is dropped along
// with the value the operator wrote: either the field falls back to its default
// (`server_adr` -> a silent default, `proxys` -> no children at all), or the
// error names the *missing* field (\"missing field `server_addr`\") instead of the
// key that was actually written, so the typo is never shown. Either way the
// process starts with a value nobody wrote and the failure surfaces far from its
// cause. The resolved (`resolved.rs`) and defaults-deriving types are unaffected:
// this attribute only changes deserialization, never serialization, so exporting
// a config and reloading it stays a round trip.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistentConfig {
	#[serde(default)]
	pub inbounds: Vec<InboundConfig>,

	#[serde(default)]
	pub outbounds: Vec<OutboundConfig>,
}

impl Default for PersistentConfig {
	fn default() -> Self {
		Self {
			inbounds: vec![InboundConfig::Socks(SocksInboundConfig {
				tag: "socks-in".into(),
				listen_addr: "127.0.0.1:6666".parse().unwrap(),
				public_addr: None,
				auth: AuthConfig::NoAuth,
				skip_auth: false,
				allow_udp: true,
			})],
			outbounds: vec![OutboundConfig::Tuic(TuicOutboundConfig {
				tag: "tuic-out".into(),
				server_addr: "127.0.0.1:9443".to_string(),
				sni: "localhost".into(),
				uuid: "c1e6dbe2-f417-4890-994c-9ee15b926597".parse().unwrap(),
				password: "test_passwd".into(),
				zero_rtt_handshake: false,
				heartbeat_secs: 10,
				gc_interval_secs: 20,
				gc_lifetime_secs: 20,
				skip_cert_verify: false,
				alpn: vec!["h3".into()],
			})],
		}
	}
}

/// One inbound protocol instance.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum InboundConfig {
	#[serde(rename = "socks")]
	Socks(SocksInboundConfig),
	// Future: Tuic(TuicInboundConfig), etc.
}

impl Default for InboundConfig {
	fn default() -> Self {
		InboundConfig::Socks(SocksInboundConfig::default())
	}
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SocksInboundConfig {
	/// Arbitrary name for this inbound (for logging / routing).
	pub tag: String,

	pub listen_addr: SocketAddr,

	#[serde(default)]
	pub public_addr: Option<std::net::IpAddr>,

	#[serde(default)]
	pub auth: AuthConfig,

	#[serde(default)]
	pub skip_auth: bool,

	#[serde(default = "default_true")]
	pub allow_udp: bool,
}

impl Default for SocksInboundConfig {
	fn default() -> Self {
		Self {
			tag: "socks-in".into(),
			listen_addr: "127.0.0.1:6666".parse().unwrap(),
			public_addr: None,
			auth: AuthConfig::NoAuth,
			skip_auth: false,
			allow_udp: true,
		}
	}
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub enum AuthConfig {
	#[default]
	NoAuth,
	Password {
		username: String,
		password: String,
	},
}

/// One outbound protocol instance.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum OutboundConfig {
	#[serde(rename = "tuic")]
	Tuic(TuicOutboundConfig),

	#[serde(rename = "naive")]
	Naive(NaiveOutboundConfig),

	#[serde(rename = "load-balance")]
	LoadBalance(LoadBalanceConfig),
}

/// Load-balance proxy group: distributes connections across multiple child
/// outbounds with optional health checks.
///
/// # Example (YAML)
///
/// ```yaml
/// outbounds:
///   - type: load-balance
///     tag: lb
///     proxies: [ss1, ss2, vmess1]
///     url: "https://www.gstatic.com/generate_204"
///     interval: 300
///     strategy: consistent-hashing
/// ```
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoadBalanceConfig {
	/// Tag (name) used by the router.
	pub tag: String,

	/// Tags of child proxies to balance across.
	#[serde(default)]
	pub proxies: Vec<String>,

	/// Health-check URL.
	#[serde(default = "default_health_url")]
	pub url: String,

	/// Health-check interval in seconds.
	#[serde(default = "default_300")]
	pub interval: u64,

	/// When `true`, health checks are deferred until first use.
	#[serde(default)]
	pub lazy: bool,

	/// Load-balancing strategy: `round-robin`, `consistent-hashing`, or
	/// `sticky-sessions`.
	#[serde(default = "default_strategy")]
	pub strategy: String,
}

fn default_health_url() -> String {
	"https://www.gstatic.com/generate_204".into()
}
fn default_300() -> u64 {
	300
}
fn default_strategy() -> String {
	"consistent-hashing".into()
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TuicOutboundConfig {
	/// Tag (name) used by the router to select this outbound.
	pub tag: String,

	/// Server address (host:port).
	pub server_addr: String,

	/// SNI override.
	#[serde(default = "default_localhost")]
	pub sni: String,

	/// Authentication UUID.
	pub uuid: Uuid,

	/// Authentication password.
	pub password: String,

	#[serde(default)]
	pub zero_rtt_handshake: bool,

	#[serde(default = "default_10")]
	pub heartbeat_secs: u64,

	#[serde(default = "default_20")]
	pub gc_interval_secs: u64,

	#[serde(default = "default_20")]
	pub gc_lifetime_secs: u64,

	/// Skip server certificate verification.
	///
	/// **WARNING**: This disables TLS authentication entirely and allows
	/// trivial MITM of the upstream relay. Defaults to `false` (verification
	/// enabled); must be set explicitly to opt out.
	#[serde(default)]
	pub skip_cert_verify: bool,

	#[serde(default = "default_h3_alpn")]
	pub alpn: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NaiveOutboundConfig {
	/// Tag (name) used by the router.
	pub tag: String,

	/// NaiveProxy server address (host:port).
	pub server_address: String,

	#[serde(default)]
	pub server_name: Option<String>,

	#[serde(default)]
	pub username: Option<String>,

	#[serde(default)]
	pub password: Option<String>,

	#[serde(default = "default_concurrency")]
	pub concurrency: u32,

	#[serde(default)]
	pub quic_enabled: bool,

	/// QUIC congestion-control algorithm: `default`, `bbr`, `bbrv2`, `cubic`,
	/// or `reno`. Only meaningful when `quic_enabled` is true.
	#[serde(default)]
	pub quic_congestion_control: Option<String>,

	#[serde(default)]
	pub trusted_root_certificates: Option<String>,

	#[serde(default)]
	pub ech_enabled: bool,

	#[serde(default)]
	pub extra_headers: HashMap<String, String>,

	#[serde(default)]
	pub cronet_lib_path: Option<String>,
}

fn default_true() -> bool {
	true
}
fn default_10() -> u64 {
	10
}
fn default_20() -> u64 {
	20
}
fn default_localhost() -> String {
	"localhost".into()
}
fn default_h3_alpn() -> Vec<String> {
	vec!["h3".into()]
}
fn default_concurrency() -> u32 {
	1
}

impl PersistentConfig {
	/// Write the default config to a file.
	pub fn export_to_file(&self, file_path: &PathBuf, format: &str) -> eyre::Result<()> {
		use std::{fs, io::Write};
		let content = match format.to_lowercase().as_str() {
			"yaml" => serde_yaml::to_string(&self)?,
			"toml" => toml::to_string_pretty(&self)?,
			_ => return Err(eyre::eyre!("Unsupported format: {format}")),
		};
		let mut file = fs::File::create(file_path)?;
		file.write_all(content.as_bytes())?;
		Ok(())
	}

	/// Load config from CLI args (file path / dir) + env vars.
	pub fn load(config_path: Option<String>, config_dir: Option<PathBuf>) -> eyre::Result<Self> {
		let mut figment = Figment::new();

		if let Some(dir) = config_dir {
			for fname in ["config.toml", "config.yaml"] {
				let p = dir.join(fname);
				if p.exists() {
					figment = if fname.ends_with(".toml") {
						figment.merge(Toml::file(p))
					} else {
						figment.merge(Yaml::file(p))
					};
				}
			}
		} else {
			for fname in &["config.toml", "config.yaml"] {
				let p = std::path::Path::new(fname);
				if p.exists() {
					figment = if fname.ends_with(".toml") {
						figment.merge(Toml::file(p))
					} else {
						figment.merge(Yaml::file(p))
					};
				}
			}
		}

		if let Some(path) = config_path {
			// Require a known extension instead of silently treating anything
			// non-yaml as TOML. Previously `foo.json` was happily fed to the
			// TOML parser; an unknown extension is almost certainly a typo or
			// a user error, and a clear early failure beats a cryptic
			// "expected `[section]` at line 1" two layers down.
			figment = if path.ends_with(".toml") {
				figment.merge(Toml::file(path))
			} else if path.ends_with(".yaml") || path.ends_with(".yml") {
				figment.merge(Yaml::file(path))
			} else {
				return Err(eyre::eyre!(
					"unsupported config extension for {path:?}: expected `.toml`, `.yaml`, or `.yml`"
				));
			};
		}

		figment = figment.merge(Env::prefixed("WIND_"));
		Ok(figment.extract()?)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The default `PersistentConfig` MUST NOT silently disable TLS certificate
	/// verification. Pre-PR1 the default was `true`, which gave any user that
	/// relied on the bundled defaults a fully MITM-able outbound. The fix is
	/// a one-character change to the default literal; this test pins it.
	#[test]
	fn default_skip_cert_verify_is_false() {
		let cfg = PersistentConfig::default();
		let OutboundConfig::Tuic(tuic) = cfg.outbounds.first().expect("default has a tuic outbound") else {
			panic!("default outbound is not the TUIC variant");
		};
		assert!(
			!tuic.skip_cert_verify,
			"default skip_cert_verify must be false — TLS cert verification MUST be on by default"
		);
	}

	/// Default ALPN is the historically-shipped `["h3"]` list; emptying it
	/// should require explicit configuration.
	#[test]
	fn default_alpn_is_h3() {
		let cfg = PersistentConfig::default();
		let OutboundConfig::Tuic(tuic) = cfg.outbounds.first().unwrap() else {
			unreachable!()
		};
		assert_eq!(tuic.alpn, vec![String::from("h3")]);
	}

	/// Omitting `skip_cert_verify` in a config file MUST deserialize to
	/// `false`, matching the documented default. Previously the field used
	/// `#[serde(default = "default_true")]`, which silently flipped the bit
	/// back to `true` even when the operator had removed it.
	#[test]
	fn omitted_skip_cert_verify_deserializes_false() {
		let yaml = r#"
type: tuic
tag: t
server_addr: "127.0.0.1:9443"
uuid: "c1e6dbe2-f417-4890-994c-9ee15b926597"
password: "p"
"#;
		let parsed: OutboundConfig = serde_yaml::from_str(yaml).expect("parse YAML outbound");
		let OutboundConfig::Tuic(t) = parsed else {
			panic!("expected tuic")
		};
		assert!(!t.skip_cert_verify, "omitted field must default to false");
	}

	/// A misspelled top-level key must be reported, not silently dropped.
	///
	/// Before the `deny_unknown_fields` fix this YAML deserialized happily into
	/// a config with **zero** outbounds: `outbunds` was discarded, the `tuic`
	/// entry inside it vanished with it, and the operator only found out much
	/// later (at resolution time, when the missing default outbound surfaced as
	/// an unrelated "no outbounds configured" error). Fail closed on the typo.
	#[test]
	fn misspelled_config_key_is_rejected() {
		let yaml = r#"
inbounds:
  - type: socks
    tag: socks-in
    listen_addr: "127.0.0.1:6666"
outbunds:
  - type: tuic
    tag: tuic-out
    server_addr: "127.0.0.1:9443"
    uuid: "c1e6dbe2-f417-4890-994c-9ee15b926597"
    password: "test_passwd"
"#;
		let err = serde_yaml::from_str::<PersistentConfig>(yaml)
			.expect_err("`outbunds` is not a known key and must be rejected instead of ignored");
		let msg = format!("{err}");
		assert!(
			msg.contains("outbunds"),
			"the error must name the offending key so the typo is obvious, got: {msg}"
		);
	}

	/// The same fail-closed rule has to hold for a typo **inside** an entry.
	/// Misspelling a *required* field (`server_adr`) is reported, but badly:
	/// the old error was `missing field `server_addr``, which never names the
	/// key the operator actually wrote, so the typo is hidden behind a phrasing
	/// that reads like the field was absent on purpose.
	#[test]
	fn misspelled_outbound_field_is_rejected() {
		let yaml = r#"
outbounds:
  - type: tuic
    tag: tuic-out
    server_adr: "127.0.0.1:9443"
    uuid: "c1e6dbe2-f417-4890-994c-9ee15b926597"
    password: "test_passwd"
"#;
		let err = serde_yaml::from_str::<PersistentConfig>(yaml)
			.expect_err("`server_adr` is not a known TUIC field and must be rejected");
		let msg = format!("{err}");
		assert!(msg.contains("server_adr"), "expected the key to be named, got: {msg}");
	}

	/// Same rule for the inbound side and for the load-balance group, which
	/// has the largest typo surface of the three outbound kinds.
	///
	/// The group case is the genuinely **silent** one: `proxies` is
	/// `#[serde(default)]`, so before the fix `proxys: [a]` deserialized into a
	/// group with no children at all — no error until the router later failed
	/// to find a proxy the operator thought they had configured.
	#[test]
	fn misspelled_nested_fields_are_rejected() {
		let inbound_typo = r#"
inbounds:
  - type: socks
    tag: socks-in
    listen_adr: "127.0.0.1:6666"
"#;
		let err = serde_yaml::from_str::<PersistentConfig>(inbound_typo)
			.expect_err("`listen_adr` is not a known SOCKS inbound field");
		assert!(
			format!("{err}").contains("listen_adr"),
			"expected the key to be named, got: {err}"
		);

		let group_typo = r#"
outbounds:
  - type: load-balance
    tag: lb
    proxys: [a]
"#;
		let err = serde_yaml::from_str::<PersistentConfig>(group_typo).expect_err("`proxys` is not a known load-balance field");
		assert!(
			format!("{err}").contains("proxys"),
			"expected the key to be named, got: {err}"
		);
	}

	/// `PersistentConfig::load` is the path `wind` actually uses; a typo in a
	/// TOML file must fail there too (and must not be masked by the default
	/// configuration or by the `WIND_` environment provider).
	#[test]
	fn misspelled_toml_key_is_rejected_by_load() {
		let path = std::env::temp_dir().join(format!("wind-w59-unknown-key-{}.toml", std::process::id()));
		let toml = r#"
[[inbounds]]
type = "socks"
tag = "socks-in"
listen_adr = "127.0.0.1:6666"

[[outbounds]]
type = "tuic"
tag = "tuic-out"
server_addr = "127.0.0.1:9443"
uuid = "c1e6dbe2-f417-4890-994c-9ee15b926597"
password = "test_passwd"
"#;
		std::fs::write(&path, toml).expect("write temporary config");
		let result = PersistentConfig::load(Some(path.to_string_lossy().into_owned()), None);
		let _ = std::fs::remove_file(&path);
		let err = result.expect_err("a misspelled TOML key must make loading fail");
		let msg = format!("{err:#}");
		assert!(msg.contains("listen_adr"), "expected the key to be named, got: {msg}");
	}

	/// The fix must not reject anything that is actually valid: the default
	/// config round-trips through YAML and TOML, and every documented field of
	/// all three outbound kinds still parses.
	#[test]
	fn valid_config_is_still_accepted() {
		let default = PersistentConfig::default();
		let yaml = serde_yaml::to_string(&default).expect("serialize default to YAML");
		let parsed: PersistentConfig = serde_yaml::from_str(&yaml).expect("the default config must still round-trip");
		assert_eq!(parsed.inbounds.len(), default.inbounds.len());
		assert_eq!(parsed.outbounds.len(), default.outbounds.len());

		let full = r#"
inbounds:
  - type: socks
    tag: socks-in
    listen_addr: "127.0.0.1:6666"
    public_addr: "127.0.0.1"
    auth:
      Password:
        username: u
        password: p
    skip_auth: false
    allow_udp: true
outbounds:
  - type: tuic
    tag: tuic-out
    server_addr: "127.0.0.1:9443"
    sni: localhost
    uuid: "c1e6dbe2-f417-4890-994c-9ee15b926597"
    password: "test_passwd"
    zero_rtt_handshake: true
    heartbeat_secs: 10
    gc_interval_secs: 20
    gc_lifetime_secs: 20
    skip_cert_verify: false
    alpn: [h3]
  - type: naive
    tag: naive-out
    server_address: "127.0.0.1:7777"
    server_name: localhost
    username: u
    password: p
    concurrency: 1
    quic_enabled: false
    quic_congestion_control: bbr
    trusted_root_certificates: /tmp/ca.pem
    ech_enabled: false
    extra_headers:
      X-Test: "1"
    cronet_lib_path: /tmp/libcronet.so
  - type: load-balance
    tag: lb
    proxies: [tuic-out, naive-out]
    url: "https://www.gstatic.com/generate_204"
    interval: 300
    lazy: false
    strategy: consistent-hashing
"#;
		let parsed: PersistentConfig = serde_yaml::from_str(full).expect("every documented field must still deserialize");
		assert_eq!(parsed.inbounds.len(), 1);
		assert_eq!(parsed.outbounds.len(), 3);
	}
	/// PR4-M: unknown extensions used to be silently routed to the TOML
	/// parser, producing cryptic "expected `[section]`" errors when the file
	/// was actually JSON or had a typo. Now they fail loudly up front.
	#[test]
	fn unknown_config_extension_rejected() {
		let err = PersistentConfig::load(Some("/tmp/wind-pr4-bogus.json".into()), None)
			.expect_err("`.json` is not supported and must be rejected");
		let msg = format!("{err:#}");
		assert!(
			msg.contains("unsupported config extension"),
			"expected the loader to mention the extension, got: {msg}"
		);
	}
}
