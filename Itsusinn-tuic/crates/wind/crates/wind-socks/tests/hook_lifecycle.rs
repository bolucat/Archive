//! Connection-hook lifecycle tests for the SOCKS5 inbound.
//!
//! The inbound reports connection lifecycle through `ConnectionHooks`. This
//! test drives the real handshake over loopback and then cancels the inbound
//! while the session is parked mid-negotiation — the case graceful shutdown
//! produces — and asserts the disconnect hook still fires exactly once, with
//! the identity the session had already authenticated as.

use std::{
	net::SocketAddr,
	sync::{Arc, Mutex},
	time::Duration,
};

use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::TcpStream,
	sync::Notify,
};
use tokio_util::sync::CancellationToken;
use wind_core::{
	AbstractInbound, ConnInfo, ConnectDecision, Dispatcher, FlowContext, Outbound, RouteAction, Router, UserId,
	hooks::{ConnectionHooks, InboundHooks},
	tcp::AbstractTcpStream,
	udp::UdpStream,
};
use wind_socks::inbound::{AuthMode, SocksInbound, SocksInboundOpt};

/// Records the connection lifecycle as `"connect"` / `"disconnect:<user|anon>"`
/// strings so ordering and fan-out can be asserted verbatim.
#[derive(Default)]
struct EventLog {
	events: Mutex<Vec<String>>,
	changed: Notify,
}

impl EventLog {
	fn push(&self, event: String) {
		self.events.lock().unwrap().push(event);
		// `notify_one` (not `notify_waiters`) stores a permit, so a hook that
		// fires before the test starts waiting is not missed.
		self.changed.notify_one();
	}

	fn snapshot(&self) -> Vec<String> {
		self.events.lock().unwrap().clone()
	}

	async fn wait_until(&self, predicate: impl Fn(&[String]) -> bool) -> Vec<String> {
		tokio::time::timeout(Duration::from_secs(5), async {
			loop {
				let notified = self.changed.notified();
				let seen = self.snapshot();
				if predicate(&seen) {
					return seen;
				}
				notified.await;
			}
		})
		.await
		.expect("hook event did not arrive within 5s")
	}
}

struct RecordingHooks {
	log: Arc<EventLog>,
}

#[async_trait::async_trait]
impl ConnectionHooks for RecordingHooks {
	async fn on_connect(&self, _info: &ConnInfo) -> ConnectDecision {
		self.log.push("connect".to_string());
		ConnectDecision::Accept
	}

	async fn on_disconnect(&self, _info: &ConnInfo, user: Option<&UserId>) {
		let who = user
			.map(|u| String::from_utf8_lossy(u.as_bytes()).into_owned())
			.unwrap_or_else(|| "anon".to_string());
		self.log.push(format!("disconnect:{who}"));
	}
}

/// Router that forwards everything to the `"default"` outbound handler.
struct ForwardRouter;

impl Router for ForwardRouter {
	#[allow(clippy::manual_async_fn)]
	fn route(&self, _ctx: &FlowContext) -> impl std::future::Future<Output = eyre::Result<RouteAction>> + Send {
		async { Ok(RouteAction::Forward("default".to_string())) }
	}
}

/// Outbound that parks forever: the session never finishes on its own, so the
/// test controls when it ends (cancellation).
struct ParkOutbound;

#[async_trait::async_trait]
impl Outbound for ParkOutbound {
	async fn handle_tcp(&self, _ctx: FlowContext, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		std::future::pending::<()>().await;
		Ok(())
	}

	async fn handle_udp(&self, _ctx: FlowContext, _udp_stream: UdpStream) -> eyre::Result<()> {
		std::future::pending::<()>().await;
		Ok(())
	}
}

/// Spawn the inbound on a free loopback port with recording hooks. Returns the
/// bound address, the cancel token, the event log, and the listen-loop handle.
async fn spawn_inbound(
	auth: AuthMode,
) -> (
	SocketAddr,
	CancellationToken,
	Arc<EventLog>,
	tokio::task::JoinHandle<eyre::Result<()>>,
) {
	let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
	let addr = probe.local_addr().unwrap();
	drop(probe);

	let log = Arc::new(EventLog::default());
	let hooks = InboundHooks {
		connection: Some(Arc::new(RecordingHooks { log: log.clone() })),
		..Default::default()
	};
	let opts = SocksInboundOpt {
		listen_addr: addr,
		public_addr: None,
		auth,
		skip_auth: false,
		allow_udp: false,
		inbound_tag: "test-socks".into(),
		hooks,
		bound_addr: None,
	};
	let cancel = CancellationToken::new();
	let mut dispatcher = Dispatcher::new(ForwardRouter);
	dispatcher.add_handler("default", Arc::new(ParkOutbound));
	let inbound = SocksInbound::new(opts, cancel.clone());
	let handle = tokio::spawn(async move { inbound.listen(&dispatcher).await });

	tokio::time::sleep(Duration::from_millis(200)).await;
	(addr, cancel, log, handle)
}

/// RFC 1929 sub-negotiation; returns the status byte (0x00 = authenticated).
async fn negotiate_password(s: &mut TcpStream, user: &str, pass: &str) -> u8 {
	s.write_all(&[0x05, 0x01, 0x02]).await.unwrap();
	let mut method = [0u8; 2];
	s.read_exact(&mut method).await.unwrap();
	assert_eq!(method, [0x05, 0x02], "server must select username/password auth");

	let mut req = vec![0x01, user.len() as u8];
	req.extend_from_slice(user.as_bytes());
	req.push(pass.len() as u8);
	req.extend_from_slice(pass.as_bytes());
	s.write_all(&req).await.unwrap();

	let mut status = [0u8; 2];
	s.read_exact(&mut status).await.unwrap();
	status[1]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_mid_session_still_reports_the_authenticated_disconnect() {
	let (proxy, cancel, log, handle) = spawn_inbound(AuthMode::Password {
		username: "alice".into(),
		password: "s3cret".into(),
	})
	.await;

	let mut s = TcpStream::connect(proxy).await.unwrap();
	assert_eq!(negotiate_password(&mut s, "alice", "s3cret").await, 0x00, "auth must succeed");
	// The session is now parked reading the command request: authenticated but
	// with no CONNECT yet. This is the state graceful shutdown has to abort.
	log.wait_until(|seen| seen.iter().any(|e| e == "connect")).await;

	cancel.cancel();

	let res = tokio::time::timeout(Duration::from_secs(5), handle)
		.await
		.expect("listen loop did not exit within 5s of cancellation")
		.expect("listen task panicked");
	assert!(res.is_ok(), "listen returned an error on shutdown: {:?}", res.err());

	let events = log.snapshot();
	assert_eq!(
		events,
		vec!["connect".to_string(), "disconnect:alice".to_string()],
		"the cancelled session must still report exactly one disconnect, with the identity it authenticated as"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_before_authentication_reports_an_anonymous_disconnect() {
	let (proxy, cancel, log, handle) = spawn_inbound(AuthMode::NoAuth).await;

	// Connect but never negotiate: the session is aborted pre-auth.
	let _client = TcpStream::connect(proxy).await.unwrap();
	log.wait_until(|seen| seen.iter().any(|e| e == "connect")).await;

	cancel.cancel();

	let res = tokio::time::timeout(Duration::from_secs(5), handle)
		.await
		.expect("listen loop did not exit within 5s of cancellation")
		.expect("listen task panicked");
	assert!(res.is_ok(), "listen returned an error on shutdown: {:?}", res.err());

	let events = log.snapshot();
	assert_eq!(
		events,
		vec!["connect".to_string(), "disconnect:anon".to_string()],
		"a session aborted before authenticating must report an anonymous disconnect"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_connection_reports_only_the_connect_hook() {
	struct Rejecter;
	#[async_trait::async_trait]
	impl ConnectionHooks for Rejecter {
		async fn on_connect(&self, _info: &ConnInfo) -> ConnectDecision {
			ConnectDecision::Reject("blocked".into())
		}

		async fn on_disconnect(&self, _info: &ConnInfo, user: Option<&UserId>) {
			unreachable!("pre-auth rejection must not report a disconnect, got {user:?}");
		}
	}

	let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
	let addr = probe.local_addr().unwrap();
	drop(probe);

	let opts = SocksInboundOpt {
		listen_addr: addr,
		public_addr: None,
		auth: AuthMode::NoAuth,
		skip_auth: false,
		allow_udp: false,
		inbound_tag: "test-socks".into(),
		hooks: InboundHooks {
			connection: Some(Arc::new(Rejecter)),
			..Default::default()
		},
		bound_addr: None,
	};
	let cancel = CancellationToken::new();
	let mut dispatcher = Dispatcher::new(ForwardRouter);
	dispatcher.add_handler("default", Arc::new(ParkOutbound));
	let inbound = SocksInbound::new(opts, cancel.clone());
	let handle = tokio::spawn(async move { inbound.listen(&dispatcher).await });
	tokio::time::sleep(Duration::from_millis(200)).await;

	let _client = TcpStream::connect(addr).await.unwrap();
	tokio::time::sleep(Duration::from_millis(300)).await;
	cancel.cancel();

	let res = tokio::time::timeout(Duration::from_secs(5), handle)
		.await
		.expect("listen loop did not exit within 5s of cancellation")
		.expect("listen task panicked");
	assert!(res.is_ok(), "listen returned an error on shutdown: {:?}", res.err());
}
