//! End-to-end routing-decision tests for [`wind_acl::AclEngine`].
//!
//! These exercise the rule pipeline without guards, so no resolver is needed.

use std::{
	net::{Ipv4Addr, Ipv6Addr},
	sync::Arc,
};

use wind_acl::AclEngine;
use wind_core::{FlowContext, RouteAction, Router, hooks::Protocol, types::TargetAddr};
use wind_rule::{NetworkType, RuleParseError};

fn ipv4(addr: &str, port: u16) -> TargetAddr {
	TargetAddr::IPv4(addr.parse::<Ipv4Addr>().unwrap(), port)
}

fn domain(host: &str, port: u16) -> TargetAddr {
	TargetAddr::Domain(host.to_string(), port)
}

/// Minimal [`FlowContext`] for a plain TCP connection with no client metadata.
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

#[tokio::test]
async fn default_fallback_when_no_rule_matches() {
	let engine = AclEngine::builder("direct").build().unwrap();
	let action = engine.route(&fc(&ipv4("8.8.8.8", 443), true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("direct"));
}

#[tokio::test]
async fn clash_domain_suffix_forwards() {
	let engine = AclEngine::builder("direct")
		.clash_rules(["DOMAIN-SUFFIX,google.com,proxy"])
		.unwrap()
		.build()
		.unwrap();

	let hit = engine.route(&fc(&domain("www.google.com", 443), true)).await.unwrap();
	assert_eq!(forwarded(&hit), Some("proxy"));

	let miss = engine.route(&fc(&domain("example.org", 443), true)).await.unwrap();
	assert_eq!(forwarded(&miss), Some("direct"));
}

#[tokio::test]
async fn reject_keyword_rejects() {
	let engine = AclEngine::builder("direct")
		.clash_rules(["IP-CIDR,10.0.0.0/8,REJECT"])
		.unwrap()
		.build()
		.unwrap();

	let action = engine.route(&fc(&ipv4("10.1.2.3", 80), true)).await.unwrap();
	assert!(matches!(action, RouteAction::Reject(_)), "private IP should be rejected");
}

#[tokio::test]
async fn guard_without_resolver_is_build_error() {
	let err = AclEngine::builder("direct")
		.guards(wind_acl::GuardConfig {
			drop_private: true,
			drop_loopback: false,
		})
		.build();
	assert!(err.is_err(), "guard without resolver must fail to build");
}

#[tokio::test]
async fn ipv6_target_routes() {
	let engine = AclEngine::builder("direct")
		.clash_rules(["IP-CIDR6,2001:db8::/32,proxy"])
		.unwrap()
		.build()
		.unwrap();

	let target = TargetAddr::IPv6("2001:db8::1".parse::<Ipv6Addr>().unwrap(), 443);
	let action = engine.route(&fc(&target, true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("proxy"));
}

#[tokio::test]
async fn apernet_acl_compiles_and_matches() {
	// Real Hysteria 2 function-call ACL. `reject` is a rejection keyword; the
	// CIDR and protocol/port forms lower to IP + AND(network, port) rules.
	let engine = AclEngine::builder("direct")
		.apernet_acl_str("reject(10.0.0.0/8)\nproxy(1.1.1.1, tcp/443)")
		.unwrap()
		.build()
		.unwrap();

	// Private destination → rejected by the first ACL rule.
	let priv_action = engine.route(&fc(&ipv4("10.1.2.3", 1234), true)).await.unwrap();
	assert!(matches!(priv_action, RouteAction::Reject(_)));

	// 1.1.1.1:443/tcp → proxy.
	let proxy_action = engine.route(&fc(&ipv4("1.1.1.1", 443), true)).await.unwrap();
	assert_eq!(forwarded(&proxy_action), Some("proxy"));

	// 1.1.1.1:443/udp → no ACL match (tcp-only), falls through to default.
	let udp_action = engine.route(&fc(&ipv4("1.1.1.1", 443), false)).await.unwrap();
	assert_eq!(forwarded(&udp_action), Some("direct"));
}

#[tokio::test]
async fn apernet_rules_precede_clash_rules() {
	// Both an apernet rule and a Clash rule match 1.1.1.1; the apernet rule is
	// evaluated first, so its target ("aclwin") wins.
	let engine = AclEngine::builder("direct")
		.clash_rules(["IP-CIDR,1.1.1.1/32,clashwin"])
		.unwrap()
		.apernet_acl_str("aclwin(1.1.1.1)")
		.unwrap()
		.build()
		.unwrap();

	let action = engine.route(&fc(&ipv4("1.1.1.1", 443), true)).await.unwrap();
	assert_eq!(forwarded(&action), Some("aclwin"));
}

/// F13 end-to-end regression: these exact rule pairs used to build an engine
/// whose empty domain needle silently swallowed the following `REJECT` (the
/// keyword form matched every domain; the suffix form caught trailing-dot
/// hosts). The engine must now refuse to build them at all.
#[test]
fn empty_domain_needles_are_rejected_by_the_engine() {
	// Slices so a case may carry one or two rule lines.
	let cases: &[&[&str]] = &[
		&["DOMAIN-KEYWORD,,DIRECT", "DOMAIN-SUFFIX,evil.test,REJECT"],
		&["DOMAIN-KEYWORD,,REJECT"],
		&["DOMAIN-SUFFIX,,proxy", "DOMAIN-SUFFIX,evil.test,REJECT"],
		&["DOMAIN,,proxy"],
	];

	for rules in cases {
		let err = match AclEngine::builder("direct").clash_rules(*rules) {
			Ok(_) => panic!("an empty domain needle must make the engine build fail: {rules:?}"),
			Err(err) => err,
		};
		assert!(
			err.to_string().contains("non-empty"),
			"error should name the empty value: {err} (rules: {rules:?})"
		);
	}
}

/// The guard must not change matching for valid domain rules: the keyword
/// still rejects only the domains it names, and everything else falls through
/// to the default outbound.
#[tokio::test]
async fn non_empty_keyword_still_rejects_only_its_domains() {
	let engine = AclEngine::builder("direct")
		.clash_rules(["DOMAIN-KEYWORD,ads,REJECT"])
		.unwrap()
		.build()
		.unwrap();

	let blocked = engine
		.route(&fc(&domain("tracker.ads.example.com", 443), true))
		.await
		.unwrap();
	assert!(matches!(blocked, RouteAction::Reject(_)), "keyword 'ads' should reject");

	let allowed = engine.route(&fc(&domain("good.test", 443), true)).await.unwrap();
	assert_eq!(forwarded(&allowed), Some("direct"));
}

/// Parse-level twin of the engine regression: the error must name the rule
/// type so a config author can find the offending line.
#[test]
fn empty_domain_needle_error_names_the_rule_type() {
	let err = wind_acl::syntax::metacubex::parse_rule("DOMAIN-KEYWORD,,REJECT").unwrap_err();
	assert!(matches!(err, RuleParseError::InvalidFormat(_)), "got {err:?}");
	assert!(err.to_string().contains("DOMAIN-KEYWORD"), "got {err}");
}
