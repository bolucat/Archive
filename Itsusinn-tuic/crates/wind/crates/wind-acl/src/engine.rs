//! The [`AclEngine`] router and its [`AclEngineBuilder`], backed by the
//! [`Ruleset`](crate::Ruleset) IR.
//!
//! The builder collects Clash/Mihomo rules and real Hysteria 2 (apernet) ACL
//! rules (the latter converted via [`apernet::acl_to_rules`]), lowers them
//! through the degenerate embedding ([`Ruleset::from_rules`]), and runs the
//! order-preserving optimizer ([`compile`]). The resulting engine implements
//! [`wind_core::Router`] and routes by building a `MatchContext` and calling
//! [`Ruleset::route`] — so its decisions are identical to evaluating the rules
//! first-match-wins, with apernet-derived rules taking precedence over Clash.

use std::{net::IpAddr, sync::Arc};

use tracing::Instrument as _;
use wind_base::resolve::resolve_target;
use wind_core::{
	RouteAction, Router, is_private_ip,
	resolve::Resolver,
	rule::{InboundType, MatchContext, NetworkType, Rule, RuleType},
	types::TargetAddr,
};
use wind_geodata::GeoData;

use crate::{
	Ruleset, compile,
	syntax::{
		apernet::{self, AclRule},
		metacubex,
	},
};

/// Loopback / private-range guards applied *before* rule evaluation.
///
/// When any guard is enabled the engine resolves the destination to an IP and
/// rejects connections to loopback / private space. Building an engine with a
/// guard enabled but no resolver is an error.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct GuardConfig {
	/// Reject destinations that resolve to a loopback address (`127.0.0.0/8`,
	/// `::1`).
	pub drop_loopback: bool,
	/// Reject destinations that resolve to RFC1918 / link-local / ULA space.
	pub drop_private: bool,
}

impl GuardConfig {
	#[inline]
	fn enabled(&self) -> bool {
		self.drop_loopback || self.drop_private
	}
}

/// A protocol-agnostic ACL / routing engine backed by the [`Ruleset`] IR.
///
/// Construct one via [`AclEngine::builder`]. It implements [`Router`], so it
/// can be handed directly to [`wind_core::Dispatcher::new`] or
/// [`wind_core::App::set_router`].
pub struct AclEngine {
	/// Compiled IR. apernet-converted rules precede Clash rules in source
	/// order, so first-match-wins gives them precedence.
	ruleset: Ruleset,
	guards: GuardConfig,
	/// Required whenever `guards.enabled()`. Validated at build time.
	resolver: Option<Arc<dyn Resolver>>,
	inbound_name: Option<String>,
	inbound_type: Option<InboundType>,
	/// GeoIP / GeoSite database. When present, `GEOIP` / `GEOSITE` rules match
	/// against it; when absent those rules can never match (and a warning is
	/// emitted at build time if any are present).
	geodata: Option<Arc<GeoData>>,
}

impl AclEngine {
	/// Start building an engine that falls back to `default_outbound` when no
	/// rule matches.
	pub fn builder(default_outbound: impl Into<String>) -> AclEngineBuilder {
		AclEngineBuilder {
			default_outbound: default_outbound.into(),
			apernet: Vec::new(),
			clash: Vec::new(),
			raw: Vec::new(),
			guards: GuardConfig::default(),
			resolver: None,
			inbound_name: None,
			inbound_type: None,
			geodata: None,
			hijack_seen: false,
		}
	}

	async fn do_route(&self, target: &TargetAddr, is_tcp: bool) -> eyre::Result<RouteAction> {
		// 1. Guards — resolve the destination and drop loopback / private space.
		if self.guards.enabled() {
			let resolver = self
				.resolver
				.as_ref()
				.expect("guards enabled without a resolver (should be rejected at build time)");
			let resolved = resolve_target(target, resolver.as_ref()).await?;
			if self.guards.drop_loopback && resolved.ip().is_loopback() {
				tracing::debug!(resolved = %resolved, "dropping loopback connection");
				return Ok(RouteAction::Reject(format!("loopback address rejected: {resolved}")));
			}
			if self.guards.drop_private && is_private_ip(&resolved.ip()) {
				tracing::debug!(resolved = %resolved, "dropping private-range connection");
				return Ok(RouteAction::Reject(format!("private address rejected: {resolved}")));
			}
		}

		// 2. Build the match context from what `route` can see. `src_ip` and
		// `inbound_user` are intentionally left `None` — the route signature
		// carries neither.
		let (domain, dst_ip, port) = match target {
			TargetAddr::Domain(d, p) => (Some(d.as_str()), None, *p),
			TargetAddr::IPv4(ip, p) => (None, Some(IpAddr::V4(*ip)), *p),
			TargetAddr::IPv6(ip, p) => (None, Some(IpAddr::V6(*ip)), *p),
		};

		// Bind the geodata lookup closures to locals so their borrows of
		// `self.geodata` outlive the `MatchContext` that references them. Without
		// this wiring `GEOIP` / `GEOSITE` rules would silently never match.
		let geoip_fn = self.geodata.as_ref().map(|gd| gd.geoip_lookup());
		let geosite_fn = self.geodata.as_ref().map(|gd| gd.geosite_lookup());

		let ctx = MatchContext {
			domain,
			dst_ip,
			dst_port: Some(port),
			network: Some(if is_tcp { NetworkType::Tcp } else { NetworkType::Udp }),
			inbound_name: self.inbound_name.as_deref(),
			inbound_type: self.inbound_type,
			geoip_lookup: geoip_fn.as_ref().map(|f| f as &dyn Fn(&str, IpAddr) -> bool),
			geosite_lookup: geosite_fn.as_ref().map(|f| f as &dyn Fn(&str, &str) -> bool),
			..Default::default()
		};

		// 3. The IR evaluates first-match-wins and falls back to the default
		// outbound policy. It is infallible.
		Ok(self.ruleset.route(&ctx))
	}
}

impl Router for AclEngine {
	async fn route(&self, target: &TargetAddr, is_tcp: bool) -> eyre::Result<RouteAction> {
		let span = tracing::debug_span!("acl_route", target = %target, proto = if is_tcp { "tcp" } else { "udp" });
		self.do_route(target, is_tcp).instrument(span).await
	}
}

/// Builder for [`AclEngine`]. See [`AclEngine::builder`].
pub struct AclEngineBuilder {
	default_outbound: String,
	apernet: Vec<Rule>,
	clash: Vec<Rule>,
	/// Pre-converted `wind_core` rules supplied directly (e.g. a host's legacy
	/// ACL already lowered to wind rules). Evaluated between apernet and clash.
	raw: Vec<Rule>,
	guards: GuardConfig,
	resolver: Option<Arc<dyn Resolver>>,
	inbound_name: Option<String>,
	inbound_type: Option<InboundType>,
	geodata: Option<Arc<GeoData>>,
	hijack_seen: bool,
}

impl AclEngineBuilder {
	/// Add Clash / Mihomo rule lines (e.g. `DOMAIN-SUFFIX,google.com,proxy`).
	///
	/// Returns an error identifying the first invalid line.
	pub fn clash_rules<I, S>(mut self, rules: I) -> eyre::Result<Self>
	where
		I: IntoIterator<Item = S>,
		S: AsRef<str>,
	{
		self.clash.extend(metacubex::parse_lines(rules)?);
		Ok(self)
	}

	/// Add already-parsed real Hysteria 2 (apernet) ACL rules, converted to
	/// wind rules via [`apernet::acl_to_rules`]. These take precedence over
	/// Clash rules (they are placed first in source order).
	pub fn apernet_acl(mut self, acl: &[AclRule]) -> Self {
		if acl.iter().any(|r| r.hijack.is_some()) {
			self.hijack_seen = true;
		}
		self.apernet.extend(apernet::acl_to_rules(acl));
		self
	}

	/// Parse and add real Hysteria 2 (apernet) ACL rules from a multiline
	/// string (`reject(geoip:cn)` per line; `#` comments and blanks skipped).
	pub fn apernet_acl_str(self, input: &str) -> eyre::Result<Self> {
		let rules = apernet::parse_multiline(input)?;
		Ok(self.apernet_acl(&rules))
	}

	/// Add already-lowered [`wind_core::rule::Rule`]s directly, in the given
	/// order. Useful for hosts that convert their own config (e.g. a legacy ACL
	/// table plus explicit rules) to wind rules before handing them off. These
	/// are evaluated after apernet rules and before Clash rules.
	pub fn rules(mut self, rules: impl IntoIterator<Item = Rule>) -> Self {
		self.raw.extend(rules);
		self
	}

	/// Enable loopback / private-range guards. Requires [`Self::resolver`].
	pub fn guards(mut self, guards: GuardConfig) -> Self {
		self.guards = guards;
		self
	}

	/// Set the resolver used by guards (and, in future, by IP-based rules on
	/// domain targets).
	pub fn resolver(mut self, resolver: Arc<dyn Resolver>) -> Self {
		self.resolver = Some(resolver);
		self
	}

	/// Provide a static inbound name so `IN-NAME` rules can match.
	pub fn inbound_name(mut self, name: impl Into<String>) -> Self {
		self.inbound_name = Some(name.into());
		self
	}

	/// Provide the static inbound type so `IN-TYPE` rules can match. Only set
	/// this for inbounds that are genuinely SOCKS or HTTP.
	pub fn inbound_type(mut self, ty: InboundType) -> Self {
		self.inbound_type = Some(ty);
		self
	}

	/// Provide the GeoIP / GeoSite database so `GEOIP` / `GEOSITE` rules can
	/// match. Without it those rules never match (and [`Self::build`] warns if
	/// any are present).
	pub fn geodata(mut self, geodata: Arc<GeoData>) -> Self {
		self.geodata = Some(geodata);
		self
	}

	/// Finalize the engine. Errors if a guard is enabled without a resolver.
	pub fn build(self) -> eyre::Result<AclEngine> {
		if self.guards.enabled() && self.resolver.is_none() {
			eyre::bail!("loopback/private guards are enabled but no resolver was provided");
		}
		if self.hijack_seen {
			tracing::warn!("apernet ACL hijack/redirect targets are parsed but not yet honored; they will be ignored");
		}
		let apernet_count = self.apernet.len();
		if apernet_count > 0 {
			tracing::info!("[acl] compiled {apernet_count} apernet ACL rule(s) to Metacubex format");
		}

		// Warn if geo rules are present but the data to evaluate them is not:
		// without a database, `GEOIP` / `GEOSITE` rules can never match and
		// their traffic silently falls through to later rules / the default
		// outbound (fail-open). ASN rules (`IP-ASN`) are never supported yet.
		if self.geodata.is_none() {
			let (geo, asn) =
				self.apernet
					.iter()
					.chain(self.raw.iter())
					.chain(self.clash.iter())
					.fold((false, false), |(geo, asn), r| {
						let (g, a) = rule_geo_kinds(r);
						(geo || g, asn || a)
					});
			if geo {
				tracing::warn!(
					"ACL contains GEOIP/GEOSITE rules but no geodata database was provided; those rules will never match. \
					 Call `AclEngineBuilder::geodata(..)`."
				);
			}
			if asn {
				tracing::warn!("ACL contains IP-ASN rules, which are not yet supported; those rules will never match");
			}
		}

		// apernet-converted rules take precedence, then directly-supplied raw
		// rules, then explicit Clash rules. The IR embedding preserves source
		// order and the optimizer preserves first-match-wins semantics.
		let all: Vec<Rule> = self.apernet.into_iter().chain(self.raw).chain(self.clash).collect();
		let ruleset = compile(Ruleset::from_rules(all, self.default_outbound));

		Ok(AclEngine {
			ruleset,
			guards: self.guards,
			resolver: self.resolver,
			inbound_name: self.inbound_name,
			inbound_type: self.inbound_type,
			geodata: self.geodata,
		})
	}
}

/// Whether a rule (recursing through logical combinators) references a
/// geo-database match: `.0` for GEOIP/GEOSITE, `.1` for IP-ASN.
fn rule_geo_kinds(rule: &Rule) -> (bool, bool) {
	match &rule.rule_type {
		RuleType::GeoIp(_) | RuleType::SrcGeoIp(_) | RuleType::GeoSite(_) => (true, false),
		RuleType::IpAsn(_) | RuleType::SrcIpAsn(_) => (false, true),
		RuleType::And(rs) | RuleType::Or(rs) | RuleType::SubRule(rs, _) => rs.iter().fold((false, false), |(g, a), r| {
			let (rg, ra) = rule_geo_kinds(r);
			(g || rg, a || ra)
		}),
		RuleType::Not(r) => rule_geo_kinds(r),
		_ => (false, false),
	}
}
