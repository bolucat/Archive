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
#[derive(Debug, Deserialize, Serialize)]
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
}

#[derive(Debug, Deserialize, Serialize)]
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
