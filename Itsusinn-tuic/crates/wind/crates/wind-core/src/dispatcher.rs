//! Generic dispatcher: sits between inbound and outbound.
//!
//! The dispatcher receives every inbound connection from an [`InboundCallback`]
//! implementation, evaluates routing rules via a user-supplied [`Router`], and
//! hands the connection off to the matching [`Outbound`] handler.
//!
//! # Design
//!
//! * [`Router`] – an **async** trait that inspects the destination and returns
//!   a [`RouteAction`].  Implementations live in the application crate (e.g.
//!   `tuic-server`) where ACL rules and outbound configs are known.
//! * [`Outbound`] – an **object-safe** trait representing a concrete outbound
//!   handler (direct, socks5, …).  Handlers are keyed by name string.
//! * [`Dispatcher`] – wraps a router and a map of named handlers, and
//!   implements [`InboundCallback`] so it can be passed directly to
//!   `inbound.listen()`.

use std::{collections::HashMap, future::Future, sync::Arc};

use tracing::Instrument;

use crate::{AppContext, InboundCallback, flow::FlowContext, outbound::Outbound, tcp::AbstractTcpStream, udp::UdpStream};

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
	/// Classify a TCP or UDP connection from its full [`FlowContext`].
	fn route(&self, ctx: &FlowContext) -> impl Future<Output = eyre::Result<RouteAction>> + Send;
}

/// Routes inbound connections to named [`Outbound`] handlers.
///
/// # Construction
///
/// ```ignore
/// let mut dispatcher = Dispatcher::new(my_router);
/// dispatcher.add_handler("default", Arc::new(DirectOutbound::new(/* ... */)));
/// dispatcher.add_handler("socks5", Arc::new(SocksOutbound::new(/* ... */)));
/// ```
///
/// Then pass `dispatcher` (or `dispatcher.clone()`) to `inbound.listen()`.
pub struct Dispatcher<R: Router> {
	router: Arc<R>,
	handlers: Arc<HashMap<String, Arc<dyn Outbound>>>,
	/// Owner of the dispatcher's internal tasks. Without one, dispatcher-local
	/// helper tasks are detached (see [`Dispatcher::context`]).
	context: Arc<AppContext>,
}

impl<R: Router> Dispatcher<R> {
	/// Create a new dispatcher with the given router and no handlers yet.
	pub fn new(router: R) -> Self {
		Self {
			router: Arc::new(router),
			handlers: Arc::new(HashMap::new()),
			context: Arc::new(AppContext::default()),
		}
	}

	/// Attach the application context that owns this dispatcher's internal
	/// tasks, so graceful shutdown drains them like any other task.
	///
	/// Pass the same context the inbound driving this dispatcher was built
	/// with. Without this call the dispatcher owns a private context whose
	/// tasks are never drained by `App::run`; they are still all
	/// self-terminating, so this is a shutdown-timing gap rather than a leak.
	pub fn context(mut self, ctx: Arc<AppContext>) -> Self {
		self.context = ctx;
		self
	}

	/// Register a named outbound handler.
	///
	/// Call this before passing the dispatcher to an inbound.  The name
	/// `"default"` is used as the fallback when the router returns a name that
	/// is not otherwise registered.
	pub fn add_handler(&mut self, name: impl Into<String>, handler: Arc<dyn Outbound>) {
		Arc::make_mut(&mut self.handlers).insert(name.into(), handler);
	}

	/// Look up a handler by name, falling back to `"default"` if the exact
	/// name is not registered.
	fn resolve_handler(&self, name: &str) -> Option<Arc<dyn Outbound>> {
		self.handlers.get(name).or_else(|| self.handlers.get("default")).cloned()
	}
}

impl<R: Router> Clone for Dispatcher<R> {
	fn clone(&self) -> Self {
		Self {
			router: self.router.clone(),
			handlers: self.handlers.clone(),
			context: self.context.clone(),
		}
	}
}

impl<R: Router> InboundCallback for Dispatcher<R> {
	fn handle_tcpstream(
		&self,
		ctx: FlowContext,
		stream: impl AbstractTcpStream + 'static,
	) -> impl Future<Output = eyre::Result<()>> + Send {
		let span = tracing::debug_span!("dispatch_tcp", target = %ctx.target);
		async move { self.dispatch_tcp(ctx, stream).instrument(span).await }
	}

	#[allow(clippy::manual_async_fn)]
	fn handle_udpstream(&self, ctx: FlowContext, udp_stream: UdpStream) -> impl Future<Output = eyre::Result<()>> + Send {
		async move {
			self.dispatch_udp(ctx, udp_stream)
				.instrument(tracing::debug_span!("dispatch_udp"))
				.await
		}
	}
}

impl<R: Router> Dispatcher<R> {
	async fn dispatch_tcp(&self, ctx: FlowContext, stream: impl AbstractTcpStream + 'static) -> eyre::Result<()> {
		let action = self.router.route(&ctx).await?;

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

				handler.handle_tcp(ctx, Box::new(stream)).await
			}
		}
	}

	async fn dispatch_udp(&self, ctx: FlowContext, udp_stream: UdpStream) -> eyre::Result<()> {
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
		let Some(first) = rx.recv().await else {
			return Ok(()); // remote closed before sending anything
		};

		// The inbound could not know the destination before the first packet,
		// so the real target is stamped in here.
		let ctx = ctx.with_target(first.target.clone());

		let action = self.router.route(&ctx).await?;

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
				//
				// The replayer is spawned on the dispatcher's [`AppContext`]
				// so graceful shutdown awaits it: a detached task would keep
				// the session's upstream receiver and buffers alive past the
				// drain. It still self-terminates as soon as the handler drops
				// its receiver — either the next `send` fails, or the `closed`
				// arm fires while the upstream is idle — so the drain cannot
				// block on it indefinitely.
				let (proxy_tx, proxy_rx) = tokio::sync::mpsc::channel(32);
				self.context.tasks.spawn(async move {
					if proxy_tx.send(first).await.is_err() {
						return;
					}
					loop {
						let packet = tokio::select! {
							packet = rx.recv() => packet,
							_ = proxy_tx.closed() => None,
						};
						let Some(packet) = packet else {
							break;
						};
						if proxy_tx.send(packet).await.is_err() {
							break;
						}
					}
				});

				let routed_stream = UdpStream { tx, rx: proxy_rx };
				handler.handle_udp(ctx, routed_stream).await
			}
		}
	}
}

// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicBool, Ordering};

	use async_trait::async_trait;
	use tokio::sync::oneshot;

	use super::*;
	use crate::{hooks::Protocol, rule::NetworkType, types::TargetAddr};

	/// A [`Router`] stub that always returns a fixed action.
	struct StubRouter {
		action: RouteAction,
	}

	impl StubRouter {
		fn new(action: RouteAction) -> Self {
			Self { action }
		}
	}

	impl Router for StubRouter {
		fn route(&self, _ctx: &FlowContext) -> impl Future<Output = eyre::Result<RouteAction>> + Send {
			let action = self.action.clone();
			async move { Ok(action) }
		}
	}

	/// Build a minimal [`FlowContext`] for routing tests.
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

	/// UDP placeholder context — the dispatcher stamps the real target from
	/// the first packet.
	fn fc_udp() -> FlowContext {
		fc(&TargetAddr::IPv4(std::net::Ipv4Addr::UNSPECIFIED, 0), false)
	}

	/// A trivial `Outbound` that just records whether it was called.
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
	impl Outbound for MockHandler {
		async fn handle_tcp(&self, _ctx: FlowContext, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
			self.tcp_called.store(true, Ordering::Relaxed);
			Ok(())
		}

		async fn handle_udp(&self, _ctx: FlowContext, _stream: UdpStream) -> eyre::Result<()> {
			self.udp_called.store(true, Ordering::Relaxed);
			Ok(())
		}
	}

	#[tokio::test]
	async fn dispatcher_routes_tcp_to_correct_handler() {
		let proxy_handler = Arc::new(MockHandler::new());
		let default_handler = Arc::new(MockHandler::new());

		let mut dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Forward("proxy_out".into())));
		dispatcher.add_handler("proxy_out", proxy_handler.clone());
		dispatcher.add_handler("default", default_handler.clone());

		let (client, _server) = tokio::io::duplex(1024);

		let target = TargetAddr::Domain("app.proxy.me".into(), 443);
		dispatcher.handle_tcpstream(fc(&target, true), client).await.unwrap();

		assert!(proxy_handler.tcp_called.load(Ordering::Relaxed));
		assert!(!default_handler.tcp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_routes_udp_to_correct_handler() {
		let handler = Arc::new(MockHandler::new());

		let mut dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Forward("relay".into())));
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

		dispatcher.handle_udpstream(fc_udp(), stream).await.unwrap();
		assert!(handler.udp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_rejects_connection() {
		let mut dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Reject("blocked by test".into())));
		dispatcher.add_handler("default", Arc::new(MockHandler::new()));

		let (client, _server) = tokio::io::duplex(1024);
		let target = TargetAddr::Domain("blocked.com".into(), 80);

		let result = dispatcher.handle_tcpstream(fc(&target, true), client).await;
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("rejected"));
	}

	#[tokio::test]
	async fn dispatcher_fallback_to_default_handler() {
		let default_handler = Arc::new(MockHandler::new());
		let mut dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Forward("special_out".into())));
		dispatcher.add_handler("default", default_handler.clone());

		let (client, _server) = tokio::io::duplex(1024);
		let target = TargetAddr::Domain("other.com".into(), 80);
		dispatcher.handle_tcpstream(fc(&target, true), client).await.unwrap();

		assert!(default_handler.tcp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_unknown_handler_falls_back_to_default() {
		// Router returns a name that isn't registered — should fall back to
		// "default"
		let default_handler = Arc::new(MockHandler::new());
		let mut dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Forward("nonexistent_handler".into())));
		dispatcher.add_handler("default", default_handler.clone());

		let (client, _server) = tokio::io::duplex(1024);
		let target = TargetAddr::Domain("any.com".into(), 80);
		dispatcher.handle_tcpstream(fc(&target, true), client).await.unwrap();

		assert!(default_handler.tcp_called.load(Ordering::Relaxed));
	}

	#[tokio::test]
	async fn dispatcher_no_handler_returns_error() {
		// No handlers registered at all — should error
		let dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Forward("missing".into())));

		let (client, _server) = tokio::io::duplex(1024);
		let result = dispatcher
			.handle_tcpstream(fc(&TargetAddr::Domain("a.com".into(), 80), true), client)
			.await;
		assert!(result.is_err());
	}

	/// Signals the test once the replayed first packet arrives, then keeps the
	/// session open until the test flips `open` to `false`. That gives the
	/// replay task a well-defined end: it stops when this handler drops its
	/// receiver.
	struct SignalThenEnd {
		seen: std::sync::Mutex<Option<oneshot::Sender<()>>>,
		open: Arc<AtomicBool>,
	}

	#[async_trait]
	impl Outbound for SignalThenEnd {
		async fn handle_tcp(&self, _ctx: FlowContext, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
			Ok(())
		}

		async fn handle_udp(&self, _ctx: FlowContext, mut stream: UdpStream) -> eyre::Result<()> {
			let packet = stream.rx.recv().await.expect("the replay task must deliver the first packet");
			assert_eq!(
				packet.target,
				TargetAddr::Domain("drain.example".into(), 53),
				"the replayed packet must be the one the session was routed on"
			);
			if let Some(seen) = self.seen.lock().unwrap().take() {
				let _ = seen.send(());
			}
			while self.open.load(Ordering::Relaxed) {
				tokio::time::sleep(std::time::Duration::from_millis(5)).await;
			}
			// Dropping `stream` here closes the relay channel, which is what
			// tells the replay task to finish.
			Ok(())
		}
	}

	/// The UDP first-packet replayer must be registered on the dispatcher's
	/// `AppContext`, so graceful shutdown drains it instead of leaving a
	/// detached task holding the session's upstream receiver and buffers.
	///
	/// A bare `tokio::spawn` leaves the replayer invisible to the tracker, so
	/// the drain would complete while the session is still live.
	#[tokio::test]
	async fn udp_replay_task_participates_in_the_shutdown_drain() {
		let ctx = Arc::new(crate::AppContext::default());
		let open = Arc::new(AtomicBool::new(true));
		let (seen_tx, seen_rx) = oneshot::channel();

		let mut dispatcher = Dispatcher::new(StubRouter::new(RouteAction::Forward("relay".into()))).context(ctx.clone());
		dispatcher.add_handler(
			"relay",
			Arc::new(SignalThenEnd {
				seen: std::sync::Mutex::new(Some(seen_tx)),
				open: open.clone(),
			}),
		);

		let (tx, _rx) = tokio::sync::mpsc::channel(1);
		let (tx2, rx2) = tokio::sync::mpsc::channel(1);
		tx2.send(crate::udp::UdpPacket {
			source: None,
			target: TargetAddr::Domain("drain.example".into(), 53),
			payload: bytes::Bytes::new(),
		})
		.await
		.unwrap();
		let stream = UdpStream { tx, rx: rx2 };

		let dispatch = tokio::spawn({
			let dispatcher = dispatcher.clone();
			async move { dispatcher.handle_udpstream(fc_udp(), stream).await }
		});
		tokio::time::timeout(std::time::Duration::from_secs(5), seen_rx)
			.await
			.expect("the handler never saw the replayed first packet")
			.expect("the handler dropped its signal");

		// The session is live, so a drain must still be waiting on the
		// replayer.
		assert!(
			!ctx.tasks.is_empty(),
			"the UDP replay task must be registered on the app's TaskTracker"
		);
		ctx.tasks.close();
		assert!(
			tokio::time::timeout(std::time::Duration::from_secs(5), ctx.tasks.wait())
				.await
				.is_err(),
			"the drain must await the replay task while the session is still live"
		);

		// End the session; the same drain must now complete and leave nothing
		// behind.
		open.store(false, Ordering::Relaxed);
		assert!(
			tokio::time::timeout(std::time::Duration::from_secs(5), ctx.tasks.wait())
				.await
				.is_ok(),
			"the drain must finish once the session's handler is gone"
		);
		assert!(ctx.tasks.is_empty(), "no replay task may be left behind after the drain");
		assert!(
			tokio::time::timeout(std::time::Duration::from_secs(5), dispatch)
				.await
				.expect("dispatch_udp did not return after the session ended")
				.expect("dispatch task panicked")
				.is_ok(),
			"the UDP session must end without an error"
		);
	}
}
