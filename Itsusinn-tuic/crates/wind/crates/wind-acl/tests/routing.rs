//! End-to-end routing-decision tests for [`wind_acl::AclEngine`].
//!
//! These exercise the rule pipeline without guards, so no resolver is needed.

use std::net::{Ipv4Addr, Ipv6Addr};

use wind_acl::AclEngine;
use wind_core::{RouteAction, Router, types::TargetAddr};

fn ipv4(addr: &str, port: u16) -> TargetAddr {
	TargetAddr::IPv4(addr.parse::<Ipv4Addr>().unwrap(), port)
}

fn domain(host: &str, port: u16) -> TargetAddr {
	TargetAddr::Domain(host.to_string(), port)
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
	let action = engine.route(&ipv4("8.8.8.8", 443), true).await.unwrap();
	assert_eq!(forwarded(&action), Some("direct"));
}

#[tokio::test]
async fn clash_domain_suffix_forwards() {
	let engine = AclEngine::builder("direct")
		.clash_rules(["DOMAIN-SUFFIX,google.com,proxy"])
		.unwrap()
		.build()
		.unwrap();

	let hit = engine.route(&domain("www.google.com", 443), true).await.unwrap();
	assert_eq!(forwarded(&hit), Some("proxy"));

	let miss = engine.route(&domain("example.org", 443), true).await.unwrap();
	assert_eq!(forwarded(&miss), Some("direct"));
}

#[tokio::test]
async fn reject_keyword_rejects() {
	let engine = AclEngine::builder("direct")
		.clash_rules(["IP-CIDR,10.0.0.0/8,REJECT"])
		.unwrap()
		.build()
		.unwrap();

	let action = engine.route(&ipv4("10.1.2.3", 80), true).await.unwrap();
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
	let action = engine.route(&target, true).await.unwrap();
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
	let priv_action = engine.route(&ipv4("10.1.2.3", 1234), true).await.unwrap();
	assert!(matches!(priv_action, RouteAction::Reject(_)));

	// 1.1.1.1:443/tcp → proxy.
	let proxy_action = engine.route(&ipv4("1.1.1.1", 443), true).await.unwrap();
	assert_eq!(forwarded(&proxy_action), Some("proxy"));

	// 1.1.1.1:443/udp → no ACL match (tcp-only), falls through to default.
	let udp_action = engine.route(&ipv4("1.1.1.1", 443), false).await.unwrap();
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

	let action = engine.route(&ipv4("1.1.1.1", 443), true).await.unwrap();
	assert_eq!(forwarded(&action), Some("aclwin"));
}
