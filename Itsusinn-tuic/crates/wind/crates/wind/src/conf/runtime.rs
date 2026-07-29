use wind_socks::inbound::{AuthMode, SocksInboundOpt};
pub use wind_tuic::quinn::outbound::TuicOutboundOpts;

use crate::conf::persistent::{AuthConfig, InboundConfig, OutboundConfig, PersistentConfig};

/// A fully-resolved inbound ready to be started.
pub struct InboundRuntime {
	pub tag: String,
	pub opts: InboundOpts,
}

pub enum InboundOpts {
	Socks(SocksInboundOpt),
}

impl InboundRuntime {
	pub fn from_config(cfg: &InboundConfig) -> Self {
		match cfg {
			InboundConfig::Socks(s) => InboundRuntime {
				tag: s.tag.clone(),
				opts: InboundOpts::Socks(SocksInboundOpt {
					listen_addr: s.listen_addr,
					public_addr: s.public_addr,
					auth: match &s.auth {
						AuthConfig::NoAuth => AuthMode::NoAuth,
						AuthConfig::Password { username, password } => AuthMode::Password {
							username: username.clone(),
							password: password.clone(),
						},
					},
					skip_auth: s.skip_auth,
					allow_udp: s.allow_udp,
					hooks: Default::default(),
				}),
			},
		}
	}
}

/// A fully-resolved outbound ready to be started.
pub struct OutboundRuntime {
	pub tag: String,
	pub opts: OutboundOpts,
}

pub enum OutboundOpts {
	Tuic(TuicOutboundOpts),
	Naive(wind_naive::NaiveOutboundOpts),
}

impl OutboundRuntime {
	pub fn from_config(cfg: &OutboundConfig) -> Self {
		match cfg {
			OutboundConfig::Tuic(t) => {
				let addr = parse_socket_addr(&t.server_addr);
				OutboundRuntime {
					tag: t.tag.clone(),
					opts: OutboundOpts::Tuic(TuicOutboundOpts {
						peer_addr: addr,
						sni: t.sni.clone(),
						auth: (t.uuid, t.password.as_bytes().to_vec().into()),
						zero_rtt_handshake: t.zero_rtt_handshake,
						heartbeat: std::time::Duration::from_secs(t.heartbeat_secs),
						gc_interval: std::time::Duration::from_secs(t.gc_interval_secs),
						gc_lifetime: std::time::Duration::from_secs(t.gc_lifetime_secs),
						skip_cert_verify: t.skip_cert_verify,
						alpn: t.alpn.clone(),
						reconnect: wind_tuic::quinn::outbound::ReconnectConfig::default(),
					}),
				}
			}
			OutboundConfig::Naive(n) => OutboundRuntime {
				tag: n.tag.clone(),
				opts: OutboundOpts::Naive(wind_naive::NaiveOutboundOpts {
					server_address: n.server_address.clone(),
					server_name: n.server_name.clone(),
					username: n.username.clone(),
					password: n.password.clone(),
					concurrency: n.concurrency,
					quic_enabled: n.quic_enabled,
					quic_congestion_control: n
						.quic_congestion_control
						.as_deref()
						.map(|s| wind_core::parse_congestion_control(s).expect("invalid quic_congestion_control in config"))
						.unwrap_or(wind_core::QuicCongestionControl::Default),
					trusted_root_certificates: n.trusted_root_certificates.clone(),
					ech_enabled: n.ech_enabled,
					extra_headers: n.extra_headers.clone(),
					cronet_lib_path: n.cronet_lib_path.clone(),
				}),
			},
		}
	}
}

pub struct Config {
	pub inbounds: Vec<InboundRuntime>,
	pub outbounds: Vec<OutboundRuntime>,
}

impl Config {
	pub fn from_persist(p: PersistentConfig) -> Self {
		Self {
			inbounds: p.inbounds.iter().map(InboundRuntime::from_config).collect(),
			outbounds: p.outbounds.iter().map(OutboundRuntime::from_config).collect(),
		}
	}
}

fn parse_socket_addr(s: &str) -> std::net::SocketAddr {
	s.parse()
		.expect("server_addr must be a valid socket address (e.g. \"127.0.0.1:9443\")")
}
