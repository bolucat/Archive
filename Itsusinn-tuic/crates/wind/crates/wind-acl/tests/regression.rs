//! Zero-regression and optimizer tests for `wind-acl`.
//!
//! The core guarantee (`specs/acl-ir.md` §5) is that the degenerate embedding
//! routes identically to the legacy first-match-wins engine, and that the
//! optimizer (§7) preserves that. We assert it differentially: for a large grid
//! of `MatchContext`s, the legacy reference, the embedded ruleset, and the
//! optimized ruleset all agree.

use std::net::IpAddr;

use wind_acl::{MapField, Match, Ruleset, Side, Verdict, compile};
use wind_core::{
	RouteAction,
	rule::{MatchContext, NetworkType, Rule},
};

/// A representative config exercising typed leaves (domain/ip/port/proto),
/// Predicate fallbacks (geoip/process/wildcard/compound), Pass 1 same-verdict
/// runs, and a Pass 2 differing-verdict port run.
const CONFIG: &str = "
DOMAIN,exact.example.com,proxy
DOMAIN-SUFFIX,google.com,proxy
DOMAIN-SUFFIX,github.com,proxy
DOMAIN-KEYWORD,ads,reject
DOMAIN-WILDCARD,*.track.net,reject
IP-CIDR,10.0.0.0/8,direct
IP-CIDR,192.168.0.0/16,direct
IP-CIDR6,fc00::/7,direct
SRC-IP-CIDR,192.168.1.0/24,proxy
GEOIP,CN,direct
PROCESS-NAME,curl,direct
DST-PORT,80,proxy
DST-PORT,443,direct
DST-PORT,22,reject
NETWORK,udp,reject
AND,((NETWORK,tcp),(DST-PORT,8080)),proxy
MATCH,direct
";

const DEFAULT_OUTBOUND: &str = "direct";

/// Normalized routing decision — reject reason text is not a routing semantic.
#[derive(Debug, PartialEq, Eq)]
enum Decision {
	Forward(String),
	Reject,
}

fn norm_action(a: &RouteAction) -> Decision {
	match a {
		RouteAction::Forward(o) => Decision::Forward(o.clone()),
		RouteAction::Reject(_) => Decision::Reject,
	}
}

fn norm_target(target: &str) -> Decision {
	match target.to_ascii_lowercase().as_str() {
		"reject" | "block" | "deny" => Decision::Reject,
		_ => Decision::Forward(target.to_string()),
	}
}

/// Legacy first-match-wins reference (mirrors `wind_acl::engine::do_route`
/// without guards).
fn reference(rules: &[Rule], ctx: &MatchContext) -> Decision {
	for r in rules {
		if r.matches(ctx) {
			return norm_target(&r.target);
		}
	}
	Decision::Forward(DEFAULT_OUTBOUND.to_string())
}

fn parse(config: &str) -> Vec<Rule> {
	Rule::parse_rules(config).into_iter().filter_map(Result::ok).collect()
}

#[test]
fn embedding_and_optimization_match_legacy_engine() {
	let reference_rules = parse(CONFIG);
	let embedded = Ruleset::from_rules(parse(CONFIG), DEFAULT_OUTBOUND);
	let optimized = compile(Ruleset::from_rules(parse(CONFIG), DEFAULT_OUTBOUND));

	let domains = [
		Some("exact.example.com"),
		Some("www.google.com"),
		Some("google.com"),
		Some("x.github.com"),
		Some("myads.example.com"),
		Some("foo.track.net"),
		Some("baidu.com"),
		None,
	];
	let dst_ips = [Some("1.1.1.1"), Some("10.1.2.3"), Some("192.168.5.5"), Some("fd00::1"), None];
	let dst_ports = [Some(80u16), Some(443), Some(22), Some(8080), None];
	let networks = [Some(NetworkType::Tcp), Some(NetworkType::Udp)];
	let src_ips = [Some("192.168.1.50"), None];
	let processes = [Some("curl"), None];

	let mut cases = 0u64;
	for domain in domains {
		for dst_ip in dst_ips {
			for dst_port in dst_ports {
				for network in networks {
					for src_ip in src_ips {
						for process in processes {
							let ctx = MatchContext {
								domain,
								dst_ip: dst_ip.map(|s| s.parse::<IpAddr>().unwrap()),
								dst_port,
								network,
								src_ip: src_ip.map(|s| s.parse::<IpAddr>().unwrap()),
								process_name: process,
								..Default::default()
							};

							let want = reference(&reference_rules, &ctx);
							let got_embed = norm_action(&embedded.route(&ctx));
							let got_opt = norm_action(&optimized.route(&ctx));

							assert_eq!(want, got_embed, "embedded mismatch for ctx={ctx:?}");
							assert_eq!(want, got_opt, "optimized mismatch for ctx={ctx:?}");
							cases += 1;
						}
					}
				}
			}
		}
	}
	assert!(cases > 1000, "expected a large grid, ran {cases}");
}

#[test]
fn optimizer_builds_sets_and_a_port_map() {
	let optimized = compile(Ruleset::from_rules(parse(CONFIG), DEFAULT_OUTBOUND));

	// Pass 1 produced at least the three same-verdict buckets:
	// {domains proxy}, {domains reject}, {dst ips direct}.
	assert!(
		optimized.sets.len() >= 3,
		"expected >=3 named sets, got {}",
		optimized.sets.len()
	);

	// Pass 2 produced exactly one port verdict map (ports 80/443/22, all
	// different verdicts, disjoint keys).
	assert_eq!(optimized.maps.len(), 1, "expected one port verdict map");
	let map = &optimized.maps[0];
	assert_eq!(map.field, MapField::Port);
	assert_eq!(map.side, Side::Dst);
	assert_eq!(map.entries.len(), 3);

	// The optimized chain is strictly shorter than the embedded one.
	let embedded = Ruleset::from_rules(parse(CONFIG), DEFAULT_OUTBOUND);
	assert!(optimized.chains[0].rules.len() < embedded.chains[0].rules.len());
}

#[test]
fn overlapping_port_run_is_not_compiled_to_a_map() {
	// Overlapping ranges with differing verdicts: first-match-wins must be
	// preserved, so Pass 2 MUST bail and keep the rules ordered.
	let config = "
DST-PORT,1000-2000,proxy
DST-PORT,1500,direct
MATCH,reject
";
	let optimized = compile(Ruleset::from_rules(parse(config), DEFAULT_OUTBOUND));
	assert!(optimized.maps.is_empty(), "overlapping ports must not become a vmap");

	// 1500 hits the first rule (proxy) under first-match-wins, not direct.
	let ctx = MatchContext {
		dst_port: Some(1500),
		..Default::default()
	};
	assert_eq!(norm_action(&optimized.route(&ctx)), Decision::Forward("proxy".into()));
}

#[test]
fn chain_jump_returns_to_caller() {
	// main: if tcp jump sub; else Always -> fallback
	// sub:  if dport 443 -> https; (else exhausted -> fallthrough -> back to main)
	let rs = Ruleset {
		sets: vec![],
		maps: vec![],
		entry: 0,
		chains: vec![
			wind_acl::Chain {
				name: "main".into(),
				policy: Verdict::Forward("policy".into()),
				rules: vec![
					rule(Match::Proto(NetworkType::Tcp), Verdict::Jump("sub".into())),
					rule(Match::Always, Verdict::Forward("fallback".into())),
				],
			},
			wind_acl::Chain {
				name: "sub".into(),
				policy: Verdict::Forward("unused".into()),
				rules: vec![rule(
					Match::Port {
						side: Side::Dst,
						range: 443..=443,
					},
					Verdict::Forward("https".into()),
				)],
			},
		],
	};

	// tcp + 443 -> jump sub -> https
	assert_eq!(route_for(&rs, NetworkType::Tcp, 443), Decision::Forward("https".into()));
	// tcp + 80 -> jump sub -> no match -> return to main -> fallback
	assert_eq!(route_for(&rs, NetworkType::Tcp, 80), Decision::Forward("fallback".into()));
	// udp + 443 -> first rule misses -> fallback
	assert_eq!(route_for(&rs, NetworkType::Udp, 443), Decision::Forward("fallback".into()));
}

#[test]
fn verdict_map_dispatch() {
	let rs = Ruleset {
		sets: vec![],
		maps: vec![wind_acl::VerdictMap {
			side: Side::Dst,
			field: MapField::Port,
			entries: vec![
				(80..=80, Verdict::Forward("a".into())),
				(443..=443, Verdict::Forward("b".into())),
			],
			default: None,
		}],
		entry: 0,
		chains: vec![wind_acl::Chain {
			name: "main".into(),
			policy: Verdict::Forward("policy".into()),
			rules: vec![
				rule(Match::Always, Verdict::Map(0)),
				rule(Match::Always, Verdict::Forward("after-map".into())),
			],
		}],
	};

	// 80 -> a, 443 -> b (map hit)
	assert_eq!(route_port(&rs, 80), Decision::Forward("a".into()));
	assert_eq!(route_port(&rs, 443), Decision::Forward("b".into()));
	// 22 -> map miss (no default) -> fall through to next rule
	assert_eq!(route_port(&rs, 22), Decision::Forward("after-map".into()));
}

// -- small helpers for the hand-built chain tests --

fn rule(matches: Match, verdict: Verdict) -> wind_acl::IrRule {
	wind_acl::IrRule {
		matches,
		stmts: vec![],
		verdict,
	}
}

fn route_for(rs: &Ruleset, net: NetworkType, port: u16) -> Decision {
	let ctx = MatchContext {
		network: Some(net),
		dst_port: Some(port),
		..Default::default()
	};
	norm_action(&rs.route(&ctx))
}

fn route_port(rs: &Ruleset, port: u16) -> Decision {
	let ctx = MatchContext {
		dst_port: Some(port),
		..Default::default()
	};
	norm_action(&rs.route(&ctx))
}
