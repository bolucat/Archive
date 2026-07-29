//! Generic dispatcher: sits between inbound and outbound.
//!
//! The dispatcher receives every inbound connection from an [`InboundCallback`]
//! implementation, evaluates routing rules via a user-supplied [`Router`], and
//! hands the connection off to the matching [`OutboundAction`] handler.
//!
//! # Design
//!
//! * [`Router`] – an **async** trait that inspects the destination and returns
//!   a [`RouteAction`].  Implementations live in the application crate (e.g.
//!   `tuic-server`) where ACL rules and outbound configs are known.
//! * [`OutboundAction`] – an **object-safe** trait representing a concrete
//!   outbound handler (direct, socks5, …).  Handlers are keyed by name string.
//! * [`Dispatcher`] – wraps a router and a map of named handlers, and
//!   implements [`InboundCallback`] so it can be passed directly to
//!   `inbound.listen()`.

use std::{collections::HashMap, future::Future, net::IpAddr, pin::Pin, sync::Arc};

use async_trait::async_trait;
use tracing::Instrument;

use crate::{
	InboundCallback,
	rule::{MatchContext, NetworkType, Rule},
	tcp::AbstractTcpStream,
	types::TargetAddr,
	udp::UdpStream,
};

/// Boxed future alias used throughout this module.
///
/// Both `Send` and `Sync` are required so the future satisfies the
/// `FutResult` alias used by `InboundCallback`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Decision returned by a [`Router`].
#[derive(Debug, Clone)]
pub enum RouteAction {
	/// Reject the connection (drop it with an optional reason).
	Reject(String),
	/// Forward to the named outbound handler.
	///
	/// The name must match a key previously registered via
	/// [`Dispatcher::add_handler`] (or `"default"` which is always tried as a
	/// fallback).
	Forward(String),
}

/// Determines which outbound handler should serve a connection.
///
/// Implementations are free to perform DNS resolution, consult ACL tables, or
/// apply any other policy.
pub trait Router: Send + Sync + 'static {
	/// Classify a TCP or UDP connection.
	///
	/// * `target` – the destination address as reported by the inbound.
	/// * `is_tcp`  – `true` for TCP streams, `false` for UDP streams.
	fn route(&self, target: &TargetAddr, is_tcp: bool) -> impl Future<Output = eyre::Result<RouteAction>> + Send;
}

/// Object-safe outbound handler.
///
/// Each concrete outbound strategy (direct connect, SOCKS5 proxy, …)
/// implements this trait.  The stream types are erased via trait objects so
/// handlers can be stored in a `HashMap`.
#[async_trait]
pub trait OutboundAction: Send + Sync + 'static {
	/// Handle an inbound TCP stream.
	///
	/// The stream is boxed and `'static` so it can be stored or sent across
	/// tasks.  All concrete `AbstractTcpStream` implementations (owned
	/// `TcpStream`, `Socks5Stream<TcpStream>`, …) satisfy this bound.
	async fn handle_tcp(&self, target: TargetAddr, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()>;

	/// Handle an inbound UDP session.
	async fn handle_udp(&self, stream: UdpStream) -> eyre::Result<()>;
}

/// Routes inbound connections to named outbound handlers.
///
/// # Construction
///
/// ```ignore
/// let mut dispatcher = Dispatcher::new(my_router);
/// dispatcher.add_handler("default", Arc::new(DirectOutbound::new()));
/// dispatcher.add_handler("via_socks5", Arc::new(Socks5Outbound::new("127.0.0.1:1080")));
/// ```
///
/// Then pass `dispatcher` (or `dispatcher.clone()`) to `inbound.listen()`.
pub struct Dispatcher<R: Router> {
	router: Arc<R>,
	handlers: Arc<HashMap<String, Arc<dyn OutboundAction>>>,
}

impl<R: Router> Dispatcher<R> {
	/// Create a new dispatcher with the given router and no handlers yet.
	pub fn new(router: R) -> Self {
		Self {
			router: Arc::new(router),
			handlers: Arc::new(HashMap::new()),
		}
	}

	/// Register a named outbound handler.
	///
	/// Call this before passing the dispatcher to an inbound.  The name
	/// `"default"` is used as the fallback when the router returns a name that
	/// is not otherwise registered.
	pub fn add_handler(&mut self, name: impl Into<String>, handler: Arc<dyn OutboundAction>) {
		Arc::make_mut(&mut self.handlers).insert(name.into(), handler);
	}

	/// Look up a handler by name, falling back to `"default"` if the exact
	/// name is not registered.
	fn resolve_handler(&self, name: &str) -> Option<Arc<dyn OutboundAction>> {
		self.handlers.get(name).or_else(|| self.handlers.get("default")).cloned()
	}
}

impl<R: Router> Clone for Dispatcher<R> {
	fn clone(&self) -> Self {
		Self {
			router: self.router.clone(),
			handlers: self.handlers.clone(),
		}
	}
}

impl<R: Router> InboundCallback for Dispatcher<R> {
	async fn handle_tcpstream(&self, target_addr: TargetAddr, stream: impl AbstractTcpStream + 'static) -> eyre::Result<()> {
		let span = tracing::debug_span!("dispatch_tcp", target = %target_addr);
		self.dispatch_tcp(target_addr, stream).instrument(span).await
	}

	async fn handle_udpstream(&self, udp_stream: UdpStream) -> eyre::Result<()> {
		self.dispatch_udp(udp_stream)
			.instrument(tracing::debug_span!("dispatch_udp"))
			.await
	}
}

impl<R: Router> Dispatcher<R> {
	async fn dispatch_tcp(&self, target_addr: TargetAddr, stream: impl AbstractTcpStream + 'static) -> eyre::Result<()> {
		let action = self.router.route(&target_addr, true).await?;

		match action {
			RouteAction::Reject(reason) => {
				tracing::debug!(reason = %reason, "rejected");
				Err(eyre::eyre!("connection rejected: {}", reason))
			}
			RouteAction::Forward(name) => {
				tracing::debug!(outbound = %name, "forwarding");

				let handler = self
					.resolve_handler(&name)
					.ok_or_else(|| eyre::eyre!("no outbound handler registered for '{}' (and no 'default')", name))?;

				handler.handle_tcp(target_addr, Box::new(stream)).await
			}
		}
	}

	async fn dispatch_udp(&self, udp_stream: UdpStream) -> eyre::Result<()> {
		// Wait for the first packet so the routing decision can be made
		// against the real `packet.target`. The previous implementation
		// routed against a `TargetAddr::IPv4(0.0.0.0, 0)` sentinel, so every
		// UDP session matched the same set of rules regardless of its actual
		// destination — `IP-CIDR`, `DOMAIN-SUFFIX`, `DST-PORT` and friends
		// were effectively no-ops on UDP traffic.
		//
		// Routing is done ONCE on the first packet; subsequent packets follow
		// the same handler. SOCKS5-style sessions that target many remotes in
		// one logical UDP session will all flow through the handler picked
		// for the first packet — this matches Clash behaviour and is a
		// strict improvement over the sentinel.
		let UdpStream { tx, mut rx } = udp_stream;
		let first = match rx.recv().await {
			Some(p) => p,
			None => return Ok(()), // remote closed before sending anything
		};

		let action = self.router.route(&first.target, false).await?;

		match action {
			RouteAction::Reject(reason) => {
				tracing::debug!(reason = %reason, "rejected");
				Err(eyre::eyre!("UDP session rejected: {}", reason))
			}
			RouteAction::Forward(name) => {
				tracing::debug!(outbound = %name, target = %first.target, "forwarding");

				let handler = self
					.resolve_handler(&name)
					.ok_or_else(|| eyre::eyre!("no outbound handler registered for '{}' (and no 'default')", name))?;

				// Rebuild the inbound side of the UdpStream so the handler
				// sees the first packet too. A small proxy channel replays
				// the first packet and then forwards everything else from the
				// original receiver verbatim.
				let (proxy_tx, proxy_rx) = tokio::sync::mpsc::channel(32);
				tokio::spawn(async move {
					if proxy_tx.send(first).await.is_err() {
						return;
					}
					while let Some(pkt) = rx.recv().await {
						if proxy_tx.send(pkt).await.is_err() {
							break;
						}
					}
				});

				let routed_stream = UdpStream { tx, rx: proxy_rx };
				handler.handle_udp(routed_stream).await
			}
		}
	}
}

/// A built-in [`Router`] that evaluates a list of [`Rule`]s in order.
///
/// The first matching rule determines the outbound.  If no rule matches, the
/// configured default outbound is used.
///
/// New code should prefer `wind_acl::AclEngine`, which compiles the same
/// rules to the `wind-acl` IR and additionally supports loopback/private
/// guards.
///
/// Rule targets are mapped to [`RouteAction`] as follows:
///
/// * `"reject"` / `"block"` / `"deny"` (case-insensitive) →
///   [`RouteAction::Reject`]
/// * anything else → [`RouteAction::Forward`] with the target name
///
/// # Example
///
/// ```ignore
/// use wind_core::{Dispatcher, dispatcher::AclRouter};
/// use wind_core::rule::Rule;
///
/// let rules: Vec<Rule> = Rule::parse_rules(r#"
///     DOMAIN-SUFFIX,ads.example.com,reject
///     DOMAIN-SUFFIX,google.com,proxy
///     IP-CIDR,10.0.0.0/8,direct
///     MATCH,proxy
/// "#).into_iter().filter_map(Result::ok).collect();
///
/// let router = AclRouter::new(rules, "direct");
/// let mut dispatcher = Dispatcher::new(router);
/// // dispatcher.add_handler("direct", ...);
/// // dispatcher.add_handler("proxy", ...);
/// ```
pub struct AclRouter {
	rules: Vec<Rule>,
	default_outbound: String,
}

impl AclRouter {
	/// Create a router with the given ordered rules and a fallback outbound
	/// name used when no rule matches.
	pub fn new(rules: Vec<Rule>, default_outbound: impl Into<String>) -> Self {
		Self {
			rules,
			default_outbound: default_outbound.into(),
		}
	}
}

impl Router for AclRouter {
	async fn route(&self, target: &TargetAddr, is_tcp: bool) -> eyre::Result<RouteAction> {
		let span = tracing::trace_span!("acl_route", target = %target, proto = if is_tcp { "tcp" } else { "udp" });
		self.eval_rules(target, is_tcp).instrument(span).await
	}
}

impl AclRouter {
	async fn eval_rules(&self, target: &TargetAddr, is_tcp: bool) -> eyre::Result<RouteAction> {
		let (domain, dst_ip, port) = match target {
			TargetAddr::Domain(d, p) => (Some(d.as_str()), None, *p),
			TargetAddr::IPv4(ip, p) => (None, Some(IpAddr::V4(*ip)), *p),
			TargetAddr::IPv6(ip, p) => (None, Some(IpAddr::V6(*ip)), *p),
		};

		let ctx = MatchContext {
			domain,
			dst_ip,
			dst_port: Some(port),
			network: Some(if is_tcp { NetworkType::Tcp } else { NetworkType::Udp }),
			..Default::default()
		};

		for rule in &self.rules {
			if rule.matches(&ctx) {
				tracing::debug!(rule = %rule, "matched");
				return Ok(rule_target_to_action(&rule.target, rule));
			}
		}

		tracing::debug!(outbound = %self.default_outbound, "no rule matched, using default");
		Ok(RouteAction::Forward(self.default_outbound.clone()))
	}
}

/// Map a rule target string to a [`RouteAction`].
///
/// The reject/block/deny keywords are matched case-insensitively (so
/// `REJECT`, `Reject`, `reject` all work). Everything else is treated as an
/// outbound name and forwarded verbatim — handler lookups in
/// `Dispatcher::resolve_handler` are case-SENSITIVE, so we must NOT lower-case
/// the name. Previously the match arm rebound `name` to the lowercased string
/// and forwarded that, so an outbound registered as `"Proxy_Out"` would
/// silently fall through to the `"default"` handler.
fn rule_target_to_action(target: &str, rule: &Rule) -> RouteAction {
	let lower = target.to_ascii_lowercase();
	match lower.as_str() {
		"reject" | "block" | "deny" => RouteAction::Reject(format!("rejected by rule: {}", rule)),
		_ => RouteAction::Forward(target.to_string()),
	}
}

use crate::AbstractOutbound;

/// Placeholder type used for the `via` parameter when no outbound chaining
/// is desired.
///
/// Calling `handle_tcp` / `handle_udp` on this type will **panic**.
/// It must never be used — it only exists to fill the generic `via`
/// parameter of [`AbstractOutbound`].
pub struct NoChain;

impl AbstractOutbound for NoChain {
	async fn handle_tcp(
		&self,
		_target_addr: TargetAddr,
		_stream: impl crate::tcp::AbstractTcpStream,
		_via: Option<impl AbstractOutbound + Sized + Send>,
	) -> eyre::Result<()> {
		unreachable!("NoChain::handle_tcp should never be called")
	}

	async fn handle_udp(
		&self,
		_stream: crate::udp::UdpStream,
		_via: Option<impl AbstractOutbound + Sized + Send>,
	) -> eyre::Result<()> {
		unreachable!("NoChain::handle_udp should never be called")
	}
}

/// Adapter that wraps any [`AbstractOutbound`] implementation as an
/// [`OutboundAction`] (object-safe, store-able in the dispatcher).
///
/// The `via` chain parameter is filled with [`NoChain`] (panics if called).
pub struct OutboundAsAction<O> {
	pub inner: O,
}

#[async_trait]
impl<O: AbstractOutbound + Send + Sync + 'static> OutboundAction for OutboundAsAction<O> {
	async fn handle_tcp(
		&self,
		target: TargetAddr,
		stream: Box<dyn crate::tcp::AbstractTcpStream + 'static>,
	) -> eyre::Result<()> {
		self.inner.handle_tcp(target, stream, Option::<NoChain>::None).await
	}

	async fn handle_udp(&self, stream: crate::udp::UdpStream) -> eyre::Result<()> {
		self.inner.handle_udp(stream, Option::<NoChain>::None).await
	}
}

#[cfg(test)]
mod tests {
	use std::sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	};

	use super::*;

	fn parse_rules(text: &str) -> Vec<Rule> {
		Rule::parse_rules(text).into_iter().filter_map(Result::ok).collect()
	}

	/// A trivial `OutboundAction` that just records whether it was called.
	struct MockHandler {
		tcp_called: AtomicBool,
		udp_called: AtomicBool,
	}

	impl MockHandler {
		fn new() -> Self {
			Self {
				tcp_called: AtomicBool::new(false),
				udp_called: AtomicBool::new(false),
			}
		}
	}

	#[async_trait]
	impl OutboundAction for MockHandler {
		async fn handle_tcp(&self, _target: TargetAddr, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
			self.tcp_called.store(true, Ordering::Relaxed);
			Ok(())
		}

		async fn handle_udp(&self, _stream: UdpStream) -> eyre::Result<()> {
			self.udp_called.store(true, Ordering::Relaxed);
			Ok(())
		}
	}

	#[tokio::test]
	async fn acl_router_domain_suffix_match() {
		let router = AclRouter::new(parse_rules("DOMAIN-SUFFIX,google.com,proxy"), "direct");

		let target = TargetAddr::Domain("www.google.com".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));
	}

	#[tokio::test]
	async fn acl_router_domain_exact_no_match() {
		let router = AclRouter::new(parse_rules("DOMAIN,example.com,proxy"), "direct");

		let target = TargetAddr::Domain("other.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_ip_cidr_match() {
		let router = AclRouter::new(parse_rules("IP-CIDR,192.168.0.0/16,lan"), "default");

		let target = TargetAddr::IPv4("192.168.1.100".parse().unwrap(), 8080);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "lan"));

		let target = TargetAddr::IPv4("10.0.0.1".parse().unwrap(), 8080);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "default"));
	}

	#[tokio::test]
	async fn acl_router_reject_action() {
		let router = AclRouter::new(parse_rules("DOMAIN-KEYWORD,ads,reject"), "direct");

		let target = TargetAddr::Domain("ads.example.com".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Reject(_)));
	}

	#[tokio::test]
	async fn acl_router_block_and_deny_also_reject() {
		let rules = parse_rules(
			r#"
			DOMAIN,block.com,block
			DOMAIN,deny.com,deny
			"#,
		);
		let router = AclRouter::new(rules, "direct");

		let action = router.route(&TargetAddr::Domain("block.com".into(), 80), true).await.unwrap();
		assert!(matches!(action, RouteAction::Reject(_)));

		let action = router.route(&TargetAddr::Domain("deny.com".into(), 80), true).await.unwrap();
		assert!(matches!(action, RouteAction::Reject(_)));
	}

	#[tokio::test]
	async fn acl_router_network_type_filter() {
		let rules = parse_rules(
			r#"
			NETWORK,tcp,proxy
			NETWORK,udp,direct
			"#,
		);
		let router = AclRouter::new(rules, "fallback");

		let target = TargetAddr::Domain("any.com".into(), 443);

		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));

		let action = router.route(&target, false).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_dst_port_match() {
		let router = AclRouter::new(parse_rules("DST-PORT,443,proxy"), "direct");

		let target = TargetAddr::Domain("example.com".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));

		let target = TargetAddr::Domain("example.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_first_match_wins() {
		let rules = parse_rules(
			r#"
			DOMAIN-SUFFIX,google.com,first
			DOMAIN-SUFFIX,google.com,second
			MATCH,last
			"#,
		);
		let router = AclRouter::new(rules, "default");

		let target = TargetAddr::Domain("www.google.com".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "first"));
	}

	#[tokio::test]
	async fn acl_router_match_all_catchall() {
		let rules = parse_rules(
			r#"
			DOMAIN,specific.com,specific
			MATCH,catchall
			"#,
		);
		let router = AclRouter::new(rules, "default");

		let target = TargetAddr::Domain("random.org".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "catchall"));
	}

	#[tokio::test]
	async fn acl_router_ipv6_target() {
		let router = AclRouter::new(parse_rules("IP-CIDR6,fc00::/7,local"), "default");

		let target = TargetAddr::IPv6("fd12::1".parse().unwrap(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "local"));

		let target = TargetAddr::IPv6("2001:db8::1".parse().unwrap(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "default"));
	}

	#[tokio::test]
	async fn dispatcher_routes_tcp_to_correct_handler() {
		let rules = parse_rules(
			r#"
			DOMAIN-SUFFIX,proxy.me,proxy_out
			MATCH,default
			"#,
		);

		let proxy_handler = Arc::new(MockHandler::new());
		let default_handler = Arc::new(MockHandler::new());

		let mut dispatcher = Dispatcher::new(AclRouter::new(rules, "default"));
		dispatcher.add_handler("proxy_out", proxy_handler.clone());
		dispatcher.add_handler("default", default_handler.clone());

		let (client, _server) = tokio::io::duplex(1024);

		let target = TargetAddr::Domain("app.proxy.me".into(), 443);
		dispatcher.handle_tcpstream(target, client).await.unwrap();

		assert!(proxy_handler.tcp_called.load(Ordering::Relaxed));
		assert!(!default_handler.tcp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_routes_udp_to_correct_handler() {
		let rules = parse_rules("MATCH,relay");

		let handler = Arc::new(MockHandler::new());

		let mut dispatcher = Dispatcher::new(AclRouter::new(rules, "default"));
		dispatcher.add_handler("relay", handler.clone());

		let (tx, _rx) = tokio::sync::mpsc::channel(1);
		let (tx2, rx2) = tokio::sync::mpsc::channel(1);
		let stream = UdpStream { tx, rx: rx2 };

		// `dispatch_udp` now awaits the first packet so it can route by the
		// real target rather than a sentinel — push one in so the routing
		// decision happens, then drop the sender to signal end-of-stream.
		tx2.send(crate::udp::UdpPacket {
			source: None,
			target: TargetAddr::Domain("anywhere.example".into(), 80),
			payload: bytes::Bytes::new(),
		})
		.await
		.unwrap();
		drop(tx2);

		dispatcher.handle_udpstream(stream).await.unwrap();
		assert!(handler.udp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_rejects_connection() {
		let rules = parse_rules("DOMAIN,blocked.com,reject");

		let mut dispatcher = Dispatcher::new(AclRouter::new(rules, "default"));
		dispatcher.add_handler("default", Arc::new(MockHandler::new()));

		let (client, _server) = tokio::io::duplex(1024);
		let target = TargetAddr::Domain("blocked.com".into(), 80);

		let result = dispatcher.handle_tcpstream(target, client).await;
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("rejected"));
	}

	#[tokio::test]
	async fn dispatcher_fallback_to_default_handler() {
		let rules = parse_rules("DOMAIN,special.com,special_out");

		let default_handler = Arc::new(MockHandler::new());
		let mut dispatcher = Dispatcher::new(AclRouter::new(rules, "default"));
		dispatcher.add_handler("default", default_handler.clone());

		let (client, _server) = tokio::io::duplex(1024);
		let target = TargetAddr::Domain("other.com".into(), 80);
		dispatcher.handle_tcpstream(target, client).await.unwrap();

		assert!(default_handler.tcp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_unknown_handler_falls_back_to_default() {
		// Router returns a name that isn't registered — should fall back to "default"
		let rules = parse_rules("MATCH,nonexistent_handler");

		let default_handler = Arc::new(MockHandler::new());
		let mut dispatcher = Dispatcher::new(AclRouter::new(rules, "default"));
		dispatcher.add_handler("default", default_handler.clone());

		let (client, _server) = tokio::io::duplex(1024);
		let target = TargetAddr::Domain("any.com".into(), 80);
		dispatcher.handle_tcpstream(target, client).await.unwrap();

		assert!(default_handler.tcp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_no_handler_returns_error() {
		// No handlers registered at all — should error
		let rules = parse_rules("MATCH,missing");
		let dispatcher = Dispatcher::new(AclRouter::new(rules, "default"));

		let (client, _server) = tokio::io::duplex(1024);
		let result = dispatcher
			.handle_tcpstream(TargetAddr::Domain("a.com".into(), 80), client)
			.await;
		assert!(result.is_err());
	}

	#[tokio::test]
	async fn acl_router_dst_port_range() {
		let router = AclRouter::new(parse_rules("DST-PORT,8000-9000,proxy"), "direct");

		let target = TargetAddr::Domain("example.com".into(), 8080);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));

		let target = TargetAddr::Domain("example.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_and_compound() {
		let rules = parse_rules("AND,((DOMAIN-SUFFIX,example.com),(DST-PORT,443)),secure_proxy");
		let router = AclRouter::new(rules, "direct");

		// Both match → secure_proxy
		let target = TargetAddr::Domain("www.example.com".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "secure_proxy"));

		// Domain matches but port doesn't → direct
		let target = TargetAddr::Domain("www.example.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));

		// Port matches but domain doesn't → direct
		let target = TargetAddr::Domain("other.org".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_or_compound() {
		let rules = parse_rules("OR,((DOMAIN,a.com),(DOMAIN,b.com)),proxy");
		let router = AclRouter::new(rules, "direct");

		let target = TargetAddr::Domain("a.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));

		let target = TargetAddr::Domain("b.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));

		let target = TargetAddr::Domain("c.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_not_compound() {
		let rules = parse_rules("NOT,((DOMAIN-SUFFIX,internal.corp)),proxy");
		let router = AclRouter::new(rules, "direct");

		// Doesn't match suffix → NOT succeeds → proxy
		let target = TargetAddr::Domain("external.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "proxy"));

		// Matches suffix → NOT fails → direct
		let target = TargetAddr::Domain("app.internal.corp".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	#[tokio::test]
	async fn acl_router_src_ip_cidr_no_match_without_context() {
		let router = AclRouter::new(parse_rules("SRC-IP-CIDR,192.168.0.0/16,local"), "default");

		let target = TargetAddr::Domain("example.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		// SRC-IP-CIDR can't match because AclRouter doesn't have src_ip context
		assert!(matches!(action, RouteAction::Forward(name) if name == "default"));
	}

	#[tokio::test]
	async fn acl_router_domain_and_port_combination() {
		let rules = parse_rules(
			r#"
			AND,((DOMAIN-SUFFIX,api.example.com),(DST-PORT,8443)),api_proxy
			DOMAIN-SUFFIX,example.com,web_proxy
			MATCH,direct
			"#,
		);
		let router = AclRouter::new(rules, "direct");

		// api.example.com:8443 → api_proxy (first rule)
		let target = TargetAddr::Domain("api.example.com".into(), 8443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "api_proxy"));

		// api.example.com:443 → web_proxy (second rule)
		let target = TargetAddr::Domain("api.example.com".into(), 443);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "web_proxy"));

		// other.org:80 → direct (MATCH)
		let target = TargetAddr::Domain("other.org".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "direct"));
	}

	/// Outbound names registered with mixed case must survive routing —
	/// previously the `name => RouteAction::Forward(name.to_string())` arm
	/// bound to the lowercased string and silently routed `Proxy_Out` to
	/// `proxy_out`, which `Dispatcher::resolve_handler` failed to find.
	#[tokio::test]
	async fn router_forwards_with_original_case() {
		let router = AclRouter::new(parse_rules("DOMAIN-SUFFIX,example.com,Proxy_Out"), "default");
		let target = TargetAddr::Domain("foo.example.com".into(), 80);
		let action = router.route(&target, true).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "Proxy_Out"));
	}

	/// Reject keywords are still recognised case-insensitively across all
	/// three spellings.
	#[tokio::test]
	async fn router_reject_keywords_case_insensitive() {
		for kw in ["REJECT", "Reject", "reject", "BLOCK", "Block", "deny", "Deny", "DENY"] {
			let r = AclRouter::new(parse_rules(&format!("DOMAIN-SUFFIX,blocked.com,{kw}")), "default");
			let target = TargetAddr::Domain("a.blocked.com".into(), 443);
			let action = r.route(&target, true).await.unwrap();
			assert!(
				matches!(action, RouteAction::Reject(_)),
				"keyword {kw:?} must map to RouteAction::Reject"
			);
		}
	}
}
