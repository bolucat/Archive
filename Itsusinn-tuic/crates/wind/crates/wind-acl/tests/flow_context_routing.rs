//! End-to-end tests for routing rules that depend on the connection context
//! carried by [`wind_core::FlowContext`] — the fields that the old
//! `route(&TargetAddr, is_tcp)` signature could never see.
//!
//! Before `FlowContext`, `SRC-IP-CIDR` / `SRC-PORT` / `IN-NAME` / `IN-USER` /
//! `IN-PORT` / `IN-TYPE` rules silently never matched in the real servers:
//! the route signature carried neither a source address nor inbound/user
//! metadata. These tests pin the behavior now that the context flows through.

use std::{
	net::{Ipv4Addr, SocketAddr},
	sync::Arc,
};

use wind_acl::AclEngine;
use wind_core::{FlowContext, RouteAction, Router, hooks::Protocol, types::TargetAddr};
use wind_rule::{InboundType, NetworkType};

fn build(rules: &[&str]) -> AclEngine {
	AclEngine::builder("default")
		.clash_rules(rules.iter().copied())
		.unwrap()
		.build()
		.unwrap()
}

/// Full context mimicking a SOCKS5 inbound on `127.0.0.1:1080`:
/// client `192.168.1.50:43210`, authenticated as `alice`.
fn socks_ctx(target: &TargetAddr) -> FlowContext {
	FlowContext {
		target: target.clone(),
		network: NetworkType::Tcp,
		source: Some("192.168.1.50:43210".parse::<SocketAddr>().unwrap()),
		inbound_tag: Arc::from("socks-main"),
		protocol: Protocol::Socks5,
		user: Some("alice".into()),
		inbound_port: Some(1080),
		inbound_type: Some(InboundType::Socks),
	}
}

fn forwarded(action: &RouteAction) -> Option<&str> {
	match action {
		RouteAction::Forward(name) => Some(name.as_str()),
		RouteAction::Reject(_) => None,
	}
}

#[tokio::test]
async fn src_ip_cidr_matches_source_address() {
	let engine = build(&["SRC-IP-CIDR,192.168.0.0/16,lan"]);

	// Client inside the source range → lan.
	let hit = engine
		.route(&socks_ctx(&TargetAddr::IPv4(Ipv4Addr::new(1, 1, 1, 1), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("lan"));

	// Client outside the range → default.
	let outside = FlowContext {
		source: Some("10.0.0.9:5555".parse::<SocketAddr>().unwrap()),
		..socks_ctx(&TargetAddr::IPv4(Ipv4Addr::new(1, 1, 1, 1), 443))
	};
	let miss = engine.route(&outside).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}

#[tokio::test]
async fn src_port_matches_source_port() {
	let engine = build(&["SRC-PORT,43210,edge"]);

	let hit = engine
		.route(&socks_ctx(&TargetAddr::IPv4(Ipv4Addr::new(8, 8, 8, 8), 53)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("edge"));

	let other_port = FlowContext {
		source: Some("192.168.1.50:53".parse::<SocketAddr>().unwrap()),
		..socks_ctx(&TargetAddr::IPv4(Ipv4Addr::new(8, 8, 8, 8), 53))
	};
	let miss = engine.route(&other_port).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}

#[tokio::test]
async fn in_name_matches_inbound_tag() {
	let engine = build(&["IN-NAME,socks-main,ss"]);

	let hit = engine
		.route(&socks_ctx(&TargetAddr::Domain("example.com".into(), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("ss"));

	let other_tag = FlowContext {
		inbound_tag: Arc::from("tuic-main"),
		..socks_ctx(&TargetAddr::Domain("example.com".into(), 443))
	};
	let miss = engine.route(&other_tag).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}

#[tokio::test]
async fn in_user_matches_authenticated_user() {
	let engine = build(&["IN-USER,alice,userout"]);

	let hit = engine
		.route(&socks_ctx(&TargetAddr::Domain("example.com".into(), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("userout"));

	let anonymous = FlowContext {
		user: None,
		..socks_ctx(&TargetAddr::Domain("example.com".into(), 443))
	};
	let miss = engine.route(&anonymous).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}

#[tokio::test]
async fn in_port_matches_inbound_listen_port() {
	let engine = build(&["IN-PORT,1080,lan"]);

	let hit = engine
		.route(&socks_ctx(&TargetAddr::Domain("example.com".into(), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("lan"));

	let other_port = FlowContext {
		inbound_port: Some(2080),
		..socks_ctx(&TargetAddr::Domain("example.com".into(), 443))
	};
	let miss = engine.route(&other_port).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}

#[tokio::test]
async fn in_type_matches_socks_but_not_others() {
	let engine = build(&["IN-TYPE,socks,bypass"]);

	// SOCKS5 inbound → bypass.
	let hit = engine
		.route(&socks_ctx(&TargetAddr::Domain("example.com".into(), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("bypass"));

	// TUIC / Naive have no inbound type → default.
	let no_type = FlowContext {
		inbound_type: None,
		..socks_ctx(&TargetAddr::Domain("example.com".into(), 443))
	};
	let miss = engine.route(&no_type).await.unwrap();
	assert_eq!(forwarded(&miss), Some("default"));
}

#[tokio::test]
async fn combined_in_user_and_domain_rule() {
	// Compound rule: authenticated user AND destination domain.
	let engine = build(&["AND,((IN-USER,alice),(DOMAIN-SUFFIX,example.com)),user-web"]);

	let hit = engine
		.route(&socks_ctx(&TargetAddr::Domain("www.example.com".into(), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&hit), Some("user-web"));

	let wrong_domain = engine
		.route(&socks_ctx(&TargetAddr::Domain("other.org".into(), 443)))
		.await
		.unwrap();
	assert_eq!(forwarded(&wrong_domain), Some("default"));
}
