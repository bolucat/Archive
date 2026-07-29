//! Real Hysteria 2 ACL syntax (`apernet/hysteria`).
//!
//! Parses the genuine apernet/hysteria ACL — a **function-call** form,
//! `outbound(address[, proto/port[, hijack]])` (e.g. `reject(geoip:cn)`,
//! `default(8.8.8.8, udp/53, 1.1.1.1)`) — into [`AclRule`]s and lowers them to
//! `wind_core::rule::Rule`s via [`acl_to_rules`].
//!
//! This is distinct from the tuic-server `legacy` dialect, which is
//! space-separated (`proxy 10.0.0.0/8 tcp/443`). The grammar and lowering here
//! follow apernet's `extras/outbounds/acl/{parse,compile,matchers}.go`:
//!
//! * **Address dispatch is ordered and structural** (first match wins, after
//!   lower-casing and trailing-dot trimming): `all`/`*` → `geoip:` → `geosite:`
//!   → `suffix:` → contains `/` (CIDR) → parses as IP → contains `*` (wildcard)
//!   → exact domain.
//! * **proto/port** accepts the omitted/`*`/`*/*` (both, any port), bare
//!   `tcp`/`udp` (that protocol, any port), and `<proto>/<port>` forms where
//!   `<proto>` is `tcp`/`udp`/`*` and `<port>` is `*`, a single port, or an
//!   inclusive `lo-hi` range. A resulting start port of `0` is apernet's "any
//!   port" sentinel and lowers to no port condition.
//! * **hijack** is an IPv4/IPv6 literal (no domain, no port). It is retained on
//!   the parsed rule but cannot be expressed in the wind IR, so it is dropped
//!   (with a warning) during lowering — see [`crate::engine`].
//!
//! Everything is case-insensitive (outbound name, address, proto/port), and a
//! `#` truncates the line at the first occurrence.
//!
//! Deliberate divergences from upstream (all benign — they only affect
//! malformed or non-DNS inputs, and this parser is stricter, never looser):
//!
//! * A `*`-bearing address is lowered to `DOMAIN-WILDCARD`, whose compiler also
//!   treats `?` as a single-character wildcard; upstream's `deepMatchRune`
//!   matches `?` literally. `?` is not a valid hostname character, so this only
//!   differs for pathological patterns.
//! * Matching does **not** apply IDNA `ToUnicode` to the host (punycode `xn--`
//!   hosts compare verbatim), and exact-domain matching folds ASCII case only.
//! * An argument containing a literal `)`, an empty/whitespace-only argument,
//!   and an empty address are rejected here; upstream's end-anchored line regex
//!   tolerates some of these degenerate forms. None occur in real ACLs.

use std::{fmt, net::IpAddr};

use pest::Parser;
use pest_derive::Parser;
use serde::{Deserialize, Deserializer, Serialize, de};
use wind_core::rule::{self as wrule, NetworkType};

#[derive(Parser)]
#[grammar = "syntax/apernet.pest"]
struct AclParser;

/// A single parsed apernet ACL rule: `outbound(address[, proto/port[,
/// hijack]])`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AclRule {
	/// The outbound name (`direct`, `reject`, `default`, or a custom name).
	pub outbound: String,
	/// The matched address / condition (required).
	pub address: AclAddress,
	/// Optional protocol + port constraint. `None` means the argument was
	/// omitted (both protocols, all ports).
	pub proto_port: Option<AclProtoPort>,
	/// Optional hijack/redirect target — an IP literal. Retained for fidelity
	/// but not honored during routing.
	pub hijack: Option<String>,
}

/// An apernet address / condition (arg 1).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AclAddress {
	/// `all` or `*` — match every connection.
	All,
	/// A bare IPv4/IPv6 literal.
	Ip(String),
	/// CIDR notation (IPv4 or IPv6).
	Cidr(String),
	/// `geoip:<country>` — GeoIP country-code match.
	GeoIp(String),
	/// `geosite:<name>[@<attr>…]` — GeoSite category match with optional
	/// attribute filters.
	GeoSite {
		/// Base category name.
		name: String,
		/// Zero or more `@attr` filters (not representable in the wind IR).
		attrs: Vec<String>,
	},
	/// `suffix:<domain>` — domain suffix (matches the apex and subdomains).
	Suffix(String),
	/// A glob domain containing at least one `*` (`*` spans label boundaries).
	Wildcard(String),
	/// An exact domain (matches only itself, no subdomains).
	Domain(String),
}

/// A protocol + port constraint (arg 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AclProtoPort {
	/// Constrained protocol.
	pub proto: AclProto,
	/// Constrained port (or [`AclPort::Any`]).
	pub port: AclPort,
}

/// Protocol constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AclProto {
	/// TCP only.
	Tcp,
	/// UDP only.
	Udp,
	/// Both protocols (`*` or omitted).
	Both,
}

/// Port constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AclPort {
	/// All ports (`*`, omitted, or the apernet start-port-`0` sentinel).
	Any,
	/// A single port.
	Single(u16),
	/// An inclusive port range (`lo <= hi`, `lo >= 1`).
	Range(u16, u16),
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a single apernet ACL rule line.
///
/// `#` comments are stripped and the line is trimmed first; a comment-only or
/// blank line is an error.
pub fn parse_rule(line: &str) -> eyre::Result<AclRule> {
	let line = strip_comment(line).trim();
	if line.is_empty() {
		return Err(eyre::eyre!("comment or empty line"));
	}
	parse_with_pest(line)
}

/// Parse a multiline apernet ACL string (one rule per line; blank and
/// `#`-comment lines skipped). Errors are reported with a 1-based line number.
pub fn parse_multiline(input: &str) -> eyre::Result<Vec<AclRule>> {
	input
		.lines()
		.enumerate()
		.map(|(i, line)| (i, strip_comment(line).trim()))
		.filter(|(_, line)| !line.is_empty())
		.map(|(i, line)| parse_rule(line).map_err(|e| eyre::eyre!("line {}: {}", i + 1, e)))
		.collect()
}

/// Truncate a line at the first `#`, matching apernet's comment handling.
fn strip_comment(line: &str) -> &str {
	match line.find('#') {
		Some(i) => &line[..i],
		None => line,
	}
}

fn parse_with_pest(line: &str) -> eyre::Result<AclRule> {
	let mut pairs = AclParser::parse(Rule::acl_rule, line).map_err(|e| eyre::eyre!("parse error: {}", e))?;
	let rule_pair = pairs.next().ok_or_else(|| eyre::eyre!("empty rule"))?;

	let mut outbound = String::new();
	let mut args: Vec<String> = Vec::new();
	for pair in rule_pair.into_inner() {
		match pair.as_rule() {
			Rule::outbound => outbound = pair.as_str().to_string(),
			Rule::arg => args.push(pair.as_str().trim().to_string()),
			Rule::EOI => {}
			_ => {}
		}
	}

	let address = parse_address(args.first().ok_or_else(|| eyre::eyre!("missing address"))?)?;
	let proto_port = match args.get(1) {
		Some(s) => Some(parse_proto_port(s)?),
		None => None,
	};
	let hijack = match args.get(2) {
		Some(s) => Some(parse_hijack(s)?),
		None => None,
	};

	Ok(AclRule {
		outbound,
		address,
		proto_port,
		hijack,
	})
}

/// Parse arg 1 (the address) via apernet's ordered structural dispatch.
fn parse_address(s: &str) -> eyre::Result<AclAddress> {
	// apernet: `addr = strings.TrimRight(strings.ToLower(addr), ".")`.
	let addr = s.trim().to_ascii_lowercase();
	let addr = addr.trim_end_matches('.').to_string();

	// Reject a degenerate empty address (e.g. `direct(.)`). Upstream would
	// compile this to a never-matching exact-domain rule; surfacing it as a
	// config error is friendlier and keeps `direct(.)` / `direct( )` consistent.
	if addr.is_empty() {
		return Err(eyre::eyre!("empty address"));
	}
	if addr == "*" || addr == "all" {
		return Ok(AclAddress::All);
	}
	if let Some(cc) = addr.strip_prefix("geoip:") {
		if cc.is_empty() {
			return Err(eyre::eyre!("empty geoip country code"));
		}
		return Ok(AclAddress::GeoIp(cc.to_string()));
	}
	if let Some(rest) = addr.strip_prefix("geosite:") {
		let mut iter = rest.split('@');
		let name = iter.next().unwrap_or("").trim().to_string();
		if name.is_empty() {
			return Err(eyre::eyre!("empty geosite name"));
		}
		let attrs: Vec<String> = iter.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
		return Ok(AclAddress::GeoSite { name, attrs });
	}
	if let Some(suf) = addr.strip_prefix("suffix:") {
		if suf.is_empty() {
			return Err(eyre::eyre!("empty domain suffix"));
		}
		return Ok(AclAddress::Suffix(suf.to_string()));
	}
	if addr.contains('/') {
		// Validate now so malformed CIDRs fail at parse time (matching apernet).
		addr.parse::<ipnet::IpNet>()
			.map_err(|e| eyre::eyre!("invalid CIDR {:?}: {}", addr, e))?;
		return Ok(AclAddress::Cidr(addr));
	}
	if addr.parse::<IpAddr>().is_ok() {
		return Ok(AclAddress::Ip(addr));
	}
	if addr.contains('*') {
		return Ok(AclAddress::Wildcard(addr));
	}
	Ok(AclAddress::Domain(addr))
}

/// Parse arg 2 (the protocol/port constraint), mirroring `parseProtoPort`.
fn parse_proto_port(s: &str) -> eyre::Result<AclProtoPort> {
	let pp = s.trim().to_ascii_lowercase();

	// Early-return all-protocols/all-ports forms (checked before any `/`).
	if pp.is_empty() || pp == "*" || pp == "*/*" {
		return Ok(AclProtoPort {
			proto: AclProto::Both,
			port: AclPort::Any,
		});
	}

	if !pp.contains('/') {
		let proto = match pp.as_str() {
			"tcp" => AclProto::Tcp,
			"udp" => AclProto::Udp,
			other => return Err(eyre::eyre!("invalid protocol/port: {:?}", other)),
		};
		return Ok(AclProtoPort {
			proto,
			port: AclPort::Any,
		});
	}

	let (proto_s, port_s) = pp.split_once('/').expect("contains '/'");
	let proto = match proto_s {
		"tcp" => AclProto::Tcp,
		"udp" => AclProto::Udp,
		"*" => AclProto::Both,
		other => return Err(eyre::eyre!("invalid protocol: {:?}", other)),
	};
	let port = parse_port(port_s)?;
	Ok(AclProtoPort { proto, port })
}

/// Parse the port portion (`*`, a single port, or an inclusive range). A
/// resulting start port of `0` becomes [`AclPort::Any`] (apernet sentinel).
fn parse_port(port_s: &str) -> eyre::Result<AclPort> {
	if port_s == "*" {
		return Ok(AclPort::Any);
	}
	if let Some((lo, hi)) = port_s.split_once('-') {
		let lo: u16 = lo.parse().map_err(|_| {
			eyre::eyre!(
				"invalid port range {:?}: start port {:?} is non-numeric or out of range (0-65535)",
				port_s,
				lo
			)
		})?;
		let hi: u16 = hi.parse().map_err(|_| {
			eyre::eyre!(
				"invalid port range {:?}: end port {:?} is non-numeric or out of range (0-65535)",
				port_s,
				hi
			)
		})?;
		if lo > hi {
			return Err(eyre::eyre!("invalid port range {:?}: {} > {}", port_s, lo, hi));
		}
		// apernet: `StartPort == 0` disables the port check entirely.
		Ok(if lo == 0 { AclPort::Any } else { AclPort::Range(lo, hi) })
	} else {
		let p: u16 = port_s
			.parse()
			.map_err(|_| eyre::eyre!("invalid port {:?}: non-numeric or out of range (0-65535)", port_s))?;
		Ok(if p == 0 { AclPort::Any } else { AclPort::Single(p) })
	}
}

/// Parse arg 3 (the hijack target): an IPv4/IPv6 literal only.
fn parse_hijack(s: &str) -> eyre::Result<String> {
	let h = s.trim();
	h.parse::<IpAddr>()
		.map_err(|_| eyre::eyre!("invalid hijack address (must be an IP): {:?}", h))?;
	Ok(h.to_string())
}

// ---------------------------------------------------------------------------
// Lowering to wind rules
// ---------------------------------------------------------------------------

/// Lower a list of apernet rules into Metacubex-style [`wrule::Rule`]s.
///
/// Each [`AclRule`] may expand to multiple rules (an address combined with a
/// port condition yields one `AND` rule per pair, exactly as the `legacy`
/// dialect does). Malformed addresses are dropped with a warning. `hijack` is
/// not representable in the IR and is silently dropped here — callers that want
/// to warn about it should inspect [`AclRule::hijack`] before lowering.
pub fn acl_to_rules(acl: &[AclRule]) -> Vec<wrule::Rule> {
	acl.iter().flat_map(acl_rule_to_rules).collect()
}

fn acl_rule_to_rules(acl: &AclRule) -> Vec<wrule::Rule> {
	// Outbound is passed through verbatim: `reject`/`block`/`deny` are mapped to
	// a reject verdict (case-insensitively) by the IR embedding, and every other
	// name is a forward target resolved by the engine's outbound registry.
	let target = acl.outbound.clone();

	let addr_rules = address_to_rule_types(&acl.address);
	let port_conds = proto_port_to_conditions(&acl.proto_port);

	// If the address failed to compile (an inner warning already fired), drop the
	// whole rule explicitly. Without this guard the cross-product loop below would
	// silently emit nothing even when a port condition is present — a confusing,
	// fail-open disappearance of the rule's port filter.
	if addr_rules.is_empty() {
		tracing::warn!(
			"apernet rule {acl} produced no address condition; the whole rule (including any port filter) is dropped"
		);
		return Vec::new();
	}

	if port_conds.is_empty() {
		return addr_rules
			.into_iter()
			.map(|rt| wrule::Rule {
				rule_type: rt,
				target: target.clone(),
				options: Vec::new(),
			})
			.collect();
	}

	// `all`/`*` address is match-everything, so only the port conditions matter.
	if matches!(acl.address, AclAddress::All) {
		return port_conds
			.into_iter()
			.map(|rt| wrule::Rule {
				rule_type: rt,
				target: target.clone(),
				options: Vec::new(),
			})
			.collect();
	}

	let mut result = Vec::new();
	for ar in &addr_rules {
		for pc in &port_conds {
			result.push(wrule::Rule {
				rule_type: and2(clone_rule_type(ar), clone_rule_type(pc)),
				target: target.clone(),
				options: Vec::new(),
			});
		}
	}
	result
}

fn address_to_rule_types(addr: &AclAddress) -> Vec<wrule::RuleType> {
	match addr {
		AclAddress::All => vec![wrule::RuleType::Match],
		AclAddress::Ip(s) => match s.parse::<IpAddr>() {
			Ok(IpAddr::V4(v4)) => ipnet::Ipv4Net::new(v4, 32)
				.map(|n| vec![wrule::RuleType::IpCidr(ipnet::IpNet::V4(n))])
				.unwrap_or_default(),
			Ok(IpAddr::V6(v6)) => ipnet::Ipv6Net::new(v6, 128)
				.map(|n| vec![wrule::RuleType::IpCidr(ipnet::IpNet::V6(n))])
				.unwrap_or_default(),
			Err(e) => {
				tracing::warn!("apernet address {s:?} could not be parsed as IPv4/IPv6 ({e}); rule dropped");
				vec![]
			}
		},
		AclAddress::Cidr(s) => match s.parse::<ipnet::IpNet>() {
			Ok(net) => vec![wrule::RuleType::IpCidr(net)],
			Err(_) => {
				tracing::warn!("apernet CIDR {s:?} invalid; rule dropped");
				vec![]
			}
		},
		AclAddress::GeoIp(cc) => vec![wrule::RuleType::GeoIp(cc.clone())],
		AclAddress::GeoSite { name, attrs } => {
			if !attrs.is_empty() {
				tracing::warn!(
					"apernet geosite attribute(s) {attrs:?} on {name:?} are not representable in the wind IR; matching the \
					 base category only"
				);
			}
			vec![wrule::RuleType::GeoSite(name.clone())]
		}
		AclAddress::Suffix(d) => vec![wrule::RuleType::DomainSuffix(d.clone())],
		AclAddress::Wildcard(p) => match wildcard_rule_type(p) {
			Some(rt) => vec![rt],
			None => {
				tracing::warn!("apernet wildcard {p:?} could not be compiled; rule dropped");
				vec![]
			}
		},
		AclAddress::Domain(d) => vec![wrule::RuleType::Domain(d.clone())],
	}
}

/// Build a `DOMAIN-WILDCARD` rule type by round-tripping through the Metacubex
/// parser, which compiles the glob (`*`/`?`) into an anchored case-insensitive
/// regex. `DomainWildcard` (unlike `DomainRegex`) is clone-safe via
/// Display→parse, so it survives [`clone_rule_type`] when AND-ed with a port.
fn wildcard_rule_type(pattern: &str) -> Option<wrule::RuleType> {
	wrule::Rule::parse(&format!("DOMAIN-WILDCARD,{pattern},__APERNET"))
		.ok()
		.map(|r| r.rule_type)
}

fn proto_port_to_conditions(pp: &Option<AclProtoPort>) -> Vec<wrule::RuleType> {
	let Some(pp) = pp else {
		return Vec::new();
	};

	let net = match pp.proto {
		AclProto::Tcp => Some(NetworkType::Tcp),
		AclProto::Udp => Some(NetworkType::Udp),
		AclProto::Both => None,
	};
	let port = match pp.port {
		AclPort::Any => None,
		AclPort::Single(p) => Some(wrule::RuleType::DstPort(p)),
		AclPort::Range(lo, hi) => Some(wrule::RuleType::DstPortRange(lo, hi)),
	};

	match (net, port) {
		(None, None) => Vec::new(),
		(Some(n), None) => vec![wrule::RuleType::Network(n)],
		(None, Some(p)) => vec![p],
		(Some(n), Some(p)) => vec![and2(wrule::RuleType::Network(n), p)],
	}
}

/// Build an `AND` rule type over two leaf rule types (sub-rules carry no
/// target).
fn and2(a: wrule::RuleType, b: wrule::RuleType) -> wrule::RuleType {
	wrule::RuleType::And(vec![
		wrule::Rule {
			rule_type: a,
			target: String::new(),
			options: Vec::new(),
		},
		wrule::Rule {
			rule_type: b,
			target: String::new(),
			options: Vec::new(),
		},
	])
}

/// Clone a `RuleType` via Display→parse (no `Regex`-only types are produced by
/// this module's lowering, so the round-trip is total).
fn clone_rule_type(rt: &wrule::RuleType) -> wrule::RuleType {
	let s = format!("{rt},__CLONE");
	wrule::Rule::parse(&s).map(|r| r.rule_type).unwrap_or(wrule::RuleType::Match)
}

// ---------------------------------------------------------------------------
// Display (round-trips to the function-call surface form)
// ---------------------------------------------------------------------------

impl fmt::Display for AclAddress {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			AclAddress::All => write!(f, "all"),
			AclAddress::Ip(s) | AclAddress::Cidr(s) | AclAddress::Wildcard(s) | AclAddress::Domain(s) => write!(f, "{s}"),
			AclAddress::GeoIp(cc) => write!(f, "geoip:{cc}"),
			AclAddress::GeoSite { name, attrs } => {
				write!(f, "geosite:{name}")?;
				for a in attrs {
					write!(f, "@{a}")?;
				}
				Ok(())
			}
			AclAddress::Suffix(d) => write!(f, "suffix:{d}"),
		}
	}
}

impl fmt::Display for AclProtoPort {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let proto = match self.proto {
			AclProto::Tcp => "tcp",
			AclProto::Udp => "udp",
			AclProto::Both => "*",
		};
		match self.port {
			AclPort::Any => match self.proto {
				AclProto::Both => write!(f, "*"),
				_ => write!(f, "{proto}"),
			},
			AclPort::Single(p) => write!(f, "{proto}/{p}"),
			AclPort::Range(lo, hi) => write!(f, "{proto}/{lo}-{hi}"),
		}
	}
}

impl fmt::Display for AclRule {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}({}", self.outbound, self.address)?;
		// hijack is positional (arg 3), so it requires a proto/port slot (arg 2).
		// When a hijack is set without an explicit proto/port, emit the `*`
		// (both protocols, any port) placeholder so the surface form round-trips.
		if let Some(h) = &self.hijack {
			let pp = self.proto_port.unwrap_or(AclProtoPort {
				proto: AclProto::Both,
				port: AclPort::Any,
			});
			write!(f, ", {pp}, {h}")?;
		} else if let Some(pp) = self.proto_port {
			write!(f, ", {pp}")?;
		}
		write!(f, ")")
	}
}

// ---------------------------------------------------------------------------
// Serde
// ---------------------------------------------------------------------------

impl<'de> Deserialize<'de> for AclRule {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		parse_rule(&s).map_err(de::Error::custom)
	}
}

/// Deserialize the apernet `acl` field, which may be either a single multiline
/// string or a sequence of rule strings.
pub fn deserialize_acl<'de, D>(deserializer: D) -> Result<Vec<AclRule>, D::Error>
where
	D: Deserializer<'de>,
{
	use serde::de::Visitor;

	struct AclVisitor;

	impl<'de> Visitor<'de> for AclVisitor {
		type Value = Vec<AclRule>;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter.write_str("a multiline string or a sequence of apernet ACL rule strings")
		}

		fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			parse_multiline(v).map_err(de::Error::custom)
		}

		fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			self.visit_str(&v)
		}

		fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
		where
			A: de::SeqAccess<'de>,
		{
			let mut out = Vec::new();
			while let Some(s) = seq.next_element::<String>()? {
				let line = strip_comment(&s).trim();
				if line.is_empty() {
					continue;
				}
				out.push(parse_rule(line).map_err(de::Error::custom)?);
			}
			Ok(out)
		}
	}

	deserializer.deserialize_any(AclVisitor)
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
	use wind_core::rule::MatchContext;

	use super::*;

	fn one(rule: &str) -> AclRule {
		parse_rule(rule).unwrap_or_else(|e| panic!("parse {rule:?}: {e}"))
	}

	fn lowered(rule: &str) -> Vec<wrule::Rule> {
		acl_to_rules(std::slice::from_ref(&one(rule)))
	}

	// ---- parsing: addresses ----

	#[test]
	fn parse_ip_v4() {
		let r = one("direct(1.1.1.1)");
		assert_eq!(r.outbound, "direct");
		assert_eq!(r.address, AclAddress::Ip("1.1.1.1".into()));
		assert_eq!(r.proto_port, None);
		assert_eq!(r.hijack, None);
	}

	#[test]
	fn parse_ip_v6() {
		assert_eq!(
			one("reject(2606:4700:4700::1111)").address,
			AclAddress::Ip("2606:4700:4700::1111".into())
		);
	}

	#[test]
	fn parse_cidr_v4_and_v6() {
		assert_eq!(one("direct(8.8.8.0/24)").address, AclAddress::Cidr("8.8.8.0/24".into()));
		assert_eq!(one("reject(2001:db8::/32)").address, AclAddress::Cidr("2001:db8::/32".into()));
	}

	#[test]
	fn parse_exact_domain_is_lowercased() {
		assert_eq!(one("some_proxy(IPinfo.io)").address, AclAddress::Domain("ipinfo.io".into()));
	}

	#[test]
	fn parse_trailing_dot_stripped() {
		assert_eq!(one("direct(example.com.)").address, AclAddress::Domain("example.com".into()));
	}

	#[test]
	fn parse_suffix() {
		assert_eq!(
			one("v6_only(suffix:google.com)").address,
			AclAddress::Suffix("google.com".into())
		);
	}

	#[test]
	fn parse_wildcard_single_and_multi() {
		assert_eq!(one("reject(*.v2ex.com)").address, AclAddress::Wildcard("*.v2ex.com".into()));
		assert_eq!(one("reject(*.google.*)").address, AclAddress::Wildcard("*.google.*".into()));
	}

	#[test]
	fn parse_geoip() {
		assert_eq!(one("reject(geoip:CN)").address, AclAddress::GeoIp("cn".into()));
	}

	#[test]
	fn parse_geosite_with_attrs() {
		assert_eq!(
			one("reject(geosite:google@cn@ads)").address,
			AclAddress::GeoSite {
				name: "google".into(),
				attrs: vec!["cn".into(), "ads".into()],
			}
		);
		assert_eq!(
			one("reject(geosite:netflix)").address,
			AclAddress::GeoSite {
				name: "netflix".into(),
				attrs: vec![],
			}
		);
	}

	#[test]
	fn parse_all_and_star_are_catch_all() {
		assert_eq!(one("direct(all)").address, AclAddress::All);
		assert_eq!(one("direct(*)").address, AclAddress::All);
	}

	// ---- parsing: proto/port ----

	#[test]
	fn parse_proto_port_forms() {
		assert_eq!(
			one("reject(all, udp/443)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Udp,
				port: AclPort::Single(443)
			})
		);
		assert_eq!(
			one("direct(all, tcp)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Tcp,
				port: AclPort::Any
			})
		);
		assert_eq!(
			one("direct(all, *)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Both,
				port: AclPort::Any
			})
		);
		assert_eq!(
			one("direct(all, */443)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Both,
				port: AclPort::Single(443)
			})
		);
		assert_eq!(
			one("direct(all, udp/20000-30000)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Udp,
				port: AclPort::Range(20000, 30000)
			})
		);
	}

	#[test]
	fn parse_port_zero_is_any() {
		assert_eq!(
			one("direct(all, tcp/0)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Tcp,
				port: AclPort::Any
			})
		);
		// A range starting at 0 disables the port check entirely (apernet sentinel).
		assert_eq!(
			one("direct(all, */0-100)").proto_port,
			Some(AclProtoPort {
				proto: AclProto::Both,
				port: AclPort::Any
			})
		);
	}

	#[test]
	fn parse_reversed_range_is_error() {
		assert!(parse_rule("direct(all, */3-1)").is_err());
	}

	#[test]
	fn parse_hijack() {
		let r = one("default(8.8.8.8, *, 1.1.1.1)");
		assert_eq!(r.address, AclAddress::Ip("8.8.8.8".into()));
		assert_eq!(
			r.proto_port,
			Some(AclProtoPort {
				proto: AclProto::Both,
				port: AclPort::Any
			})
		);
		assert_eq!(r.hijack, Some("1.1.1.1".into()));
	}

	#[test]
	fn parse_hijack_must_be_ip() {
		assert!(parse_rule("default(8.8.8.8, udp/53, example.com)").is_err());
	}

	// ---- parsing: mechanics ----

	#[test]
	fn parse_zero_args_is_error() {
		assert!(parse_rule("boom()").is_err());
	}

	#[test]
	fn parse_four_args_is_error() {
		assert!(parse_rule("lol(1,1,1,1)").is_err());
	}

	#[test]
	fn parse_inline_comment_and_whitespace() {
		let r = one("  reject(all, udp/443)   # inline comment");
		assert_eq!(r.outbound, "reject");
		assert_eq!(
			r.proto_port,
			Some(AclProtoPort {
				proto: AclProto::Udp,
				port: AclPort::Single(443)
			})
		);
	}

	#[test]
	fn parse_edge_whitespace_in_args_trimmed() {
		let r = one("my_custom_outbound1(9.9.9.9,*,   8.8.8.8)");
		assert_eq!(r.address, AclAddress::Ip("9.9.9.9".into()));
		assert_eq!(r.hijack, Some("8.8.8.8".into()));
	}

	#[test]
	fn parse_outbound_charset_rejects_hyphen() {
		// `\w+` does not permit hyphens (unlike the legacy dialect).
		assert!(parse_rule("v4-only(all)").is_err());
	}

	#[test]
	fn parse_multiline_skips_blank_and_comment_lines() {
		let input = "\n# a comment\nreject(geoip:cn)\n\ndirect(all) # trailing\n";
		let rules = parse_multiline(input).unwrap();
		assert_eq!(rules.len(), 2);
		assert_eq!(rules[0].address, AclAddress::GeoIp("cn".into()));
		assert_eq!(rules[1].address, AclAddress::All);
	}

	#[test]
	fn parse_multiline_reports_line_number() {
		let err = parse_multiline("direct(all)\nboom()").unwrap_err().to_string();
		assert!(err.contains("line 2"), "got: {err}");
	}

	// ---- lowering ----

	#[test]
	fn lower_ip_v4_host_route() {
		let rules = lowered("direct(1.1.1.1)");
		assert_eq!(rules.len(), 1);
		assert_eq!(rules[0].target, "direct");
		let ctx = MatchContext {
			dst_ip: Some("1.1.1.1".parse().unwrap()),
			..Default::default()
		};
		assert!(rules[0].matches(&ctx));
		let miss = MatchContext {
			dst_ip: Some("1.1.1.2".parse().unwrap()),
			..Default::default()
		};
		assert!(!rules[0].matches(&miss));
	}

	#[test]
	fn lower_ipv6_is_128_host_route() {
		let rules = lowered("direct(2001:db8::1)");
		let hit = MatchContext {
			dst_ip: Some("2001:db8::1".parse().unwrap()),
			..Default::default()
		};
		assert!(rules.iter().any(|r| r.matches(&hit)));
		let other = MatchContext {
			dst_ip: Some("2001:db8::2".parse().unwrap()),
			..Default::default()
		};
		assert!(!rules.iter().any(|r| r.matches(&other)), "/128 must not match a sibling");
	}

	#[test]
	fn lower_cidr() {
		let rules = lowered("direct(10.0.0.0/8)");
		let ctx = MatchContext {
			dst_ip: Some("10.1.2.3".parse().unwrap()),
			..Default::default()
		};
		assert!(rules[0].matches(&ctx));
	}

	#[test]
	fn lower_exact_domain_excludes_subdomains() {
		let rules = lowered("some_proxy(example.com)");
		assert_eq!(rules.len(), 1);
		let exact = MatchContext {
			domain: Some("example.com"),
			..Default::default()
		};
		let sub = MatchContext {
			domain: Some("www.example.com"),
			..Default::default()
		};
		assert!(rules[0].matches(&exact));
		assert!(!rules[0].matches(&sub), "exact domain must not match subdomains");
	}

	#[test]
	fn lower_suffix_includes_apex_and_subdomains() {
		let rules = lowered("v6_only(suffix:example.com)");
		let apex = MatchContext {
			domain: Some("example.com"),
			..Default::default()
		};
		let sub = MatchContext {
			domain: Some("a.b.example.com"),
			..Default::default()
		};
		assert!(rules[0].matches(&apex));
		assert!(rules[0].matches(&sub));
	}

	#[test]
	fn lower_wildcard_excludes_apex() {
		let rules = lowered("reject(*.example.com)");
		assert_eq!(rules.len(), 1);
		let sub = MatchContext {
			domain: Some("www.example.com"),
			..Default::default()
		};
		let apex = MatchContext {
			domain: Some("example.com"),
			..Default::default()
		};
		assert!(rules[0].matches(&sub), "wildcard must match subdomains");
		assert!(!rules[0].matches(&apex), "apernet *.d must NOT match bare d");
	}

	#[test]
	fn lower_geoip_and_geosite() {
		let geoip = lowered("reject(geoip:cn)");
		assert!(matches!(geoip[0].rule_type, wrule::RuleType::GeoIp(ref c) if c == "cn"));
		assert_eq!(geoip[0].target, "reject");

		// Attributes are dropped; base category retained.
		let geosite = lowered("reject(geosite:google@ads)");
		assert!(matches!(geosite[0].rule_type, wrule::RuleType::GeoSite(ref n) if n == "google"));
	}

	#[test]
	fn lower_all_no_ports_is_match() {
		let rules = lowered("direct(all)");
		assert_eq!(rules.len(), 1);
		assert!(matches!(rules[0].rule_type, wrule::RuleType::Match));
	}

	#[test]
	fn lower_all_with_proto_port_drops_address() {
		// reject(all, udp/443) -> port-only AND(Network(udp), DstPort(443)).
		let rules = lowered("reject(all, udp/443)");
		assert_eq!(rules.len(), 1);
		assert!(matches!(rules[0].rule_type, wrule::RuleType::And(_)));
		let udp = MatchContext {
			dst_port: Some(443),
			network: Some(NetworkType::Udp),
			..Default::default()
		};
		let tcp = MatchContext {
			dst_port: Some(443),
			network: Some(NetworkType::Tcp),
			..Default::default()
		};
		assert!(rules[0].matches(&udp));
		assert!(!rules[0].matches(&tcp), "udp/443 must not match tcp");
	}

	#[test]
	fn lower_bare_protocol_is_network_only() {
		let rules = lowered("direct(all, tcp)");
		assert!(matches!(rules[0].rule_type, wrule::RuleType::Network(NetworkType::Tcp)));
		let tcp_any = MatchContext {
			dst_port: Some(12345),
			network: Some(NetworkType::Tcp),
			..Default::default()
		};
		assert!(rules[0].matches(&tcp_any));
	}

	#[test]
	fn lower_star_port_is_dst_port_only() {
		// */443 -> both protocols on port 443 (no Network constraint).
		let rules = lowered("direct(all, */443)");
		assert!(matches!(rules[0].rule_type, wrule::RuleType::DstPort(443)));
		let udp = MatchContext {
			dst_port: Some(443),
			network: Some(NetworkType::Udp),
			..Default::default()
		};
		assert!(rules[0].matches(&udp));
	}

	#[test]
	fn lower_addr_and_port_is_nested_and() {
		// default(8.8.4.4, udp/53, ...) -> AND(IpCidr/32, AND(Network(udp),
		// DstPort(53))).
		let rules = lowered("default(8.8.4.4, udp/53, 1.1.1.1)");
		assert_eq!(rules.len(), 1);
		let hit = MatchContext {
			dst_ip: Some("8.8.4.4".parse().unwrap()),
			dst_port: Some(53),
			network: Some(NetworkType::Udp),
			..Default::default()
		};
		let wrong_port = MatchContext {
			dst_ip: Some("8.8.4.4".parse().unwrap()),
			dst_port: Some(54),
			network: Some(NetworkType::Udp),
			..Default::default()
		};
		assert!(rules[0].matches(&hit));
		assert!(!rules[0].matches(&wrong_port));
	}

	#[test]
	fn lower_range_with_proto() {
		let rules = lowered("direct(2001:db8::/32, */20000-30000)");
		assert_eq!(rules.len(), 1);
		let inside = MatchContext {
			dst_ip: Some("2001:db8::5".parse().unwrap()),
			dst_port: Some(25000),
			..Default::default()
		};
		let outside = MatchContext {
			dst_ip: Some("2001:db8::5".parse().unwrap()),
			dst_port: Some(40000),
			..Default::default()
		};
		assert!(rules[0].matches(&inside));
		assert!(!rules[0].matches(&outside));
	}

	#[test]
	fn lower_reject_target_passthrough() {
		assert_eq!(lowered("reject(all)")[0].target, "reject");
		assert_eq!(lowered("REJECT(all)")[0].target, "REJECT");
	}

	#[test]
	fn lower_malformed_cidr_is_parse_error() {
		// A `/`-bearing address that is not a valid CIDR fails at parse time.
		assert!(parse_rule("direct(10.0.0.0/999)").is_err());
	}

	#[test]
	fn dispatch_checks_slash_before_star() {
		// `/` (CIDR) is tested before `*` (wildcard), mirroring apernet — so an
		// address with both is treated as a (here invalid) CIDR, not a wildcard.
		assert!(parse_rule("direct(*.ex/ample.com)").is_err());
	}

	#[test]
	fn empty_address_and_empty_geo_names_rejected() {
		assert!(parse_rule("direct(.)").is_err(), "empty address (dot-only) must be rejected");
		assert!(parse_rule("reject(geoip:)").is_err());
		assert!(
			parse_rule("reject(geosite:@cn)").is_err(),
			"empty geosite name must be rejected"
		);
		assert!(parse_rule("reject(suffix:)").is_err());
	}

	#[test]
	fn lower_geosite_drops_multiple_attrs() {
		let rules = lowered("reject(geosite:google@ads@cn)");
		assert_eq!(rules.len(), 1);
		assert!(
			matches!(rules[0].rule_type, wrule::RuleType::GeoSite(ref n) if n == "google"),
			"multi-attr geosite must lower to the bare base category",
		);
	}

	// ---- display round-trip ----

	#[test]
	fn display_round_trips() {
		for line in [
			"direct(1.1.1.1)",
			"reject(geoip:cn)",
			"reject(geosite:google@cn@ads)",
			"v6_only(suffix:google.com)",
			"reject(all, udp/443)",
			"direct(all, *)",
			"direct(all, tcp)",
			"direct(all, udp)",
			"direct(all, */443)",
			"direct(2001:db8::/32, */20000-30000)",
			"default(8.8.8.8, *, 1.1.1.1)",
		] {
			let parsed = one(line);
			let shown = parsed.to_string();
			let reparsed = one(&shown);
			assert_eq!(parsed, reparsed, "round-trip mismatch for {line:?} -> {shown:?}");
		}
	}

	#[test]
	fn display_couples_hijack_with_protoport_slot() {
		// A hand-built rule with a hijack but no proto/port must still round-trip:
		// the `*` placeholder fills the positional proto/port slot.
		let rule = AclRule {
			outbound: "default".into(),
			address: AclAddress::Ip("8.8.8.8".into()),
			proto_port: None,
			hijack: Some("1.1.1.1".into()),
		};
		assert_eq!(rule.to_string(), "default(8.8.8.8, *, 1.1.1.1)");
		assert_eq!(one(&rule.to_string()), one("default(8.8.8.8, *, 1.1.1.1)"));
	}

	#[test]
	fn port_zero_sentinel_round_trips_to_star() {
		// `*/0-100` collapses to "any port" at parse time, so Display normalizes
		// it to `*`; the normalized form must re-parse to an equal rule.
		let parsed = one("direct(all, */0-100)");
		assert_eq!(
			parsed.proto_port,
			Some(AclProtoPort {
				proto: AclProto::Both,
				port: AclPort::Any
			})
		);
		assert_eq!(parsed.to_string(), "direct(all, *)");
		assert_eq!(one(&parsed.to_string()), parsed);
	}

	// ---- serde ----

	#[test]
	fn deserialize_multiline_string() {
		#[derive(Deserialize)]
		struct Cfg {
			#[serde(deserialize_with = "deserialize_acl")]
			acl: Vec<AclRule>,
		}
		let toml = "acl = \"\"\"\nreject(geoip:cn)\ndirect(all)\n\"\"\"\n";
		let cfg: Cfg = toml::from_str(toml).unwrap();
		assert_eq!(cfg.acl.len(), 2);
		assert_eq!(cfg.acl[0].address, AclAddress::GeoIp("cn".into()));
	}

	#[test]
	fn deserialize_sequence_of_strings() {
		#[derive(Deserialize)]
		struct Cfg {
			#[serde(deserialize_with = "deserialize_acl")]
			acl: Vec<AclRule>,
		}
		let toml = "acl = [\"reject(geoip:cn)\", \"direct(all, tcp/443)\"]\n";
		let cfg: Cfg = toml::from_str(toml).unwrap();
		assert_eq!(cfg.acl.len(), 2);
		assert_eq!(
			cfg.acl[1].proto_port,
			Some(AclProtoPort {
				proto: AclProto::Tcp,
				port: AclPort::Single(443)
			})
		);
	}
}
