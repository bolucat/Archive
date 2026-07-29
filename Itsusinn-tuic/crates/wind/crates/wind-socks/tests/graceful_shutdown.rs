//! Graceful-shutdown tests for the SOCKS5 inbound accept loop.
//!
//! `SocksInbound::listen` selects on its `CancellationToken`; on cancellation
//! it breaks out of the accept loop, closes its per-connection `TaskTracker`,
//! and waits for in-flight sessions before returning. Each session also holds a
//! child token so a blocked handshake is aborted promptly. These tests confirm
//! the loop returns within a bounded time — both idle and with a live
//! connection — rather than hanging on shutdown.

use std::{net::SocketAddr, time::Duration};

use tokio_util::sync::CancellationToken;
use wind_core::{AbstractInbound, InboundCallback, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream};
use wind_socks::inbound::{AuthMode, SocksInbound, SocksInboundOpt};

/// A callback whose handlers never complete on their own. Forces shutdown to be
/// driven by the cancellation chain (the per-connection child token), not by a
/// session finishing naturally.
#[derive(Clone)]
struct ParkCallback;

impl InboundCallback for ParkCallback {
	async fn handle_tcpstream(&self, _target: TargetAddr, _stream: impl AbstractTcpStream + 'static) -> eyre::Result<()> {
		std::future::pending::<()>().await;
		Ok(())
	}

	async fn handle_udpstream(&self, _udp_stream: UdpStream) -> eyre::Result<()> {
		std::future::pending::<()>().await;
		Ok(())
	}
}

/// Bind a SOCKS inbound on a free loopback port and spawn its `listen` loop,
/// returning the bound address and the loop's join handle.
async fn spawn_inbound(cancel: CancellationToken) -> (SocketAddr, tokio::task::JoinHandle<eyre::Result<()>>) {
	// Reserve a free port without holding the listener.
	let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve port");
	let addr = probe.local_addr().unwrap();
	drop(probe);

	let opts = SocksInboundOpt {
		listen_addr: addr,
		public_addr: None,
		auth: AuthMode::NoAuth,
		skip_auth: false,
		allow_udp: false,
		hooks: Default::default(),
	};
	let inbound = SocksInbound::new(opts, cancel);
	let handle = tokio::spawn(async move {
		let cb = ParkCallback;
		inbound.listen(&cb).await
	});

	// Let the loop bind and reach `accept()`.
	tokio::time::sleep(Duration::from_millis(200)).await;
	(addr, handle)
}

/// An idle inbound must break its accept loop and return `Ok(())` promptly
/// after the token is cancelled.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_listen_loop_exits_on_cancel() {
	let cancel = CancellationToken::new();
	let (_addr, handle) = spawn_inbound(cancel.clone()).await;

	cancel.cancel();

	let res = tokio::time::timeout(Duration::from_secs(5), handle)
		.await
		.expect("listen loop did not exit within 5s of cancellation")
		.expect("listen task panicked");
	assert!(res.is_ok(), "listen returned an error on shutdown: {:?}", res.err());
}

/// With a live connection parked mid-session, cancellation must abort the
/// in-flight session (via its child token), let the per-connection
/// `TaskTracker` drain, and return — not block forever in `conn_tasks.wait()`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn active_session_is_drained_on_cancel() {
	let cancel = CancellationToken::new();
	let (addr, handle) = spawn_inbound(cancel.clone()).await;

	// Open a connection so the inbound spawns a tracked session task. We don't
	// drive the SOCKS handshake — the handler is parked reading from the stream,
	// which is exactly the in-flight state shutdown must abort.
	let _client = tokio::net::TcpStream::connect(addr).await.expect("connect to inbound");
	tokio::time::sleep(Duration::from_millis(200)).await;

	cancel.cancel();

	let res = tokio::time::timeout(Duration::from_secs(5), handle)
		.await
		.expect("listen loop did not drain in-flight session within 5s of cancellation")
		.expect("listen task panicked");
	assert!(res.is_ok(), "listen returned an error on shutdown: {:?}", res.err());
}
