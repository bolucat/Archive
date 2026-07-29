use std::{
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	sync::{Arc, atomic::AtomicU16},
	time::Duration,
};

use arc_swap::ArcSwap;
use moka::future::Cache;
use quinn::TokioRuntime;
use quinn_congestions::bbr::BbrConfig;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument as _, info, warn};
use uuid::Uuid;
use wind_core::{AbstractOutbound, AppContext, tcp::AbstractTcpStream, types::TargetAddr};
use wind_quic::{QuicConnection as _, quinn::QuinnConnection};

use crate::{
	Error,
	client::ClientTaskExt,
	proto::{ClientProtoExt, UdpStream as TuicUdpStream},
};

pub struct TuicOutboundOpts {
	pub peer_addr: SocketAddr,
	pub sni: String,
	pub auth: (Uuid, Arc<[u8]>),
	pub zero_rtt_handshake: bool,
	pub heartbeat: Duration,
	pub gc_interval: Duration,
	pub gc_lifetime: Duration,
	pub skip_cert_verify: bool,
	pub alpn: Vec<String>,
	/// Automatic reconnect behaviour for the outbound connection.
	pub reconnect: ReconnectConfig,
}

/// Controls how the outbound supervisor re-establishes the QUIC connection
/// after it drops.
#[derive(Clone, Debug)]
pub struct ReconnectConfig {
	/// When `false`, a dropped connection is not re-established — the
	/// supervisor closes it and exits (the pre-reconnect behaviour). When
	/// `true`, it retries with exponential backoff until it succeeds or the
	/// client shuts down.
	pub enabled: bool,
	/// Delay before the first reconnect attempt; doubled after each failure.
	pub initial_backoff: Duration,
	/// Upper bound on the backoff delay.
	pub max_backoff: Duration,
}

impl Default for ReconnectConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			initial_backoff: Duration::from_millis(500),
			max_backoff: Duration::from_secs(30),
		}
	}
}

pub struct TuicOutbound {
	pub ctx: Arc<AppContext>,
	pub endpoint: quinn::Endpoint,
	pub peer_addr: SocketAddr,
	pub sni: String,
	pub opts: TuicOutboundOpts,
	/// The live QUIC connection, swappable so the reconnect supervisor can
	/// replace it after a drop without callers holding a stale handle. Read
	/// sites `load_full()` the current connection per operation.
	pub connection: Arc<ArcSwap<QuinnConnection>>,
	pub udp_assoc_counter: AtomicU16,
	pub token: CancellationToken,
	pub udp_session: Cache<u16, Arc<TuicUdpStream<QuinnConnection>>>,
}

impl TuicOutbound {
	pub async fn new(ctx: Arc<AppContext>, opts: TuicOutboundOpts) -> Result<Self, Error> {
		let peer_addr = opts.peer_addr;
		let server_name = opts.sni.clone();

		// Install the rustls default crypto provider EXACTLY ONCE per process.
		// `install_default` returns `Err` after the first call (the global is
		// already set), which we previously swallowed via `let _ = ...` on
		// every single `TuicOutbound::new`. `OnceLock::get_or_init` is the
		// canonical race-free single-init primitive.
		static PROVIDER_INSTALLED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
		PROVIDER_INSTALLED.get_or_init(|| {
			#[cfg(feature = "aws-lc-rs")]
			let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
			#[cfg(feature = "ring")]
			let _ = rustls::crypto::ring::default_provider().install_default();
		});
		info!(target: "tuic_out", "Creating a new outbound");
		let client_config = {
			let tls_config = super::tls::tls_config(&server_name, &opts)?;

			let mut client_config = quinn::ClientConfig::new(Arc::new(
				quinn::crypto::rustls::QuicClientConfig::try_from(tls_config).unwrap(),
			));
			let mut transport_config = quinn::TransportConfig::default();
			transport_config
				.congestion_controller_factory(Arc::new(BbrConfig::default()))
				.keep_alive_interval(None);

			client_config.transport_config(Arc::new(transport_config));
			client_config
		};
		// Bind the local socket in the same address family as the peer. A quinn
		// endpoint bound to 0.0.0.0 cannot dial an IPv6 peer -- `connect` returns
		// `InvalidRemoteAddress` -- so an IPv6 server (`[::1]:8444`) was
		// unreachable when we always bound IPv4.
		let socket_addr = if peer_addr.is_ipv6() {
			SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))
		} else {
			SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))
		};
		let socket = UdpSocket::bind(&socket_addr)
			.await
			.map_err(|e| eyre::eyre!("Failed to bind socket to {}: {}", socket_addr, e))?
			.into_std()?;

		let endpoint = quinn::Endpoint::new(quinn::EndpointConfig::default(), None, socket, Arc::new(TokioRuntime))?;
		endpoint.set_default_client_config(client_config);

		// Establish and authenticate the initial connection. The reconnect
		// supervisor reuses the same configured endpoint via `connect_and_auth`.
		let connection = connect_and_auth(&endpoint, peer_addr, &server_name, &opts.auth).await?;

		Ok(Self {
			token: ctx.token.child_token(),
			ctx,
			endpoint,
			peer_addr,
			sni: server_name,
			opts,
			connection: Arc::new(ArcSwap::from_pointee(connection)),
			udp_assoc_counter: AtomicU16::new(0),
			udp_session: Cache::new(u16::MAX.into()),
		})
	}

	pub async fn start_poll(&self) -> eyre::Result<()> {
		let shutdown = self.ctx.token.child_token();
		let connection_cell = self.connection.clone();
		let endpoint = self.endpoint.clone();
		let peer_addr = self.peer_addr;
		let sni = self.sni.clone();
		let auth = self.opts.auth.clone();
		let heartbeat = self.opts.heartbeat;
		let reconnect = self.opts.reconnect.clone();
		let ctx = self.ctx.clone();
		let udp_session = self.udp_session.clone();

		// Connection supervisor: run one session (heartbeat + incoming handling)
		// over the live connection until it drops or we shut down. On an
		// unexpected drop, reconnect with backoff and swap the fresh connection
		// into `connection_cell` so `handle_tcp` / `handle_udp` transparently use
		// it. In-flight streams on the old connection are NOT resurrected —
		// callers see them close and retry, getting new streams on the new
		// connection.
		let supervisor = async move {
			loop {
				let session_cancel = shutdown.child_token();
				let conn = connection_cell.load_full().as_ref().clone();

				let end = run_session(&ctx, &conn, &udp_session, heartbeat, session_cancel.clone(), shutdown.clone()).await;
				// Tear down this session's accept loops before reconnecting.
				session_cancel.cancel();

				match end {
					Ok(SessionEnd::Shutdown) => {
						// Tell the server we are going away so it can reap the
						// connection immediately instead of waiting out its idle
						// timeout.
						conn.close(0, b"client shutdown");
						return eyre::Ok(());
					}
					Ok(SessionEnd::Lost) => {
						warn!(target: "tuic_out", "Connection to {} lost; attempting to reconnect", peer_addr);
					}
					Err(e) => {
						warn!(target: "tuic_out", "Session ended with error: {e:?}; attempting to reconnect");
					}
				}

				// A shutdown that raced the session drop: don't reconnect.
				if shutdown.is_cancelled() {
					conn.close(0, b"client shutdown");
					return eyre::Ok(());
				}
				// Reconnect disabled: close and stop, mirroring the pre-reconnect
				// behaviour where a dropped connection ended the poll task.
				if !reconnect.enabled {
					warn!(target: "tuic_out", "Reconnect disabled; connection to {} will not be re-established", peer_addr);
					conn.close(0, b"client connection lost");
					return eyre::Ok(());
				}
				// Abandon the dead connection explicitly so the server reaps it.
				conn.close(0, b"reconnecting");

				match reconnect_loop(&endpoint, peer_addr, &sni, &auth, &reconnect, &shutdown).await {
					Some(new_conn) => {
						connection_cell.store(Arc::new(new_conn));
						info!(target: "tuic_out", "Reconnected to {}", peer_addr);
					}
					// Cancelled while backing off.
					None => return eyre::Ok(()),
				}
			}
		};
		self.ctx.tasks.spawn(supervisor.in_current_span());

		Ok(())
	}
}

/// Outcome of a single connection session.
enum SessionEnd {
	/// The shutdown token fired — the supervisor should stop, not reconnect.
	Shutdown,
	/// The connection dropped (peer/transport close or heartbeat failures) —
	/// the supervisor should reconnect.
	Lost,
}

/// Open a fresh QUIC connection on `endpoint` and complete the TUIC auth
/// handshake. Shared by initial connect ([`TuicOutbound::new`]) and reconnect.
async fn connect_and_auth(
	endpoint: &quinn::Endpoint,
	peer_addr: SocketAddr,
	sni: &str,
	auth: &(Uuid, Arc<[u8]>),
) -> Result<QuinnConnection, Error> {
	let raw = endpoint
		.connect(peer_addr, sni)
		.map_err(|e| eyre::eyre!("Failed to connect to {} ({}): {}", peer_addr, sni, e))?
		.await?;
	// Wrap in the backend-agnostic handle so the shared client/proto code
	// (auth, heartbeat, TCP/UDP relay) drives it.
	let connection = QuinnConnection::new(raw);
	connection.send_auth(&auth.0, &auth.1).await?;
	Ok(connection)
}

/// Next exponential-backoff delay: double `current`, capped at `max`.
fn next_backoff(current: Duration, max: Duration) -> Duration {
	current.saturating_mul(2).min(max)
}

/// Retry [`connect_and_auth`] with exponential backoff until it succeeds or
/// `shutdown` fires. Returns `None` if cancelled before a connection is made.
async fn reconnect_loop(
	endpoint: &quinn::Endpoint,
	peer_addr: SocketAddr,
	sni: &str,
	auth: &(Uuid, Arc<[u8]>),
	reconnect: &ReconnectConfig,
	shutdown: &CancellationToken,
) -> Option<QuinnConnection> {
	let mut backoff = reconnect.initial_backoff;

	loop {
		// Race the connect attempt against shutdown so a hung handshake (e.g.
		// the server is still down) doesn't delay a graceful exit.
		let attempt = tokio::select! {
			_ = shutdown.cancelled() => return None,
			r = connect_and_auth(endpoint, peer_addr, sni, auth) => r,
		};
		match attempt {
			Ok(conn) => return Some(conn),
			Err(e) => {
				warn!(target: "tuic_out", "Reconnect to {} failed: {e}; retrying in {:?}", peer_addr, backoff);
				tokio::select! {
					_ = shutdown.cancelled() => return None,
					_ = tokio::time::sleep(backoff) => {}
				}
				backoff = next_backoff(backoff, reconnect.max_backoff);
			}
		}
	}
}

/// Drive one connection's heartbeat and incoming-stream handling until the
/// connection drops, heartbeats fail repeatedly, or shutdown fires. `conn` is
/// the live connection; `session_cancel` scopes this session's accept loops.
async fn run_session(
	ctx: &Arc<AppContext>,
	conn: &QuinnConnection,
	udp_session: &Cache<u16, Arc<TuicUdpStream<QuinnConnection>>>,
	heartbeat: Duration,
	session_cancel: CancellationToken,
	shutdown: CancellationToken,
) -> eyre::Result<SessionEnd> {
	let (datagram_rx, bi_rx, uni_rx) = conn.handle_incoming(ctx.clone(), session_cancel).await?;

	let mut hb_interval = tokio::time::interval(heartbeat);
	const HEARTBEAT_MAX_FAILURES: usize = 3;
	let mut hb_failures = 0;
	hb_interval.tick().await;

	loop {
		tokio::select! {
			_ = shutdown.cancelled() => {
				info!(target: "tuic_out", "Heartbeat poll cancelled");
				return Ok(SessionEnd::Shutdown);
			}
			_ = conn.closed() => {
				info!(target: "tuic_out", "Connection closed");
				return Ok(SessionEnd::Lost);
			}
			_ = hb_interval.tick() => {
				if let Err(e) = conn.send_heartbeat().await {
					hb_failures += 1;
					info!(target: "tuic_out", "Heartbeat failed ({}/{}): {}", hb_failures, HEARTBEAT_MAX_FAILURES, e);

					if hb_failures >= HEARTBEAT_MAX_FAILURES {
						return Ok(SessionEnd::Lost);
					}
				} else if hb_failures > 0 {
					info!(target: "tuic_out", "Heartbeat succeeded after {} failures", hb_failures);
					hb_failures = 0;
				}
			}
			Ok(_) = bi_rx.recv() => {
				warn!(target: "tuic_out", "Received bi-directional stream on Outbound");
			}
			Ok(mut buf) = datagram_rx.recv() => {
				info!(target: "tuic_out", "Received datagram: {} bytes", buf.len());
				use bytes::Buf;

				let header = match crate::proto::decode_header(&mut buf, "datagram") {
					Ok(h) => h,
					Err(e) => {
						warn!(target: "tuic_out", "Failed to decode header: {}", e);
						continue;
					}
				};

				let cmd = match crate::proto::decode_command(header.command, &mut buf, "datagram") {
					Ok(c) => c,
					Err(e) => {
						warn!(target: "tuic_out", "Failed to decode command: {}", e);
						continue;
					}
				};

				if let crate::proto::Command::Packet {
					assoc_id,
					pkt_id,
					frag_total,
					frag_id,
					size,
				} = cmd {
					let addr = match crate::proto::decode_address(&mut buf, "UDP packet") {
						Ok(a) => a,
						Err(e) => {
							warn!(target: "tuic_out", "Failed to decode address: {}", e);
							continue;
						}
					};

					// Extract payload. `size` is attacker-controlled (it comes straight
					// from the wire); `copy_to_bytes` panics when `size > buf.remaining()`,
					// so a malicious peer could crash the outbound poll task by
					// over-declaring it. Validate first and bail out cleanly instead.
					let size = size as usize;
					if buf.remaining() < size {
						warn!(
							target: "tuic_out",
							"Packet command claims {} bytes of payload but only {} remain — dropping",
							size, buf.remaining()
						);
						continue;
					}
					let payload = buf.copy_to_bytes(size);

					let (target, has_address) = match crate::proto::address_to_target(addr) {
						Ok(t) => (t, true),
						Err(_) => {
							(TargetAddr::IPv4(std::net::Ipv4Addr::UNSPECIFIED, 0), false)
						}
					};

					if has_address {
						info!(target: "tuic_out", "Received UDP packet: assoc={:#06x}, pkt={}, frag={}/{}, size={}, target={}",
							assoc_id, pkt_id, frag_id + 1, frag_total, size, target);
					} else {
						info!(target: "tuic_out", "Received UDP fragment: assoc={:#06x}, pkt={}, frag={}/{}, size={} (no address - non-first fragment)",
							assoc_id, pkt_id, frag_id + 1, frag_total, size);
					}

					if let Some(tuic_udp_stream) = udp_session.get(&assoc_id).await {
						let complete_packet = if frag_total > 1 {
							tuic_udp_stream.process_fragment(assoc_id, pkt_id, frag_total, frag_id, payload, None, target).await
						} else {
							Some(wind_core::udp::UdpPacket {
								source: None,
								target,
								payload,
							})
						};

					if let Some(packet) = complete_packet
						&& let Err(e) = tuic_udp_stream.receive_packet(packet).await {
							warn!(target: "tuic_out", "Failed to send packet to UDP session {:#06x}: {}", assoc_id, e);
						}
				} else {
						warn!(target: "tuic_out", "Received UDP packet for unknown association {:#06x}", assoc_id);
					}
				} else {
					warn!(target: "tuic_out", "Received non-Packet command in datagram: {:?}", cmd);
				}
			}

			Ok(_recv) = uni_rx.recv() => {
				info!(target: "tuic_out", "Received uni-directional stream");
			}
		}
	}
}

pub struct TuicTcpStream;

impl AbstractOutbound for TuicOutbound {
	async fn handle_tcp(
		&self,
		target_addr: TargetAddr,
		stream: impl AbstractTcpStream,
		_dialer: Option<impl AbstractOutbound>,
	) -> eyre::Result<()> {
		let connection = self.connection.load_full();
		connection.open_tcp(&target_addr, stream).await?;
		Ok(())
	}

	async fn handle_udp(
		&self,
		client_stream: wind_core::udp::UdpStream,
		_dialer: Option<impl AbstractOutbound>,
	) -> eyre::Result<()> {
		use std::sync::atomic::Ordering;
		let cancel = self.token.child_token();

		// Allocate a u16 association id, skipping ids that already have a live
		// session. Plain `fetch_add` would wrap silently into an active slot and
		// `udp_session.insert` would overwrite the previous Arc<UdpStream>,
		// dropping any in-flight packets for the original session and confusing
		// the peer (which still routes by the now-stolen id). We probe up to
		// `u16::MAX` candidates and refuse to allocate if every slot is in use.
		let assoc_id = {
			let mut id = self.udp_assoc_counter.fetch_add(1, Ordering::SeqCst);
			let mut probes = 0u32;
			while self.udp_session.get(&id).await.is_some() {
				probes += 1;
				if probes >= u16::MAX as u32 {
					return Err(eyre::eyre!(
						"UDP association id exhausted ({} concurrent sessions on this outbound)",
						self.udp_session.entry_count()
					));
				}
				id = self.udp_assoc_counter.fetch_add(1, Ordering::SeqCst);
			}
			id
		};
		info!(target: "tuic_out", "Creating new UDP association: {:#06x}", assoc_id);

		// Snapshot the live connection for this association. If a reconnect swaps
		// the connection later, this session's streams die and the caller retries.
		let connection = self.connection.load_full().as_ref().clone();
		let (receive_tx, receive_rx) = crossfire::mpmc::bounded_async(256);
		let tuic_stream = Arc::new(crate::proto::UdpStream::new(connection.clone(), assoc_id, receive_tx));
		self.udp_session.insert(assoc_id, tuic_stream.clone()).await;
		let cancel_stream = cancel.clone();

		let mut gc_interval = tokio::time::interval(self.opts.gc_interval);
		gc_interval.tick().await;

		let mut client_rx = client_stream.rx;
		let client_tx = client_stream.tx;

		let udp_task = async move {
			loop {
				tokio::select! {
					_ = cancel_stream.cancelled() => {
						info!(target: "tuic_out", "UDP stream sender for association {:#06x} cancelled", assoc_id);
						break;
					}

					result = receive_rx.recv() => {
						let packet = match result {
							Err(e) => {
								warn!(target: "tuic_out", "Error receiving packet from channel for association {:#06x}: {}", assoc_id, e);
								break;
							}
							Ok(packet) => packet,
						};

						if let Err(e) = client_tx.send(packet).await {
							warn!(target: "tuic_out", "Failed to send UDP packet to local socket (assoc {:#06x}): {:?}", assoc_id, e);
							break;
						} else {
							info!(target: "tuic_out", "Received UDP packet forward to local (assoc {:#06x})", assoc_id);
						}
					}
					packet = client_rx.recv() => {
						let packet = match packet {
							None => {
								warn!(target: "tuic_out", "Error receiving packet from channel for association {:#06x}: channel closed", assoc_id);
								break;
							}
							Some(packet) => packet,
						};

						// Send packet to remote via UDP stream
						let payload_len = packet.payload.len();
						if let Err(e) = tuic_stream.send_packet(packet).await {
							warn!(target: "tuic_out", "Failed to send UDP packet to remote (assoc {:#06x}): {}", assoc_id, e);
						} else {
							info!(target: "tuic_out", "Sent UDP packet to remote ({} bytes, assoc {:#06x})", payload_len, assoc_id);
						}
					}
					_ = gc_interval.tick() => {
						tuic_stream.collect_garbage().await;
					}
				}
			}
			eyre::Ok(())
		};
		let handle = self.ctx.tasks.spawn(udp_task.in_current_span());

		// Wait for the session to end: either the bridge task finished (the local
		// UDP channel closed, the remote errored, or `cancel` fired) or the
		// process is shutting down. The previous version looped forever on the
		// global token only, so once a session's bridge task exited on its own,
		// `handle_udp` kept spinning — pinning the `udp_session` cache entry and
		// never sending a `Dissociate`. A long-lived client that churns UDP
		// associations thus leaked one task slot and one assoc id per dead
		// session until the u16 assoc space was exhausted.
		tokio::select! {
			_ = handle => {}
			_ = self.ctx.token.cancelled() => {}
		}

		// Tear down: stop the bridge task if it is still running (global-shutdown
		// path), release the association id, and tell the peer to dissociate.
		cancel.cancel();
		self.udp_session.remove(&assoc_id).await;
		let connection = self.connection.load_full();
		if let Err(err) = connection.drop_udp(assoc_id).await {
			info!(target: "tuic_out", "Error dropping UDP association {:#06x}: {}", assoc_id, err);
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use super::{ReconnectConfig, next_backoff};

	#[test]
	fn next_backoff_doubles_until_capped() {
		let max = Duration::from_secs(30);
		assert_eq!(next_backoff(Duration::from_millis(500), max), Duration::from_secs(1));
		assert_eq!(next_backoff(Duration::from_secs(1), max), Duration::from_secs(2));
		assert_eq!(next_backoff(Duration::from_secs(16), max), max);
		// Already at/over the cap stays capped.
		assert_eq!(next_backoff(max, max), max);
		assert_eq!(next_backoff(Duration::from_secs(60), max), max);
	}

	#[test]
	fn next_backoff_does_not_overflow() {
		// Doubling near Duration::MAX must saturate, not panic.
		let huge = Duration::from_secs(u64::MAX / 2 + 1);
		let max = Duration::from_secs(u64::MAX);
		assert_eq!(next_backoff(huge, max), max);
	}

	#[test]
	fn reconnect_config_default_is_enabled_with_sane_bounds() {
		let cfg = ReconnectConfig::default();
		assert!(cfg.enabled);
		assert_eq!(cfg.initial_backoff, Duration::from_millis(500));
		assert_eq!(cfg.max_backoff, Duration::from_secs(30));
		assert!(cfg.initial_backoff <= cfg.max_backoff);
	}
}
