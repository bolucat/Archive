//! Unified per-connection routing context.
//!
//! [`FlowContext`] is the single object that travels from an inbound to the
//! [`Router`](crate::Router) and on to the outbound handler. It carries every
//! piece of connection information the routing rules can see:
//!
//! * the destination [`target`](FlowContext::target) and
//!   [`network`](FlowContext::network) type (replacing the old `route(target,
//!   is_tcp)` pair),
//! * the client [`source`](FlowContext::source) address (for `SRC-IP-CIDR` /
//!   `SRC-PORT` rules),
//! * the [`inbound_tag`](FlowContext::inbound_tag) and
//!   [`inbound_port`](FlowContext::inbound_port) (for `IN-NAME` / `IN-PORT`),
//! * the authenticated [`user`](FlowContext::user) (for `IN-USER`),
//! * the wire [`protocol`](FlowContext::protocol) and
//!   [`inbound_type`](FlowContext::inbound_type) (for `IN-TYPE`).
//!
//! Inbounds construct one per connection; the dispatcher and routers never
//! rebuild it. [`FlowContext::apply_to_match_context`] lowers it into a
//! [`MatchContext`] for rule evaluation, so the target→context reconstruction
//! that used to be duplicated in every router disappears.

use std::{net::SocketAddr, sync::Arc};

use crate::{
	hooks::{Protocol, UserId},
	rule::{InboundType, MatchContext, NetworkType},
	types::TargetAddr,
};

/// Everything the routing layer needs to know about one connection.
///
/// Cheap to clone (`Arc`-backed identity strings) and `Send + Sync`, so it can
/// be moved into tasks and shared across await points.
#[derive(Clone, Debug)]
pub struct FlowContext {
	/// Destination as reported by the inbound. For UDP this is the target of
	/// the session's *first* packet (or an unspecified placeholder until the
	/// dispatcher has seen one).
	pub target: TargetAddr,
	/// Transport type — replaces the historical `is_tcp: bool` parameter.
	pub network: NetworkType,
	/// Client (peer) address, when the inbound knows it.
	pub source: Option<SocketAddr>,
	/// Stable per-inbound identifier used by `IN-NAME` rules. Every inbound
	/// must supply one (protocol-name fallback if not configured).
	pub inbound_tag: Arc<str>,
	/// Wire protocol the connection arrived on.
	pub protocol: Protocol,
	/// Authenticated identity, if the inbound authenticated the client.
	pub user: Option<UserId>,
	/// The inbound's listening port, used by `IN-PORT` rules.
	pub inbound_port: Option<u16>,
	/// Protocol flavour for `IN-TYPE` rules (SOCKS / HTTP); `None` for
	/// protocols that are neither.
	pub inbound_type: Option<InboundType>,
}

impl FlowContext {
	/// `true` for TCP connections (equivalent to the old `is_tcp` argument).
	pub fn is_tcp(&self) -> bool {
		self.network == NetworkType::Tcp
	}

	/// Replace the destination. The dispatcher uses this for UDP sessions,
	/// where the first packet determines the real target.
	pub fn with_target(mut self, target: TargetAddr) -> Self {
		self.target = target;
		self
	}

	/// Lower this context into the rule-evaluation [`MatchContext`].
	///
	/// Fills every connection-derived field (`src_ip`, `src_port`, `dst_*`,
	/// `domain`, `network`, `inbound_*`). The `geoip_lookup` / `asn_lookup` /
	/// `geosite_lookup` closures are owned by the engine (they borrow its
	/// geodata), so callers bind those after this call — see
	/// `wind_acl::AclEngine::do_route` for the canonical wiring.
	///
	/// `inbound_user` is only populated when the [`UserId`] is valid UTF-8,
	/// since `IN-USER` rules match against rule text; binary identities simply
	/// never match `IN-USER`.
	pub fn apply_to_match_context<'a>(&'a self, ctx: &mut MatchContext<'a>) {
		ctx.src_ip = self.source.map(|s| s.ip());
		ctx.src_port = self.source.map(|s| s.port());
		ctx.network = Some(self.network);
		ctx.inbound_port = self.inbound_port;
		ctx.inbound_type = self.inbound_type;
		ctx.inbound_name = Some(&self.inbound_tag);
		ctx.inbound_user = self.user.as_ref().and_then(|u| std::str::from_utf8(u.as_bytes()).ok());

		match &self.target {
			TargetAddr::Domain(d, p) => {
				ctx.domain = Some(d);
				ctx.dst_port = Some(*p);
			}
			TargetAddr::IPv4(ip, p) => {
				ctx.dst_ip = Some(std::net::IpAddr::V4(*ip));
				ctx.dst_port = Some(*p);
			}
			TargetAddr::IPv6(ip, p) => {
				ctx.dst_ip = Some(std::net::IpAddr::V6(*ip));
				ctx.dst_port = Some(*p);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::net::{IpAddr, Ipv4Addr, SocketAddr};

	use super::*;
	use crate::hooks::Protocol;

	fn ctx() -> FlowContext {
		FlowContext {
			target: TargetAddr::Domain("example.com".into(), 443),
			network: NetworkType::Tcp,
			source: Some("192.168.1.5:12345".parse::<SocketAddr>().unwrap()),
			inbound_tag: Arc::from("socks-main"),
			protocol: Protocol::Socks5,
			user: Some(UserId::from("alice")),
			inbound_port: Some(1080),
			inbound_type: Some(InboundType::Socks),
		}
	}

	#[test]
	fn lowers_all_connection_fields() {
		let mut mc = MatchContext::default();
		let c = ctx();
		c.apply_to_match_context(&mut mc);

		assert_eq!(mc.src_ip, Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5))));
		assert_eq!(mc.src_port, Some(12345));
		assert_eq!(mc.dst_ip, None);
		assert_eq!(mc.dst_port, Some(443));
		assert_eq!(mc.domain, Some("example.com"));
		assert_eq!(mc.network, Some(NetworkType::Tcp));
		assert_eq!(mc.inbound_name, Some("socks-main"));
		assert_eq!(mc.inbound_port, Some(1080));
		assert_eq!(mc.inbound_type, Some(InboundType::Socks));
		assert_eq!(mc.inbound_user, Some("alice"));
	}

	#[test]
	fn binary_user_never_matches_in_user() {
		// Non-UTF-8 identities must not leak into `inbound_user` (IN-USER
		// rules compare against rule text).
		let mut c = ctx();
		c.user = Some(UserId::from(vec![0xff, 0xfe, 0x80]));
		let mut mc = MatchContext::default();
		c.apply_to_match_context(&mut mc);
		assert_eq!(mc.inbound_user, None);
	}

	#[test]
	fn with_target_replaces_destination() {
		let c = ctx().with_target(TargetAddr::IPv4(Ipv4Addr::new(8, 8, 8, 8), 53));
		assert_eq!(c.target, TargetAddr::IPv4(Ipv4Addr::new(8, 8, 8, 8), 53));
		assert!(c.is_tcp());

		let udp = FlowContext {
			network: NetworkType::Udp,
			..c
		};
		assert!(!udp.is_tcp());
	}
}
