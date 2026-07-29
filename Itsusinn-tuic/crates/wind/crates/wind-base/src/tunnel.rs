//! Tunnel inbounds: TCP/UDP port forwarders as wind-core inbounds.
//!
//! Each tunnel listens on a local address and relays all traffic to a fixed
//! remote target through the dispatcher — same path as any other inbound.
//!
//! Reference: mihomo tunnel-type inbound pattern.

use std::{
	collections::HashMap,
	net::{SocketAddr, TcpListener as StdTcpListener},
	sync::{
		Arc,
		atomic::{AtomicU16, Ordering},
	},
};

use bytes::Bytes;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use tokio::net::{TcpListener, UdpSocket};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, debug, info, warn};
use wind_core::{
	AbstractInbound, InboundCallback,
	types::TargetAddr,
	udp::{UdpPacket, UdpStream},
};

static NEXT_ASSOC_ID: AtomicU16 = AtomicU16::new(0);

fn next_assoc_id() -> u16 {
	0x8000 | (NEXT_ASSOC_ID.fetch_add(1, Ordering::Relaxed) & 0x7fff)
}

// ── TCP tunnel ─────────────────────────────────────────────────────────────

/// TCP port forwarder as a wind-core inbound.
///
/// Each accepted connection is handed to the dispatcher via
/// [`InboundCallback::handle_tcpstream`] with the configured remote target.
pub struct TunnelTcpInbound {
	listen: SocketAddr,
	remote: (String, u16),
	cancel: CancellationToken,
}

impl TunnelTcpInbound {
	pub fn new(listen: SocketAddr, remote: (String, u16), cancel: CancellationToken) -> Self {
		Self { listen, remote, cancel }
	}
}

impl AbstractInbound for TunnelTcpInbound {
	async fn listen(&self, cb: &impl InboundCallback) -> eyre::Result<()> {
		let listener = create_tcp_listener(self.listen)?;
		info!(
			"[tunnel-tcp] listening on {listen} -> {remote:?}",
			listen = self.listen,
			remote = self.remote
		);

		let conn_tasks = tokio_util::task::TaskTracker::new();
		loop {
			tokio::select! {
				_ = self.cancel.cancelled() => {
					info!("[tunnel-tcp] cancellation received, shutting down");
					break;
				}
				res = listener.accept() => match res {
					Ok((stream, peer)) => {
						let cb = cb.clone();
						let target = TargetAddr::Domain(self.remote.0.clone(), self.remote.1);
						let conn_cancel = self.cancel.child_token();
						conn_tasks.spawn(
							async move {
								tokio::select! {
									_ = conn_cancel.cancelled() => {}
									res = cb.handle_tcpstream(target, stream) => {
										if let Err(e) = res {
											warn!("[tunnel-tcp] [{peer}] error: {e}");
										}
									}
								}
							}
							.in_current_span(),
						);
					}
					Err(err) => warn!("[tunnel-tcp] accept error: {err}"),
				}
			}
		}
		conn_tasks.close();
		conn_tasks.wait().await;
		Ok(())
	}
}

// ── UDP tunnel ─────────────────────────────────────────────────────────────

/// Per-source-address UDP tunnel session.
struct UdpTunnelSession {
	assoc_id: u16,
	tx_to_out: tokio::sync::mpsc::Sender<UdpPacket>,
	last_seen: std::time::Instant,
}

/// UDP port forwarder as a wind-core inbound.
///
/// Packets from different source addresses get separate UDP relay sessions.
/// Each session is routed through the dispatcher via
/// [`InboundCallback::handle_udpstream`].
pub struct TunnelUdpInbound {
	socket: Arc<UdpSocket>,
	remote: (String, u16),
	timeout: std::time::Duration,
	cancel: CancellationToken,
}

impl TunnelUdpInbound {
	pub fn new(
		listen: SocketAddr,
		remote: (String, u16),
		timeout: std::time::Duration,
		cancel: CancellationToken,
	) -> std::io::Result<Self> {
		let socket = std::net::UdpSocket::bind(listen)?;
		socket.set_nonblocking(true)?;
		let socket = UdpSocket::from_std(socket)?;
		Ok(Self {
			socket: Arc::new(socket),
			remote,
			timeout,
			cancel,
		})
	}
}

impl AbstractInbound for TunnelUdpInbound {
	async fn listen(&self, cb: &impl InboundCallback) -> eyre::Result<()> {
		info!(
			"[tunnel-udp] listening on {listen} -> {remote:?} timeout={timeout:?}",
			listen = self.socket.local_addr()?,
			remote = self.remote,
			timeout = self.timeout
		);

		let mut buf = vec![0u8; 65535];
		let mut sessions: HashMap<SocketAddr, UdpTunnelSession> = HashMap::new();
		let mut gc_interval = tokio::time::interval(self.timeout / 4);
		gc_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

		loop {
			tokio::select! {
				_ = self.cancel.cancelled() => {
					info!("[tunnel-udp] cancellation received, shutting down");
					break;
				}
				recv = self.socket.recv_from(&mut buf) => match recv {
					Ok((n, src_addr)) => {
						let pkt = Bytes::copy_from_slice(&buf[..n]);
						let target = TargetAddr::Domain(self.remote.0.clone(), self.remote.1);

						let session = sessions.entry(src_addr).or_insert_with(|| {
							let assoc_id = next_assoc_id();
							let socket_for_reply = self.socket.clone();
							let (tx_to_out, rx_from_local) = tokio::sync::mpsc::channel::<UdpPacket>(64);
							let (tx_to_local, mut rx_from_out) = tokio::sync::mpsc::channel::<UdpPacket>(64);
							let udp_stream = UdpStream { tx: tx_to_local, rx: rx_from_local };

							// Reply bridge: packets from remote → local socket.
							tokio::spawn(async move {
								while let Some(reply_pkt) = rx_from_out.recv().await {
									if let Err(err) = socket_for_reply.send_to(&reply_pkt.payload, src_addr).await {
										warn!("[tunnel-udp] [{assoc_id:#06x}] reply send error: {err}");
									}
								}
							}.in_current_span());

							// Relay through dispatcher
							let cb = cb.clone();
							tokio::spawn(async move {
								if let Err(e) = cb.handle_udpstream(udp_stream).await {
									warn!("[tunnel-udp] [{assoc_id:#06x}] relay error: {e}");
								}
							}.in_current_span());

							UdpTunnelSession {
								assoc_id,
								tx_to_out,
								last_seen: std::time::Instant::now(),
							}
						});

						session.last_seen = std::time::Instant::now();
						let assoc_id = session.assoc_id;

						match session.tx_to_out.try_send(UdpPacket {
							source: None,
							target,
							payload: pkt,
						}) {
							Ok(()) => {}
							Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
								debug!("[tunnel-udp] [{assoc_id:#06x}] outbound queue full; dropping packet from {src_addr}");
							}
							Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
								debug!("[tunnel-udp] [{assoc_id:#06x}] outbound closed; removing session for {src_addr}");
								sessions.remove(&src_addr);
							}
						}
					}
					Err(err) => warn!("[tunnel-udp] recv_from error: {err}"),
				},
				_ = gc_interval.tick() => {
					let now = std::time::Instant::now();
					sessions.retain(|src_addr, s| {
						if now.duration_since(s.last_seen) >= self.timeout {
							debug!(
								"[tunnel-udp] [{assoc:#06x}] idle timeout; dropping session for {src_addr}",
								assoc = s.assoc_id
							);
							false
						} else {
							true
						}
					});
				}
			}
		}
		Ok(())
	}
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn create_tcp_listener(addr: SocketAddr) -> std::io::Result<TcpListener> {
	let domain = match addr {
		SocketAddr::V4(_) => Domain::IPV4,
		SocketAddr::V6(_) => Domain::IPV6,
	};
	let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
	socket.set_reuse_address(true)?;
	socket.set_nonblocking(true)?;
	socket.bind(&SockAddr::from(addr))?;
	socket.listen(i32::MAX)?;
	TcpListener::from_std(StdTcpListener::from(socket))
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use tokio::io::{AsyncReadExt, AsyncWriteExt};

	use super::*;

	fn free_tcp_addr() -> SocketAddr {
		let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
		let a = l.local_addr().unwrap();
		drop(l);
		a
	}

	fn free_udp_addr() -> SocketAddr {
		let s = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
		let a = s.local_addr().unwrap();
		drop(s);
		a
	}

	/// Callback that echoes TCP data back (for testing basic relay).
	#[derive(Clone)]
	struct EchoCallback;

	impl InboundCallback for EchoCallback {
		async fn handle_tcpstream(
			&self,
			_target_addr: TargetAddr,
			mut stream: impl wind_core::tcp::AbstractTcpStream,
		) -> eyre::Result<()> {
			let mut buf = vec![0u8; 1024];
			let n = stream.read(&mut buf).await?;
			stream.write_all(&buf[..n]).await?;
			stream.shutdown().await?;
			Ok(())
		}

		async fn handle_udpstream(&self, _udp_stream: UdpStream) -> eyre::Result<()> {
			Ok(())
		}
	}

	/// Callback that records the target address for assertion.
	#[derive(Clone)]
	struct RecordCallback {
		targets: Arc<std::sync::Mutex<Vec<TargetAddr>>>,
	}

	impl InboundCallback for RecordCallback {
		async fn handle_tcpstream(
			&self,
			target_addr: TargetAddr,
			mut stream: impl wind_core::tcp::AbstractTcpStream,
		) -> eyre::Result<()> {
			self.targets.lock().unwrap().push(target_addr);
			// drain the stream so the peer sees EOF
			let mut buf = [0u8; 64];
			let _ = stream.read(&mut buf).await;
			stream.shutdown().await?;
			Ok(())
		}

		async fn handle_udpstream(&self, _udp_stream: UdpStream) -> eyre::Result<()> {
			Ok(())
		}
	}

	/// Callback that rejects every connection (for testing reject paths).
	#[derive(Clone)]
	struct RejectCallback;

	impl InboundCallback for RejectCallback {
		async fn handle_tcpstream(
			&self,
			_target_addr: TargetAddr,
			_stream: impl wind_core::tcp::AbstractTcpStream,
		) -> eyre::Result<()> {
			Err(eyre::eyre!("rejected"))
		}

		async fn handle_udpstream(&self, _udp_stream: UdpStream) -> eyre::Result<()> {
			Err(eyre::eyre!("rejected"))
		}
	}

	// ── Helpers for tests ───────────────────────────────────────────────────

	async fn connect_with_retry(addr: SocketAddr) -> std::io::Result<tokio::net::TcpStream> {
		let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
		loop {
			match tokio::net::TcpStream::connect(addr).await {
				Ok(s) => return Ok(s),
				Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused && tokio::time::Instant::now() < deadline => {
					tokio::time::sleep(Duration::from_millis(10)).await;
				}
				Err(e) => return Err(e),
			}
		}
	}

	// ── TCP tunnel tests ──────────────────────────────────────────────────

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn tcp_tunnel_binds_and_listens() {
		let cancel = CancellationToken::new();
		let addr = free_tcp_addr();
		let inbound = TunnelTcpInbound::new(addr, ("example.com".into(), 80), cancel);

		let _join = tokio::spawn(async move { inbound.listen(&RejectCallback).await });

		let stream = connect_with_retry(addr).await;
		assert!(stream.is_ok(), "must be able to connect to tunnel tcp listener");
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn tcp_tunnel_forwards_correct_target() {
		let cancel = CancellationToken::new();
		let addr = free_tcp_addr();
		let targets = Arc::new(std::sync::Mutex::new(Vec::new()));
		let cb = RecordCallback {
			targets: targets.clone(),
		};
		let inbound = TunnelTcpInbound::new(addr, ("google.com".into(), 443), cancel);

		let _join = tokio::spawn(async move { inbound.listen(&cb).await });
		let mut stream = connect_with_retry(addr).await.unwrap();
		// Write something so the callback can read/drain.
		stream.write_all(b"hello").await.unwrap();
		stream.shutdown().await.unwrap();

		tokio::time::sleep(Duration::from_millis(100)).await;

		let recorded = targets.lock().unwrap();
		assert_eq!(recorded.len(), 1);
		assert_eq!(recorded[0], TargetAddr::Domain("google.com".into(), 443));
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn tcp_tunnel_echo_relay() {
		let cancel = CancellationToken::new();
		let addr = free_tcp_addr();
		let inbound = TunnelTcpInbound::new(addr, ("example.com".into(), 80), cancel);

		let _join = tokio::spawn(async move { inbound.listen(&EchoCallback).await });
		let mut stream = connect_with_retry(addr).await.unwrap();
		stream.write_all(b"ping").await.unwrap();
		stream.shutdown().await.unwrap();

		let mut buf = Vec::new();
		stream.read_to_end(&mut buf).await.unwrap();
		assert_eq!(buf, b"ping");
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn tcp_tunnel_multiple_connections() {
		let cancel = CancellationToken::new();
		let addr = free_tcp_addr();
		let targets = Arc::new(std::sync::Mutex::new(Vec::new()));
		let cb = RecordCallback {
			targets: targets.clone(),
		};
		let inbound = TunnelTcpInbound::new(addr, ("multi.test".into(), 8080), cancel);

		let _join = tokio::spawn(async move { inbound.listen(&cb).await });
		for _ in 0..5 {
			let mut stream = connect_with_retry(addr).await.unwrap();
			stream.write_all(b"x").await.unwrap();
			stream.shutdown().await.unwrap();
		}
		tokio::time::sleep(Duration::from_millis(200)).await;

		let recorded = targets.lock().unwrap();
		assert_eq!(recorded.len(), 5);
		for t in recorded.iter() {
			assert_eq!(*t, TargetAddr::Domain("multi.test".into(), 8080));
		}
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn tcp_tunnel_drains_on_cancel() {
		let cancel = CancellationToken::new();
		let inbound = TunnelTcpInbound::new(free_tcp_addr(), ("127.0.0.1".into(), 9), cancel.clone());

		let _join = tokio::spawn(async move { inbound.listen(&RejectCallback).await });

		tokio::time::sleep(Duration::from_millis(100)).await;
		cancel.cancel();

		let _result = tokio::time::timeout(Duration::from_secs(5), _join)
			.await
			.expect("timed out")
			.expect("join error")
			.expect("listen returned error");
		// listen() returns Ok(()) on clean cancel
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn tcp_tunnel_with_ipv6_loopback() {
		let cancel = CancellationToken::new();
		let addr: SocketAddr = "[::1]:0".parse().unwrap();
		let listener = std::net::TcpListener::bind(addr).unwrap();
		let addr = listener.local_addr().unwrap();
		drop(listener);

		let inbound = TunnelTcpInbound::new(addr, ("ipv6.test".into(), 443), cancel);
		let _join = tokio::spawn(async move { inbound.listen(&RejectCallback).await });

		let stream = connect_with_retry(addr).await;
		assert!(stream.is_ok(), "must connect on IPv6 loopback");
	}

	// ── UDP tunnel tests ──────────────────────────────────────────────────

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn udp_tunnel_binds_and_listens() {
		let cancel = CancellationToken::new();
		let addr = free_udp_addr();
		let inbound = TunnelUdpInbound::new(addr, ("example.com".into(), 53), Duration::from_secs(60), cancel).unwrap();

		let _join = tokio::spawn(async move { inbound.listen(&RejectCallback).await });
		tokio::time::sleep(Duration::from_millis(50)).await;

		// Send a UDP packet → should reach the tunnel (callback rejects, but
		// we just want to confirm the socket is listening).
		let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
		let _result = client.send_to(b"test", addr).await;
		assert!(_result.is_ok());
	}

	/// Callback that echoes the first UDP packet payload back to the sender
	/// via the UdpStream reply channel.
	#[derive(Clone)]
	struct UdpEchoCallback;

	impl InboundCallback for UdpEchoCallback {
		async fn handle_tcpstream(
			&self,
			_target_addr: TargetAddr,
			_stream: impl wind_core::tcp::AbstractTcpStream,
		) -> eyre::Result<()> {
			Ok(())
		}

		async fn handle_udpstream(&self, udp_stream: UdpStream) -> eyre::Result<()> {
			let UdpStream { tx, mut rx } = udp_stream;
			while let Some(pkt) = rx.recv().await {
				// Echo the payload back with source=target so the reply bridge
				// sends it to the original client.
				let reply = UdpPacket {
					payload: pkt.payload,
					target: pkt.target,
					source: None,
				};
				if tx.send(reply).await.is_err() {
					break;
				}
			}
			Ok(())
		}
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn udp_tunnel_echo_relay() {
		let cancel = CancellationToken::new();
		let addr = free_udp_addr();
		let inbound = TunnelUdpInbound::new(addr, ("echo.test".into(), 53), Duration::from_secs(60), cancel).unwrap();

		let _join = tokio::spawn(async move { inbound.listen(&UdpEchoCallback).await });
		tokio::time::sleep(Duration::from_millis(50)).await;

		let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
		client.send_to(b"hello-udp", addr).await.unwrap();

		let mut buf = vec![0u8; 1024];
		let (n, from) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut buf))
			.await
			.expect("timed out waiting for echo")
			.unwrap();

		assert_eq!(&buf[..n], b"hello-udp");
		assert_eq!(from, addr);
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn udp_tunnel_per_source_session_isolation() {
		let cancel = CancellationToken::new();
		let addr = free_udp_addr();
		let inbound = TunnelUdpInbound::new(addr, ("multi.test".into(), 53), Duration::from_secs(60), cancel).unwrap();

		let _join = tokio::spawn(async move { inbound.listen(&UdpEchoCallback).await });
		tokio::time::sleep(Duration::from_millis(50)).await;

		// Two clients from different source ports → two separate sessions.
		let c1 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
		let c2 = UdpSocket::bind("127.0.0.1:0").await.unwrap();

		c1.send_to(b"from-c1", addr).await.unwrap();
		c2.send_to(b"from-c2", addr).await.unwrap();

		let mut buf = vec![0u8; 1024];
		let (n, _) = tokio::time::timeout(Duration::from_secs(2), c1.recv_from(&mut buf))
			.await
			.expect("c1 timed out")
			.unwrap();
		assert_eq!(&buf[..n], b"from-c1");

		let (n, _) = tokio::time::timeout(Duration::from_secs(2), c2.recv_from(&mut buf))
			.await
			.expect("c2 timed out")
			.unwrap();
		assert_eq!(&buf[..n], b"from-c2");
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn udp_tunnel_drains_on_cancel() {
		let cancel = CancellationToken::new();
		let inbound = TunnelUdpInbound::new(
			free_udp_addr(),
			("127.0.0.1".into(), 9),
			Duration::from_secs(60),
			cancel.clone(),
		)
		.unwrap();

		let _join = tokio::spawn(async move { inbound.listen(&RejectCallback).await });

		tokio::time::sleep(Duration::from_millis(100)).await;
		cancel.cancel();

		let _result = tokio::time::timeout(Duration::from_secs(5), _join)
			.await
			.expect("timed out")
			.expect("join error")
			.expect("listen returned error");
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn udp_tunnel_session_idle_expiry() {
		// Use a short timeout so sessions expire quickly.
		let cancel = CancellationToken::new();
		let addr = free_udp_addr();

		/// Callback that counts how many handle_udpstream calls were made.
		#[derive(Clone)]
		struct CountCallback {
			count: Arc<std::sync::atomic::AtomicUsize>,
		}

		impl InboundCallback for CountCallback {
			async fn handle_tcpstream(
				&self,
				_target: TargetAddr,
				_stream: impl wind_core::tcp::AbstractTcpStream,
			) -> eyre::Result<()> {
				Ok(())
			}

			async fn handle_udpstream(&self, stream: UdpStream) -> eyre::Result<()> {
				self.count.fetch_add(1, Ordering::Relaxed);
				// Keep the session alive briefly, then drop.
				let UdpStream { tx: _tx, mut rx } = stream;
				// Drain one packet then close → session ends.
				let _ = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
				Ok(())
			}
		}

		let cb = CountCallback {
			count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
		};
		let count = cb.count.clone();
		let inbound = TunnelUdpInbound::new(
			addr,
			("expire.test".into(), 53),
			Duration::from_millis(200), // short timeout
			cancel,
		)
		.unwrap();

		let _join = tokio::spawn(async move { inbound.listen(&cb).await });
		tokio::time::sleep(Duration::from_millis(50)).await;

		// First client: sends packet, callback processes it, session stays.
		let c1 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
		c1.send_to(b"p1", addr).await.unwrap();

		tokio::time::sleep(Duration::from_millis(100)).await;

		// Second client: same happens.
		let c2 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
		c2.send_to(b"p2", addr).await.unwrap();

		tokio::time::sleep(Duration::from_millis(100)).await;

		// Wait for session timeouts (200ms timeout + 50ms tick = ~250ms).
		tokio::time::sleep(Duration::from_millis(300)).await;

		// Both sessions should have been created (2 calls).
		assert!(count.load(Ordering::Relaxed) >= 2);
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn udp_tunnel_queue_full_drops_packet() {
		let cancel = CancellationToken::new();
		let addr = free_udp_addr();

		/// Callback that never reads from the UdpStream → queue fills up.
		#[derive(Clone)]
		struct StallCallback;
		impl InboundCallback for StallCallback {
			async fn handle_tcpstream(
				&self,
				_target: TargetAddr,
				_stream: impl wind_core::tcp::AbstractTcpStream,
			) -> eyre::Result<()> {
				Ok(())
			}

			async fn handle_udpstream(&self, _stream: UdpStream) -> eyre::Result<()> {
				// Never read → queue stays full.
				std::future::pending::<()>().await;
				Ok(())
			}
		}

		let inbound = TunnelUdpInbound::new(addr, ("stall.test".into(), 53), Duration::from_secs(60), cancel).unwrap();

		let _join = tokio::spawn(async move { inbound.listen(&StallCallback).await });
		tokio::time::sleep(Duration::from_millis(50)).await;

		let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
		// Send 100 packets > 64 queue size → some get dropped.
		for i in 0..100 {
			let payload = format!("pkt-{i}");
			let _ = client.send_to(payload.as_bytes(), addr).await;
		}

		// Should not panic or deadlock.
		tokio::time::sleep(Duration::from_millis(100)).await;

		// Join handle still alive (listening, stalled).
		assert!(!_join.is_finished());
	}
}
