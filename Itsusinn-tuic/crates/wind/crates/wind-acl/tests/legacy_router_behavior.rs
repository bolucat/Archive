//! Routing-behaviour regression tests migrated from the removed
//! `wind_core::AclRouter` (first-match-wins engine over `Vec<Rule>`).
//!
//! Each test re-expresses the old `AclRouter` scenario through the canonical
//! [`wind_acl::AclEngine`], which is normatively equivalent to the legacy
//! engine for these rule shapes (see `regression.rs` for the differential
//! reference).

use std::sync::Arc;

use wind_acl::AclEngine;
use wind_core::{FlowContext, RouteAction, Router, hooks::Protocol, types::TargetAddr};
use wind_rule::NetworkType;

fn ipv4(addr: &str, port: u16) -> TargetAddr {
	TargetAddr::IPv4(addr.parse().unwrap(), port)
}

fn domain(host: &str, port: u16) -> TargetAddr {
	TargetAddr::Domain(host.to_string(), port)
}

/// Minimal [`FlowContext`] for a plain connection with no client metadata.
fn fc(target: &TargetAddr, tcp: bool) -> FlowContext {
	FlowContext {
		target: target.clone(),
		network: if tcp { NetworkType::Tcp } else { NetworkType::Udp },
		source: None,
		inbound_tag: Arc::from("test"),
		protocol: Protocol::Tuic,
		user: None,
		inbound_port: None,
		inbound_type: None,
	}
}

fn forwarded(action: &RouteAction) -> Option<&str> {
	match action {
		RouteAction::Forward(name) => Some(name.as_str()),
		RouteAction::Reject(_) => None,
	}
}

fn engine(rules: &[&str], default: &str) -> AclEngine {
	AclEngine::builder(default).clash_rules(rules).unwrap().build().unwrap()
}

#[tokio::test]
async fn dst_port_match() {
	let engine = engine(&["DST-PORT,443,proxy"], "direct");

	let hit = engine.route(&fc(&domain("example.com", 443), true)).await.unwrap();
	assert_eq!(forwarded(&hit), Some("proxy"));

	let miss = engine.route(&fc(&domain("example.com", 80), true)).await.unwrap();
	assert_eq!(forwarded(&miss), Some("direct"));
}

#[tokio::test]
async fn dst_port_range() {
	let engine = engine(&["DST-PORT,8000-9000,proxy"], "direct");

	let hit = engine.route(&fc(&domain("example.com", 8080), true)).await.unwrap();
	assert_eq!(forwarded(&hit), Some("proxy"));

	let miss = engine.route(&fc(&domain("example.com", 80), true)).await.unwrap();
	assert_eq!(forwarded(&miss), Some("direct"));
}

#[tokio::test]
async fn first_match_wins() {
	let engine = engine(
		&[
			"DOMAIN-SUFFIX,google.com,first",
			"DOMAIN-SUFFIX,google.com,second",
			"MATCH,last",
		],
		"default",
	);

	let action = engine.route(&fc(&domain("www.google.com", 443), true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("first"));
}

#[tokio::test]
async fn match_all_catchall() {
	let engine = engine(&["DOMAIN,specific.com,specific", "MATCH,catchall"], "default");

	let action = engine.route(&fc(&domain("random.org", 80), true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("catchall"));
}

#[tokio::test]
async fn and_compound() {
	let engine = engine(&["AND,((DOMAIN-SUFFIX,example.com),(DST-PORT,443)),secure_proxy"], "direct");

	// Both match → secure_proxy
	let both = engine.route(&fc(&domain("www.example.com", 443), true)).await.unwrap();
	assert_eq!(forwarded(&both), Some("secure_proxy"));

	// Domain matches but port doesn't → direct
	let bad_port = engine.route(&fc(&domain("www.example.com", 80), true)).await.unwrap();
	assert_eq!(forwarded(&bad_port), Some("direct"));

	// Port matches but domain doesn't → direct
	let bad_domain = engine.route(&fc(&domain("other.org", 443), true)).await.unwrap();
	assert_eq!(forwarded(&bad_domain), Some("direct"));
}

#[tokio::test]
async fn or_compound() {
	let engine = engine(&["OR,((DOMAIN,a.com),(DOMAIN,b.com)),proxy"], "direct");

	assert_eq!(
		forwarded(&engine.route(&fc(&domain("a.com", 80), true)).await.unwrap()),
		Some("proxy")
	);
	assert_eq!(
		forwarded(&engine.route(&fc(&domain("b.com", 80), true)).await.unwrap()),
		Some("proxy")
	);
	assert_eq!(
		forwarded(&engine.route(&fc(&domain("c.com", 80), true)).await.unwrap()),
		Some("direct")
	);
}

#[tokio::test]
async fn not_compound() {
	let engine = engine(&["NOT,((DOMAIN-SUFFIX,internal.corp)),proxy"], "direct");

	// Doesn't match suffix → NOT succeeds → proxy
	let external = engine.route(&fc(&domain("external.com", 80), true)).await.unwrap();
	assert_eq!(forwarded(&external), Some("proxy"));

	// Matches suffix → NOT fails → direct
	let internal = engine.route(&fc(&domain("app.internal.corp", 80), true)).await.unwrap();
	assert_eq!(forwarded(&internal), Some("direct"));
}

#[tokio::test]
async fn src_ip_cidr_no_match_without_source_context() {
	// The engine has no source IP on this context, so SRC-IP-CIDR cannot
	// match and the default wins — same behaviour as the legacy router.
	let engine = engine(&["SRC-IP-CIDR,192.168.0.0/16,local"], "default");

	let action = engine.route(&fc(&domain("example.com", 80), true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("default"));
}

#[tokio::test]
async fn domain_and_port_combination() {
	let engine = engine(
		&[
			"AND,((DOMAIN-SUFFIX,api.example.com),(DST-PORT,8443)),api_proxy",
			"DOMAIN-SUFFIX,example.com,web_proxy",
			"MATCH,direct",
		],
		"direct",
	);

	// api.example.com:8443 → api_proxy (first rule)
	let api = engine.route(&fc(&domain("api.example.com", 8443), true)).await.unwrap();
	assert_eq!(forwarded(&api), Some("api_proxy"));

	// api.example.com:443 → web_proxy (second rule)
	let web = engine.route(&fc(&domain("api.example.com", 443), true)).await.unwrap();
	assert_eq!(forwarded(&web), Some("web_proxy"));

	// other.org:80 → direct (MATCH)
	let other = engine.route(&fc(&domain("other.org", 80), true)).await.unwrap();
	assert_eq!(forwarded(&other), Some("direct"));
}

#[tokio::test]
async fn network_type_filter() {
	let engine = engine(&["NETWORK,tcp,proxy", "NETWORK,udp,direct"], "fallback");

	let tcp = engine.route(&fc(&domain("any.com", 443), true)).await.unwrap();
	assert_eq!(forwarded(&tcp), Some("proxy"));

	let udp = engine.route(&fc(&domain("any.com", 443), false)).await.unwrap();
	assert_eq!(forwarded(&udp), Some("direct"));
}

/// Outbound names registered with mixed case must survive routing — the
/// reject keywords are the only case-insensitive spellings; everything else
/// is forwarded verbatim so a case-sensitive handler lookup can find it.
#[tokio::test]
async fn forward_with_original_case() {
	let engine = engine(&["DOMAIN-SUFFIX,example.com,Proxy_Out"], "default");

	let action = engine.route(&fc(&domain("foo.example.com", 80), true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("Proxy_Out"));
}

/// Reject keywords are recognised case-insensitively across all three
/// spellings.
#[tokio::test]
async fn reject_keywords_case_insensitive() {
	for kw in ["REJECT", "Reject", "reject", "BLOCK", "Block", "deny", "Deny", "DENY"] {
		let engine = engine(&[&format!("DOMAIN-SUFFIX,blocked.com,{kw}")], "default");
		let action = engine.route(&fc(&domain("a.blocked.com", 443), true)).await.unwrap();
		assert!(
			matches!(action, RouteAction::Reject(_)),
			"keyword {kw:?} must map to RouteAction::Reject"
		);
	}
}

#[tokio::test]
async fn ip_cidr_match_and_miss() {
	let engine = engine(&["IP-CIDR,192.168.0.0/16,lan"], "default");

	let hit = engine.route(&fc(&ipv4("192.168.1.100", 8080), true)).await.unwrap();
	assert_eq!(forwarded(&hit), Some("lan"));

	let miss = engine.route(&fc(&ipv4("10.0.0.1", 8080), true)).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}
