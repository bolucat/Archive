//! Fully-resolved runtime configuration.
//!
//! [`ResolvedConfig`] is the counterpart of [`PersistentConfig`]: the
//! persistent layer only deserializes user files, while this layer completes
//! the parse/validation work that `conf::runtime` previously deferred to the
//! plugin and `main`:
//!
//! - string fields become real types (`server_addr` → [`SocketAddr`],
//!   `strategy` → [`LoadBalanceStrategy`], `quic_congestion_control` →
//!   [`QuicCongestionControl`]);
//! - outbound and inbound tags are checked for duplicates;
//! - load-balance `proxies` references are validated and the load-balance
//!   groups are topologically sorted (and cycle-checked), so a group may
//!   reference a proxy declared anywhere in the file — not only earlier;
//! - the router's `default_tag` is fixed here, not recomputed downstream.
//!
//! All errors are propagated as [`eyre::Result`] with the offending tag in
//! the message; nothing panics on user input.

use std::{
	collections::{HashMap, HashSet, VecDeque},
	net::SocketAddr,
	sync::Arc,
	time::Duration,
};

use wind_base::load_balance::LoadBalanceStrategy;
use wind_core::{QuicCongestionControl, parse_congestion_control};
use wind_naive::NaiveOutboundOpts;
use wind_socks::inbound::{AuthMode, SocksInboundOpt};
use wind_tuic::quinn::outbound::TuicOutboundOpts;

use crate::conf::persistent::{AuthConfig, InboundConfig, OutboundConfig, PersistentConfig};

/// A fully-resolved configuration ready to be assembled by the plugin.
pub struct ResolvedConfig {
	pub inbounds: Vec<ResolvedInbound>,

	/// Outbounds in dependency order: every load-balance child appears before
	/// its parent group, so the plugin can build them in a single pass.
	pub outbounds: Vec<ResolvedOutbound>,

	/// Tag of the first declared outbound; the router forwards to it by
	/// default.
	pub default_tag: String,
}

/// One inbound protocol instance, with its tag extracted for logging/routing.
pub struct ResolvedInbound {
	pub tag: String,
	pub opts: ResolvedInboundOpts,
}

pub enum ResolvedInboundOpts {
	Socks(SocksInboundOpt),
}

/// One outbound protocol instance. The tag lives alongside the options
/// because the runtime options types (e.g. [`TuicOutboundOpts`]) do not carry
/// it themselves.
pub enum ResolvedOutbound {
	Tuic { tag: String, opts: TuicOutboundOpts },
	Naive { tag: String, opts: NaiveOutboundOpts },
	LoadBalance { tag: String, opts: ResolvedLoadBalance },
}

impl ResolvedOutbound {
	fn tag(&self) -> &str {
		match self {
			ResolvedOutbound::Tuic { tag, .. }
			| ResolvedOutbound::Naive { tag, .. }
			| ResolvedOutbound::LoadBalance { tag, .. } => tag,
		}
	}

	fn is_load_balance(&self) -> bool {
		matches!(self, ResolvedOutbound::LoadBalance { .. })
	}

	fn proxies(&self) -> &[String] {
		match self {
			ResolvedOutbound::LoadBalance { opts, .. } => &opts.proxies,
			_ => &[],
		}
	}
}

/// Resolved load-balance group: strategy is an enum, interval a [`Duration`].
pub struct ResolvedLoadBalance {
	/// Tags of child proxies; validated to exist and to be acyclic.
	pub proxies: Vec<String>,
	pub strategy: LoadBalanceStrategy,
	pub url: String,
	pub interval: Duration,
	pub lazy: bool,
}

impl TryFrom<PersistentConfig> for ResolvedConfig {
	type Error = eyre::Error;

	fn try_from(p: PersistentConfig) -> eyre::Result<Self> {
		let inbounds = resolve_inbounds(p.inbounds)?;
		let (outbounds, default_tag) = resolve_outbounds(p.outbounds)?;

		Ok(Self {
			inbounds,
			outbounds,
			default_tag,
		})
	}
}

fn resolve_inbounds(inbounds: Vec<InboundConfig>) -> eyre::Result<Vec<ResolvedInbound>> {
	let mut tags = HashSet::new();
	let mut resolved = Vec::with_capacity(inbounds.len());

	for ib in inbounds {
		let InboundConfig::Socks(s) = ib else {
			// Future inbound variants must be resolved here too; a hard
			// error beats a silent skip (or a compile error surfaced as a
			// pattern-matching failure far from the config).
			return Err(eyre::eyre!("unsupported inbound type in configuration"));
		};
		if !tags.insert(s.tag.clone()) {
			return Err(eyre::eyre!("duplicate inbound tag '{}'", s.tag));
		}
		resolved.push(ResolvedInbound {
			tag: s.tag.clone(),
			opts: ResolvedInboundOpts::Socks(SocksInboundOpt {
				listen_addr: s.listen_addr,
				public_addr: s.public_addr,
				auth: match s.auth {
					AuthConfig::NoAuth => AuthMode::NoAuth,
					AuthConfig::Password { username, password } => AuthMode::Password { username, password },
				},
				skip_auth: s.skip_auth,
				allow_udp: s.allow_udp,
				inbound_tag: Arc::from(s.tag.as_str()),
				hooks: Default::default(),
			}),
		});
	}

	Ok(resolved)
}

fn resolve_outbounds(outbounds: Vec<OutboundConfig>) -> eyre::Result<(Vec<ResolvedOutbound>, String)> {
	if outbounds.is_empty() {
		return Err(eyre::eyre!(
			"no outbounds configured; add at least one outbound so the router has a target"
		));
	}

	let mut tag_index: HashMap<String, usize> = HashMap::new();
	let mut resolved: Vec<ResolvedOutbound> = Vec::with_capacity(outbounds.len());

	for oc in outbounds {
		let ob = resolve_outbound(oc)?;
		if tag_index.insert(ob.tag().to_owned(), resolved.len()).is_some() {
			return Err(eyre::eyre!("duplicate outbound tag '{}'", ob.tag()));
		}
		resolved.push(ob);
	}

	// `outbounds` was rejected when empty above, so the first tag is always
	// available and preserves the "first declared outbound" default.
	let default_tag = resolved
		.first()
		.map(ResolvedOutbound::tag)
		.ok_or_else(|| eyre::eyre!("no outbounds configured"))?
		.to_owned();
	let ordered = order_outbounds(&resolved, &tag_index)?;

	// Move each outbound into its dependency-ordered slot.
	let mut items: Vec<Option<ResolvedOutbound>> = resolved.into_iter().map(Some).collect();
	let mut ordered_outbounds = Vec::with_capacity(items.len());
	for &i in &ordered {
		let item = items[i].take();
		let tag = item.as_ref().map(ResolvedOutbound::tag).unwrap_or("<unknown>").to_owned();
		let ob = item.ok_or_else(|| eyre::eyre!("internal error: outbound '{tag}' resolved twice"))?;
		ordered_outbounds.push(ob);
	}

	Ok((ordered_outbounds, default_tag))
}

/// Validate load-balance references and return the outbounds in topological
/// order: non-load-balance outbounds first (declaration order), then the
/// load-balance groups ordered so that every child appears before its parent.
fn order_outbounds(resolved: &[ResolvedOutbound], tag_index: &HashMap<String, usize>) -> eyre::Result<Vec<usize>> {
	let n = resolved.len();
	let is_lb: Vec<bool> = resolved.iter().map(ResolvedOutbound::is_load_balance).collect();
	let lb_indices: Vec<usize> = (0..n).filter(|&i| is_lb[i]).collect();

	// Every referenced proxy must exist.
	for &i in &lb_indices {
		for t in resolved[i].proxies() {
			if !tag_index.contains_key(t) {
				return Err(eyre::eyre!(
					"load-balance '{}' references unknown proxy '{t}'",
					resolved[i].tag()
				));
			}
		}
	}

	// Kahn's algorithm over the load-balance subgraph only: a group is
	// "ready" once every load-balance group it references has been placed.
	let lb_set: HashSet<usize> = lb_indices.iter().copied().collect();
	let mut indegree = vec![0usize; n];
	let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];
	for &i in &lb_indices {
		for t in resolved[i].proxies() {
			let j = tag_index[t];
			if lb_set.contains(&j) {
				indegree[i] += 1;
				dependents[j].push(i);
			}
		}
	}

	let mut queue: VecDeque<usize> = lb_indices.iter().copied().filter(|&i| indegree[i] == 0).collect();
	let mut lb_order = Vec::with_capacity(lb_indices.len());
	while let Some(i) = queue.pop_front() {
		lb_order.push(i);
		for &v in &dependents[i] {
			indegree[v] -= 1;
			if indegree[v] == 0 {
				queue.push_back(v);
			}
		}
	}

	if lb_order.len() != lb_indices.len() {
		let cyclic = lb_indices
			.iter()
			.copied()
			.find(|&i| indegree[i] > 0)
			.map(|i| resolved[i].tag().to_owned())
			.unwrap_or_else(|| "<unknown>".into());
		return Err(eyre::eyre!(
			"load-balance groups contain a dependency cycle (involving '{cyclic}'); a group cannot depend on itself, directly \
			 or indirectly"
		));
	}

	let mut order: Vec<usize> = (0..n).filter(|&i| !is_lb[i]).collect();
	order.extend(lb_order);
	Ok(order)
}

fn resolve_outbound(oc: OutboundConfig) -> eyre::Result<ResolvedOutbound> {
	match oc {
		OutboundConfig::Tuic(t) => {
			let peer_addr = t.server_addr.parse::<SocketAddr>().map_err(|e| {
				eyre::eyre!(
					"outbound '{}' has invalid server_addr {:?}: {e} (expected \"host:port\")",
					t.tag,
					t.server_addr
				)
			})?;
			Ok(ResolvedOutbound::Tuic {
				tag: t.tag,
				opts: TuicOutboundOpts {
					peer_addr,
					sni: t.sni,
					auth: (t.uuid, t.password.as_bytes().to_vec().into()),
					zero_rtt_handshake: t.zero_rtt_handshake,
					heartbeat: Duration::from_secs(t.heartbeat_secs),
					gc_interval: Duration::from_secs(t.gc_interval_secs),
					gc_lifetime: Duration::from_secs(t.gc_lifetime_secs),
					skip_cert_verify: t.skip_cert_verify,
					alpn: t.alpn,
					reconnect: Default::default(),
				},
			})
		}
		OutboundConfig::Naive(n) => {
			let quic_congestion_control = n
				.quic_congestion_control
				.as_deref()
				.map(parse_congestion_control)
				.transpose()
				.map_err(|e| eyre::eyre!("outbound '{}': {e}", n.tag))?
				.unwrap_or(QuicCongestionControl::Default);
			Ok(ResolvedOutbound::Naive {
				tag: n.tag,
				opts: NaiveOutboundOpts {
					server_address: n.server_address,
					server_name: n.server_name,
					username: n.username,
					password: n.password,
					concurrency: n.concurrency,
					quic_enabled: n.quic_enabled,
					quic_congestion_control,
					trusted_root_certificates: n.trusted_root_certificates,
					ech_enabled: n.ech_enabled,
					extra_headers: n.extra_headers,
					cronet_lib_path: n.cronet_lib_path,
				},
			})
		}
		OutboundConfig::LoadBalance(lb) => Ok(ResolvedOutbound::LoadBalance {
			tag: lb.tag,
			opts: ResolvedLoadBalance {
				proxies: lb.proxies,
				strategy: parse_strategy(&lb.strategy)?,
				url: lb.url,
				interval: Duration::from_secs(lb.interval),
				lazy: lb.lazy,
			},
		}),
	}
}

/// Parse a load-balance strategy name (case-insensitive), accepting the
/// documented names plus common aliases.
fn parse_strategy(s: &str) -> eyre::Result<LoadBalanceStrategy> {
	match s.to_ascii_lowercase().as_str() {
		"round-robin" | "round_robin" | "rr" => Ok(LoadBalanceStrategy::RoundRobin),
		"consistent-hashing" | "consistent_hashing" | "ch" => Ok(LoadBalanceStrategy::ConsistentHashing),
		"sticky-sessions" | "sticky_sessions" | "ss" => Ok(LoadBalanceStrategy::StickySessions),
		other => Err(eyre::eyre!(
			"unknown load-balance strategy '{other}'; expected one of: round-robin, consistent-hashing, sticky-sessions"
		)),
	}
}

#[cfg(test)]
mod tests {
	use std::net::SocketAddr;

	use wind_base::load_balance::LoadBalanceStrategy;

	use super::*;
	use crate::conf::persistent::{LoadBalanceConfig, NaiveOutboundConfig, SocksInboundConfig, TuicOutboundConfig};

	const UUID: &str = "c1e6dbe2-f417-4890-994c-9ee15b926597";

	fn socks(tag: &str) -> InboundConfig {
		InboundConfig::Socks(SocksInboundConfig {
			tag: tag.into(),
			listen_addr: "127.0.0.1:6666".parse().unwrap(),
			public_addr: None,
			auth: AuthConfig::NoAuth,
			skip_auth: false,
			allow_udp: true,
		})
	}

	fn tuic(tag: &str) -> OutboundConfig {
		OutboundConfig::Tuic(TuicOutboundConfig {
			tag: tag.into(),
			server_addr: "127.0.0.1:9443".into(),
			sni: "localhost".into(),
			uuid: UUID.parse().unwrap(),
			password: "test_passwd".into(),
			zero_rtt_handshake: false,
			heartbeat_secs: 10,
			gc_interval_secs: 20,
			gc_lifetime_secs: 20,
			skip_cert_verify: false,
			alpn: vec!["h3".into()],
		})
	}

	fn naive(tag: &str) -> OutboundConfig {
		OutboundConfig::Naive(NaiveOutboundConfig {
			tag: tag.into(),
			server_address: "127.0.0.1:7777".into(),
			server_name: None,
			username: None,
			password: None,
			concurrency: 1,
			quic_enabled: false,
			quic_congestion_control: None,
			trusted_root_certificates: None,
			ech_enabled: false,
			extra_headers: Default::default(),
			cronet_lib_path: None,
		})
	}

	fn lb(tag: &str, proxies: &[&str], strategy: &str) -> OutboundConfig {
		OutboundConfig::LoadBalance(LoadBalanceConfig {
			tag: tag.into(),
			proxies: proxies.iter().map(|p| p.to_string()).collect(),
			url: "https://www.gstatic.com/generate_204".into(),
			interval: 300,
			lazy: false,
			strategy: strategy.into(),
		})
	}

	fn config(inbounds: Vec<InboundConfig>, outbounds: Vec<OutboundConfig>) -> PersistentConfig {
		PersistentConfig { inbounds, outbounds }
	}

	fn resolve(p: PersistentConfig) -> eyre::Result<ResolvedConfig> {
		ResolvedConfig::try_from(p)
	}

	/// `Result::expect_err` requires `Ok` to be `Debug`, which the resolved
	/// types deliberately are not (they wrap runtime options).
	fn expect_err(p: PersistentConfig, msg: &str) -> eyre::Error {
		match resolve(p) {
			Ok(_) => panic!("{msg}"),
			Err(e) => e,
		}
	}

	fn outbound_tags(spec: &ResolvedConfig) -> Vec<&str> {
		spec.outbounds.iter().map(ResolvedOutbound::tag).collect()
	}

	#[test]
	fn resolves_minimal_config() {
		let spec = resolve(config(vec![socks("socks-in")], vec![tuic("tuic-out")])).expect("valid config");
		assert_eq!(spec.default_tag, "tuic-out");
		assert_eq!(spec.inbounds.len(), 1);
		assert_eq!(spec.inbounds[0].tag, "socks-in");
		let ResolvedOutbound::Tuic { tag, opts } = &spec.outbounds[0] else {
			panic!("expected tuic outbound");
		};
		assert_eq!(tag, "tuic-out");
		assert_eq!(opts.peer_addr, "127.0.0.1:9443".parse::<SocketAddr>().unwrap());
		assert_eq!(opts.heartbeat, Duration::from_secs(10));
		assert_eq!(opts.auth.0.to_string(), UUID);
	}

	#[test]
	fn rejects_invalid_server_addr() {
		let mut t = match tuic("bad") {
			OutboundConfig::Tuic(t) => t,
			_ => unreachable!(),
		};
		t.server_addr = "not-an-address".into();
		let err = expect_err(config(vec![], vec![OutboundConfig::Tuic(t)]), "invalid address must fail");
		let msg = format!("{err:#}");
		assert!(msg.contains("bad"), "error should name the tag: {msg}");
		assert!(msg.contains("server_addr"), "error should mention server_addr: {msg}");
	}

	#[test]
	fn rejects_invalid_strategy() {
		let err = expect_err(
			config(vec![], vec![lb("lb", &["main"], "bogus"), tuic("main")]),
			"unknown strategy must fail",
		);
		let msg = format!("{err:#}");
		assert!(msg.contains("unknown load-balance strategy 'bogus'"), "got: {msg}");
	}

	#[test]
	fn accepts_strategy_aliases() {
		for (raw, expected) in [
			("round-robin", LoadBalanceStrategy::RoundRobin),
			("round_robin", LoadBalanceStrategy::RoundRobin),
			("RR", LoadBalanceStrategy::RoundRobin),
			("consistent-hashing", LoadBalanceStrategy::ConsistentHashing),
			("ch", LoadBalanceStrategy::ConsistentHashing),
			("sticky-sessions", LoadBalanceStrategy::StickySessions),
			("ss", LoadBalanceStrategy::StickySessions),
		] {
			let spec = resolve(config(vec![], vec![lb("lb", &["main"], raw), tuic("main")]))
				.unwrap_or_else(|e| panic!("strategy {raw:?} must resolve: {e:#}"));
			let ResolvedOutbound::LoadBalance { opts, .. } = &spec.outbounds[1] else {
				panic!("expected load-balance at index 1");
			};
			assert_eq!(opts.strategy, expected, "strategy {raw:?}");
		}
	}

	#[test]
	fn rejects_invalid_congestion_control() {
		let mut n = match naive("n") {
			OutboundConfig::Naive(n) => n,
			_ => unreachable!(),
		};
		n.quic_congestion_control = Some("bogus".into());
		let err = expect_err(config(vec![], vec![OutboundConfig::Naive(n)]), "bad cc must fail");
		let msg = format!("{err:#}");
		assert!(msg.contains("unknown quic_congestion_control"), "got: {msg}");
		assert!(msg.contains("'n'"), "error should name the tag: {msg}");
	}

	#[test]
	fn cc_none_defaults_to_default() {
		let spec = resolve(config(vec![], vec![naive("n")])).expect("valid config");
		let ResolvedOutbound::Naive { opts, .. } = &spec.outbounds[0] else {
			panic!("expected naive outbound");
		};
		assert_eq!(opts.quic_congestion_control, QuicCongestionControl::Default);
	}

	#[test]
	fn rejects_duplicate_outbound_tags() {
		let err = expect_err(config(vec![], vec![tuic("dup"), naive("dup")]), "duplicate tag must fail");
		let msg = format!("{err:#}");
		assert!(msg.contains("duplicate outbound tag 'dup'"), "got: {msg}");
	}

	#[test]
	fn rejects_duplicate_inbound_tags() {
		let err = expect_err(
			config(vec![socks("dup"), socks("dup")], vec![tuic("main")]),
			"duplicate inbound tag must fail",
		);
		let msg = format!("{err:#}");
		assert!(msg.contains("duplicate inbound tag 'dup'"), "got: {msg}");
	}

	#[test]
	fn rejects_unknown_proxy_reference() {
		let err = expect_err(
			config(vec![], vec![lb("lb", &["ghost"], "rr"), tuic("main")]),
			"unknown proxy must fail",
		);
		let msg = format!("{err:#}");
		assert!(msg.contains("unknown proxy 'ghost'"), "got: {msg}");
		assert!(msg.contains("'lb'"), "error should name the group: {msg}");
	}

	#[test]
	fn rejects_dependency_cycle() {
		let err = expect_err(
			config(vec![], vec![lb("a", &["b"], "rr"), lb("b", &["a"], "rr")]),
			"cycle must fail",
		);
		let msg = format!("{err:#}");
		assert!(msg.contains("cycle"), "got: {msg}");
	}

	#[test]
	fn rejects_self_reference() {
		let err = expect_err(config(vec![], vec![lb("a", &["a"], "rr")]), "self-reference must fail");
		let msg = format!("{err:#}");
		assert!(msg.contains("cycle"), "got: {msg}");
	}

	#[test]
	fn supports_arbitrary_declaration_order() {
		// `a` (load-balance) references `b` (load-balance) declared *after*
		// it; resolution must still order children before parents.
		let spec = resolve(config(
			vec![],
			vec![lb("a", &["b", "main"], "rr"), tuic("main"), lb("b", &["main"], "rr")],
		))
		.expect("forward references must resolve");
		assert_eq!(outbound_tags(&spec), vec!["main", "b", "a"]);
		assert_eq!(spec.default_tag, "a", "default is the first declared outbound");
	}

	#[test]
	fn rejects_empty_outbounds() {
		let err = expect_err(config(vec![], vec![]), "empty outbounds must fail");
		let msg = format!("{err:#}");
		assert!(msg.contains("no outbounds configured"), "got: {msg}");
	}

	/// Load-balance serde defaults (strategy/url/interval) survive resolution,
	/// and a group may reference a proxy declared later in the file.
	#[test]
	fn load_balance_defaults_from_yaml() {
		let yaml = r#"
inbounds:
  - type: socks
    tag: s
    listen_addr: "127.0.0.1:6666"
outbounds:
  - type: load-balance
    tag: lb
    proxies: [main]
  - type: tuic
    tag: main
    server_addr: "127.0.0.1:9443"
    uuid: "c1e6dbe2-f417-4890-994c-9ee15b926597"
    password: "p"
"#;
		let p: PersistentConfig = serde_yaml::from_str(yaml).expect("parse yaml");
		let spec = resolve(p).expect("resolve yaml-derived config");
		assert_eq!(outbound_tags(&spec), vec!["main", "lb"], "topological order");
		let ResolvedOutbound::LoadBalance { opts, .. } = &spec.outbounds[1] else {
			panic!("expected load-balance at index 1");
		};
		assert_eq!(opts.strategy, LoadBalanceStrategy::ConsistentHashing);
		assert_eq!(opts.url, "https://www.gstatic.com/generate_204");
		assert_eq!(opts.interval, Duration::from_secs(300));
		assert!(!opts.lazy);
	}
}
