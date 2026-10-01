//! TUIC outbound client — quiche (tokio-quiche) backend.
//!
//! Thin connection provider around [`wind_quic::quiche`], mirroring
//! [`TuicheInbound`](crate::quiche::TuicheInbound): it establishes the QUIC
//! connection (optionally resuming a cached TLS session for 0-RTT), sends the
//! shared [`ClientProtoExt::send_auth`] handshake, and drives the
//! backend-agnostic [`crate::client`] accept/heartbeat machinery. All TUIC
//! protocol logic (auth, heartbeat, TCP/UDP relay) is shared with the quinn
//! backend.
//!
//! Unlike the quinn [`TuicOutbound`](crate::quinn::outbound::TuicOutbound), the
//! quiche client connection is dialed through
//! [`wind_quic::quiche::connect_with_session`], which owns its own UDP socket
//! (there is no caller-supplied socket factory or peer resolver), and the
//! local socket is IPv4 — an IPv6 peer is not reachable today.

use std::{
	net::SocketAddr,
	sync::{
		Arc,
		atomic::{AtomicU16, Ordering},
	},
	time::Duration,
};

use arc_swap::ArcSwap;
use async_trait::async_trait;
use bytes::Buf;
use moka::future::Cache;
use tokio::{io::AsyncReadExt as _, sync::Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument as _, info, warn};
use uuid::Uuid;
use wind_core::{
	AppContext, FlowContext, Outbound, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream as CoreUdpStream,
};
use wind_quic::{
	ClientTlsConfig, QuicConnection as _,
	quiche::{QuicheConnection, QuicheRecv, QuicheSend, connect_with_session},
};

use crate::{
	Error,
	client::ClientTaskExt,
	proto::{ClientProtoExt, UdpRelayMode, UdpStream as TuicUdpStream, open_connect_stream},
	quiche::utils::{ConnectionOpts, UdpRelayMode as TransportUdpRelayMode},
};

/// Upper bound on a single TUIC UDP command frame read from a uni stream:
/// header (2) + command (8) + longest address (259) + max payload (`u16`).
const MAX_PACKET_FRAME: usize = 10 + 259 + u16::MAX as usize;

/// How many 100 ms polls to wait for the server's `NewSessionTicket` before
/// giving up on caching a 0-RTT resumption ticket for this connection.
const SESSION_TICKET_POLLS: usize = 20;

/// Controls how the outbound supervisor re-establishes the QUIC connection
/// after it drops.
///
/// Backend-local mirror of the quinn outbound's config; the two are kept
/// separate so the quiche module does not depend on the quinn feature.
#[derive(Clone, Debug)]
pub struct ReconnectConfig {
	/// When `false`, a dropped connection is not re-established — the
	/// supervisor closes it and exits. When `true`, it retries with
	/// exponential backoff until it succeeds or the client shuts down.
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

/// Static configuration for a [`TuicheOutbound`].
#[derive(Clone)]
pub struct TuicheOutboundOpts {
	/// Server socket address to dial (already resolved).
	pub peer_addr: SocketAddr,
	/// Server name (SNI) sent and verified against.
	pub sni: String,
	/// TUIC credentials: `(uuid, password)`.
	pub auth: (Uuid, Arc<[u8]>),
	/// Verify the server certificate. Disabling it is insecure (MITM-able) and
	/// intended only for tests / explicitly-trusted setups.
	pub verify_certificate: bool,
	/// ALPN protocols to advertise, most-preferred first. Empty falls back to
	/// `h3` (the TUIC convention).
	pub alpn: Vec<Vec<u8>>,
	/// Interval between client heartbeats.
	pub heartbeat: Duration,
	/// Interval between runs of fragment-reassembly garbage collection.
	pub gc_interval: Duration,
	/// Lifetime after which incomplete fragment groups are evicted.
	pub gc_lifetime: Duration,
	/// Automatic reconnect behaviour after the connection drops.
	pub reconnect: ReconnectConfig,
	/// Transport-level tuning (idle timeout, windows, congestion control, ALPN,
	/// 0-RTT, and the UDP relay mode that also toggles QUIC DATAGRAM support).
	pub connection: ConnectionOpts,
}

/// Outbound manager for a single TUIC client connection.
pub struct TuicheOutbound {
	ctx: Arc<AppContext>,
	opts: TuicheOutboundOpts,
	/// Root token for this outbound. Cancelling it (e.g. on drop) stops the
	/// poll supervisor and every per-session accept loop.
	token: CancellationToken,
	/// The live QUIC connection, swappable so the reconnect supervisor can
	/// replace it after a drop without callers holding a stale handle.
	connection: Arc<ArcSwap<QuicheConnection>>,
	/// Cached TLS resumption ticket, captured after a successful handshake and
	/// replayed on reconnect to attempt 0-RTT early data.
	session: Arc<Mutex<Option<Vec<u8>>>>,
	udp_assoc_counter: AtomicU16,
	udp_session: Cache<u16, Arc<TuicUdpStream<QuicheConnection>>>,
}

impl TuicheOutbound {
	/// Create a new TUIC outbound builder.
	pub fn builder() -> TuicheOutboundBuilder {
		TuicheOutboundBuilder::new()
	}

	/// Dial the server, authenticate, and return a ready outbound.
	///
	/// This only establishes the connection; call
	/// [`start_poll`](Self::start_poll) to start the heartbeat and
	/// incoming-stream supervisor.
	pub async fn new(ctx: Arc<AppContext>, opts: TuicheOutboundOpts) -> Result<Self, Error> {
		if opts.heartbeat.is_zero() {
			return Err(eyre::eyre!("TUIC heartbeat interval must be positive"));
		}
		if opts.gc_interval.is_zero() {
			return Err(eyre::eyre!("TUIC GC interval must be positive"));
		}
		if opts.gc_lifetime.is_zero() {
			return Err(eyre::eyre!("TUIC GC lifetime must be positive"));
		}

		let token = ctx.token.child_token();
		let session: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
		let connection = establish(&opts, None).await?;

		let outbound = Self {
			ctx,
			opts,
			token,
			connection: Arc::new(ArcSwap::from_pointee(connection)),
			session,
			udp_assoc_counter: AtomicU16::new(0),
			udp_session: Cache::new(u64::from(u16::MAX)),
		};

		if outbound.opts.connection.enable_0rtt {
			spawn_ticket_capture(
				&outbound.ctx,
				outbound.connection.load_full().as_ref().clone(),
				outbound.session.clone(),
				outbound.token.child_token(),
			);
		}

		Ok(outbound)
	}

	/// The address the outbound dials.
	pub fn peer_addr(&self) -> SocketAddr {
		self.opts.peer_addr
	}

	/// The current live connection.
	pub fn connection(&self) -> Arc<QuicheConnection> {
		self.connection.load_full()
	}

	/// Start the connection supervisor: heartbeats, incoming-stream dispatch,
	/// and (when enabled) automatic reconnection with exponential backoff.
	pub async fn start_poll(&self) -> eyre::Result<()> {
		let shutdown = self.token.child_token();
		let connection = self.connection.clone();
		let session = self.session.clone();
		let ctx = self.ctx.clone();
		let opts = self.opts.clone();
		let udp_session = self.udp_session.clone();

		let supervisor = async move {
			loop {
				let session_cancel = shutdown.child_token();
				let conn = connection.load_full().as_ref().clone();

				let end = run_session(
					&ctx,
					&conn,
					&udp_session,
					opts.heartbeat,
					session_cancel.clone(),
					shutdown.clone(),
				)
				.await;
				session_cancel.cancel();

				match end {
					Ok(SessionEnd::Shutdown) => {
						conn.close(0, b"client shutdown");
						return eyre::Ok(());
					}
					Ok(SessionEnd::Lost) => {
						warn!(target: "tuic_out", peer = %opts.peer_addr, "Connection lost; attempting to reconnect");
					}
					Err(e) => {
						warn!(target: "tuic_out", "Session ended with error: {e:?}; attempting to reconnect");
					}
				}

				if shutdown.is_cancelled() {
					conn.close(0, b"client shutdown");
					return eyre::Ok(());
				}
				if !opts.reconnect.enabled {
					warn!(target: "tuic_out", peer = %opts.peer_addr, "Reconnect disabled; connection will not be re-established");
					conn.close(0, b"client connection lost");
					return eyre::Ok(());
				}

				conn.close(0, b"reconnecting");

				match reconnect_loop(&opts, &session, &shutdown).await {
					Some(new_conn) => {
						if opts.connection.enable_0rtt {
							spawn_ticket_capture(&ctx, new_conn.clone(), session.clone(), shutdown.child_token());
						}
						connection.store(Arc::new(new_conn));
						info!(target: "tuic_out", peer = %opts.peer_addr, "Reconnected");
					}
					None => return eyre::Ok(()),
				}
			}
		};
		self.ctx.tasks.spawn(supervisor.in_current_span());

		Ok(())
	}

	/// Open a TUIC TCP stream to `target`.
	///
	/// Opens a bidirectional QUIC stream, writes the `Connect` command header,
	/// and returns the joined stream for the caller to relay.
	pub async fn connect_tcp(&self, target: &TargetAddr) -> Result<tokio::io::Join<QuicheRecv, QuicheSend>, Error> {
		let connection = self.connection.load_full();
		let (send, recv) = open_connect_stream(connection.as_ref(), target).await?;
		Ok(tokio::io::join(recv, send))
	}

	/// Close the live connection and stop the supervisor.
	pub fn close(&self) {
		self.token.cancel();
		self.connection.load_full().close(0, b"client closed");
	}

	/// How outgoing `Packet` commands are relayed on this connection.
	fn relay_mode(&self) -> UdpRelayMode {
		match self.opts.connection.udp_relay_mode {
			TransportUdpRelayMode::Datagram => UdpRelayMode::Native,
			TransportUdpRelayMode::Stream => UdpRelayMode::Quic,
		}
	}
}

impl Drop for TuicheOutbound {
	fn drop(&mut self) {
		self.token.cancel();
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

/// Open a fresh QUIC connection and complete the TUIC auth handshake.
///
/// Shared by initial connect ([`TuicheOutbound::new`]) and reconnect. The
/// optional `session` ticket attempts TLS resumption (and 0-RTT when both
/// sides enable it); a rejected/absent ticket transparently falls back to a
/// full 1-RTT handshake.
async fn establish(opts: &TuicheOutboundOpts, session: Option<Vec<u8>>) -> Result<QuicheConnection, Error> {
	let tls = ClientTlsConfig {
		server_name: opts.sni.clone(),
		verify_certificate: opts.verify_certificate,
		alpn: if opts.alpn.is_empty() {
			vec![b"h3".to_vec()]
		} else {
			opts.alpn.clone()
		},
		enable_early_data: opts.connection.enable_0rtt,
	};
	let transport = opts.connection.to_transport();

	let conn = connect_with_session(opts.peer_addr, &tls, &transport, session).await?;
	conn.send_auth(&opts.auth.0, &opts.auth.1).await?;
	Ok(conn)
}

/// Poll the connection for the server's `NewSessionTicket` and cache it for a
/// future reconnect. Best-effort: gives up after a bounded wait.
fn spawn_ticket_capture(
	ctx: &Arc<AppContext>,
	conn: QuicheConnection,
	session: Arc<Mutex<Option<Vec<u8>>>>,
	shutdown: CancellationToken,
) {
	ctx.tasks.spawn(
		async move {
			for _ in 0..SESSION_TICKET_POLLS {
				if let Some(ticket) = conn.session().await {
					*session.lock().await = Some(ticket);
					return;
				}
				tokio::select! {
					_ = shutdown.cancelled() => return,
					_ = tokio::time::sleep(Duration::from_millis(100)) => {}
				}
			}
		}
		.in_current_span(),
	);
}

/// Next exponential-backoff delay: double `current`, capped at `max`.
fn next_backoff(current: Duration, max: Duration) -> Duration {
	current.saturating_mul(2).min(max)
}

/// Retry [`establish`] with exponential backoff until it succeeds or
/// `shutdown` fires. Returns `None` if cancelled before a connection is made.
async fn reconnect_loop(
	opts: &TuicheOutboundOpts,
	session: &Arc<Mutex<Option<Vec<u8>>>>,
	shutdown: &CancellationToken,
) -> Option<QuicheConnection> {
	let mut backoff = opts.reconnect.initial_backoff;

	loop {
		let cached = if opts.connection.enable_0rtt {
			session.lock().await.clone()
		} else {
			None
		};

		let attempt = tokio::select! {
			_ = shutdown.cancelled() => return None,
			r = establish(opts, cached) => r,
		};

		match attempt {
			Ok(conn) => return Some(conn),
			Err(e) => {
				warn!(target: "tuic_out", peer = %opts.peer_addr, "Reconnect failed: {e}; retrying in {backoff:?}");
				tokio::select! {
					_ = shutdown.cancelled() => return None,
					_ = tokio::time::sleep(backoff) => {}
				}
				backoff = next_backoff(backoff, opts.reconnect.max_backoff);
			}
		}
	}
}

/// Drive one connection's heartbeat and incoming-stream handling until the
/// connection drops, heartbeats fail repeatedly, or shutdown fires.
async fn run_session(
	ctx: &Arc<AppContext>,
	conn: &QuicheConnection,
	udp_session: &Cache<u16, Arc<TuicUdpStream<QuicheConnection>>>,
	heartbeat: Duration,
	session_cancel: CancellationToken,
	shutdown: CancellationToken,
) -> eyre::Result<SessionEnd> {
	let (datagram_rx, bi_rx, uni_rx) = conn.handle_incoming(ctx.clone(), session_cancel.clone()).await?;

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
					info!(target: "tuic_out", "Heartbeat failed ({hb_failures}/{HEARTBEAT_MAX_FAILURES}): {e}");
					if hb_failures >= HEARTBEAT_MAX_FAILURES {
						return Ok(SessionEnd::Lost);
					}
				} else if hb_failures > 0 {
					info!(target: "tuic_out", "Heartbeat succeeded after {hb_failures} failures");
					hb_failures = 0;
				}
			}
			Ok(_) = bi_rx.recv() => {
				warn!(target: "tuic_out", "Received bi-directional stream on Outbound");
			}
			Ok(buf) = datagram_rx.recv() => {
				info!(target: "tuic_out", "Received datagram: {} bytes", buf.len());
				dispatch_incoming_udp(udp_session, buf).await;
			}
			Ok(mut recv) = uni_rx.recv() => {
				// QUIC relay mode delivers UDP responses on uni streams; read
				// (bounded) and feed the matching association, mirroring the
				// datagram path above.
				let udp_session = udp_session.clone();
				let read_cancel = session_cancel.clone();
				ctx.tasks.spawn(
					async move {
						let mut data = Vec::new();
						let mut limited = (&mut recv).take(MAX_PACKET_FRAME as u64);
						tokio::select! {
							_ = read_cancel.cancelled() => {}
							result = limited.read_to_end(&mut data) => match result {
								Ok(_) => dispatch_incoming_udp(&udp_session, bytes::Bytes::from(data)).await,
								Err(e) => warn!(target: "tuic_out", "Failed to read UDP stream: {e}"),
							}
						}
					}
					.in_current_span(),
				);
			}
		}
	}
}

/// Decode one incoming `Packet` command (from a datagram or a uni stream) and
/// deliver the reassembled packet to the matching UDP association.
async fn dispatch_incoming_udp(udp_session: &Cache<u16, Arc<TuicUdpStream<QuicheConnection>>>, mut buf: bytes::Bytes) {
	let header = match crate::proto::decode_header(&mut buf, "datagram") {
		Ok(h) => h,
		Err(e) => {
			warn!(target: "tuic_out", "Failed to decode header: {e}");
			return;
		}
	};

	let cmd = match crate::proto::decode_command(header.command, &mut buf, "datagram") {
		Ok(c) => c,
		Err(e) => {
			warn!(target: "tuic_out", "Failed to decode command: {e}");
			return;
		}
	};

	let crate::proto::Command::Packet {
		assoc_id,
		pkt_id,
		frag_total,
		frag_id,
		size,
	} = cmd
	else {
		warn!(target: "tuic_out", "Received non-Packet command in datagram: {cmd:?}");
		return;
	};

	let addr = match crate::proto::decode_address(&mut buf, "UDP packet") {
		Ok(a) => a,
		Err(e) => {
			warn!(target: "tuic_out", "Failed to decode address: {e}");
			return;
		}
	};

	// `size` is attacker-controlled; `copy_to_bytes` panics when
	// `size > buf.remaining()`, so validate before slicing.
	let size = size as usize;
	if buf.remaining() < size {
		warn!(
			target: "tuic_out",
			"Packet command claims {size} bytes of payload but only {} remain — dropping",
			buf.remaining()
		);
		return;
	}
	let payload = buf.copy_to_bytes(size);

	let (target, has_address) = match crate::proto::address_to_target(addr) {
		Ok(t) => (t, true),
		Err(_) => (TargetAddr::IPv4(std::net::Ipv4Addr::UNSPECIFIED, 0), false),
	};

	if has_address {
		info!(target: "tuic_out", "Received UDP packet: assoc={assoc_id:#06x}, pkt={pkt_id}, frag={}/{frag_total}, size={size}, target={target}",
			frag_id + 1);
	} else {
		info!(target: "tuic_out", "Received UDP fragment: assoc={assoc_id:#06x}, pkt={pkt_id}, frag={}/{frag_total}, size={size} (no address - non-first fragment)",
			frag_id + 1);
	}

	if let Some(tuic_udp_stream) = udp_session.get(&assoc_id).await {
		let complete_packet = if frag_total > 1 {
			tuic_udp_stream
				.process_fragment(assoc_id, pkt_id, frag_total, frag_id, payload, None, target)
				.await
		} else {
			Some(wind_core::udp::UdpPacket {
				source: None,
				target,
				payload,
			})
		};

		if let Some(packet) = complete_packet
			&& let Err(e) = tuic_udp_stream.receive_packet(packet).await
		{
			warn!(target: "tuic_out", "Failed to send packet to UDP session {assoc_id:#06x}: {e}");
		}
	} else {
		warn!(target: "tuic_out", "Received UDP packet for unknown association {assoc_id:#06x}");
	}
}

#[async_trait]
impl Outbound for TuicheOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, mut stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let mut tcp = self.connect_tcp(&ctx.target).await?;
		let (_, _, err) = wind_core::io::copy_io(&mut stream, &mut tcp).await;
		if let Some(e) = err {
			return Err(e.into());
		}
		Ok(())
	}

	async fn handle_udp(&self, _ctx: FlowContext, client_stream: CoreUdpStream) -> eyre::Result<()> {
		let cancel = self.token.child_token();

		// Allocate a u16 association id, skipping ids that already have a live
		// session so a wrap cannot steal an active slot.
		let assoc_id = {
			let mut id = self.udp_assoc_counter.fetch_add(1, Ordering::SeqCst);
			let mut probes = 0u32;
			while self.udp_session.get(&id).await.is_some() {
				probes += 1;
				if probes >= u32::from(u16::MAX) {
					return Err(eyre::eyre!(
						"UDP association id exhausted ({} concurrent sessions on this outbound)",
						self.udp_session.entry_count()
					));
				}
				id = self.udp_assoc_counter.fetch_add(1, Ordering::SeqCst);
			}
			id
		};
		info!(target: "tuic_out", "Creating new UDP association: {assoc_id:#06x}");

		// Snapshot the live connection for this association. If a reconnect
		// swaps the connection later, this session's streams die and the
		// caller retries.
		let connection = self.connection.load_full().as_ref().clone();
		let assoc_connection = connection.clone();
		let (receive_tx, receive_rx) = crossfire::mpmc::bounded_async(256);
		let tuic_stream = Arc::new(
			TuicUdpStream::new(connection.clone(), assoc_id, receive_tx)
				.with_relay_mode(self.relay_mode())
				.with_gc_lifetime(self.opts.gc_lifetime),
		);
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
						info!(target: "tuic_out", "UDP stream sender for association {assoc_id:#06x} cancelled");
						break;
					}

					// The connection this association was created on is gone
					// (reconnect, peer close, or idle timeout): its streams can
					// no longer carry traffic, so end the session and let the
					// caller recreate the association on the new connection.
					_ = assoc_connection.closed() => {
						info!(target: "tuic_out", "Connection lost; ending UDP association {assoc_id:#06x}");
						break;
					}

					result = receive_rx.recv() => {
						let packet = match result {
							Err(e) => {
								warn!(target: "tuic_out", "Error receiving packet from channel for association {assoc_id:#06x}: {e}");
								break;
							}
							Ok(packet) => packet,
						};

						if let Err(e) = client_tx.send(packet).await {
							warn!(target: "tuic_out", "Failed to send UDP packet to local socket (assoc {assoc_id:#06x}): {e:?}");
							break;
						}
						info!(target: "tuic_out", "Received UDP packet forward to local (assoc {assoc_id:#06x})");
					}
					packet = client_rx.recv() => {
						let packet = match packet {
							None => {
								warn!(target: "tuic_out", "Channel closed for association {assoc_id:#06x}");
								break;
							}
							Some(packet) => packet,
						};

						let payload_len = packet.payload.len();
						if let Err(e) = tuic_stream.send_packet(packet).await {
							warn!(target: "tuic_out", "Failed to send UDP packet to remote (assoc {assoc_id:#06x}): {e}");
						} else {
							info!(target: "tuic_out", "Sent UDP packet to remote ({payload_len} bytes, assoc {assoc_id:#06x})");
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

		tokio::select! {
			_ = handle => {}
			_ = self.ctx.token.cancelled() => {}
		}

		cancel.cancel();
		self.udp_session.remove(&assoc_id).await;
		if let Err(err) = connection.drop_udp(assoc_id).await {
			info!(target: "tuic_out", "Error dropping UDP association {assoc_id:#06x}: {err}");
		}

		Ok(())
	}
}

/// Builder for [`TuicheOutbound`], mirroring
/// [`TuicheInboundBuilder`](crate::quiche::TuicheInboundBuilder).
pub struct TuicheOutboundBuilder {
	server_addr: Option<SocketAddr>,
	server_name: Option<String>,
	uuid: Option<Uuid>,
	password: Option<Vec<u8>>,
	verify_certificate: bool,
	alpn: Vec<Vec<u8>>,
	heartbeat: Duration,
	gc_interval: Duration,
	gc_lifetime: Duration,
	reconnect: ReconnectConfig,
	connection: ConnectionOpts,
	context: Option<Arc<AppContext>>,
}

impl TuicheOutboundBuilder {
	/// Create a new builder.
	pub fn new() -> Self {
		Self {
			server_addr: None,
			server_name: None,
			uuid: None,
			password: None,
			verify_certificate: true,
			alpn: Vec::new(),
			heartbeat: Duration::from_secs(3),
			gc_interval: Duration::from_secs(3),
			gc_lifetime: Duration::from_secs(15),
			reconnect: ReconnectConfig::default(),
			connection: ConnectionOpts::default(),
			context: None,
		}
	}

	/// Set the server socket address.
	pub fn server_addr(mut self, addr: SocketAddr) -> Self {
		self.server_addr = Some(addr);
		self
	}

	/// Set the server name (SNI).
	pub fn server_name(mut self, name: String) -> Self {
		self.server_name = Some(name);
		self
	}

	/// Set the user UUID.
	pub fn uuid(mut self, uuid: Uuid) -> Self {
		self.uuid = Some(uuid);
		self
	}

	/// Set the password.
	pub fn password(mut self, password: String) -> Self {
		self.password = Some(password.into_bytes());
		self
	}

	/// Enable or disable server certificate verification.
	pub fn verify_certificate(mut self, verify: bool) -> Self {
		self.verify_certificate = verify;
		self
	}

	/// Set the ALPN protocols to advertise, most-preferred first. Empty falls
	/// back to `h3`.
	pub fn alpn(mut self, alpn: Vec<Vec<u8>>) -> Self {
		self.alpn = alpn;
		self
	}

	/// Set the heartbeat interval.
	pub fn heartbeat(mut self, interval: Duration) -> Self {
		self.heartbeat = interval;
		self
	}

	/// Set the fragment-reassembly GC interval.
	pub fn gc_interval(mut self, interval: Duration) -> Self {
		self.gc_interval = interval;
		self
	}

	/// Set the fragment-reassembly GC lifetime.
	pub fn gc_lifetime(mut self, lifetime: Duration) -> Self {
		self.gc_lifetime = lifetime;
		self
	}

	/// Set the reconnect behaviour.
	pub fn reconnect(mut self, reconnect: ReconnectConfig) -> Self {
		self.reconnect = reconnect;
		self
	}

	/// Set the transport-level connection options (including the UDP relay
	/// mode).
	pub fn connection_opts(mut self, opts: ConnectionOpts) -> Self {
		self.connection = opts;
		self
	}

	/// Set the application context that owns the outbound's tasks. Defaults to
	/// a fresh context.
	pub fn context(mut self, ctx: Arc<AppContext>) -> Self {
		self.context = Some(ctx);
		self
	}

	/// Dial the server and authenticate, returning a ready outbound.
	pub async fn build(self) -> Result<TuicheOutbound, Error> {
		let server_addr = self.server_addr.ok_or_else(|| eyre::eyre!("Server address not set"))?;
		let server_name = self.server_name.ok_or_else(|| eyre::eyre!("Server name not set"))?;
		let uuid = self.uuid.ok_or_else(|| eyre::eyre!("UUID not set"))?;
		let password = self.password.ok_or_else(|| eyre::eyre!("Password not set"))?;

		let opts = TuicheOutboundOpts {
			peer_addr: server_addr,
			sni: server_name,
			auth: (uuid, Arc::from(password.into_boxed_slice())),
			verify_certificate: self.verify_certificate,
			alpn: self.alpn,
			heartbeat: self.heartbeat,
			gc_interval: self.gc_interval,
			gc_lifetime: self.gc_lifetime,
			reconnect: self.reconnect,
			connection: self.connection,
		};

		TuicheOutbound::new(self.context.unwrap_or_default(), opts).await
	}
}

impl Default for TuicheOutboundBuilder {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn next_backoff_doubles_until_capped() {
		let max = Duration::from_secs(30);
		assert_eq!(next_backoff(Duration::from_millis(500), max), Duration::from_secs(1));
		assert_eq!(next_backoff(Duration::from_secs(16), max), max);
		assert_eq!(next_backoff(max, max), max);
	}

	#[test]
	fn reconnect_config_default_is_enabled_with_sane_bounds() {
		let cfg = ReconnectConfig::default();
		assert!(cfg.enabled);
		assert!(cfg.initial_backoff <= cfg.max_backoff);
	}

	#[tokio::test]
	async fn builder_requires_required_fields() {
		match TuicheOutboundBuilder::new().build().await {
			Ok(_) => panic!("missing server address must fail"),
			Err(err) => assert!(format!("{err}").contains("Server address"), "unexpected error: {err}"),
		}
	}

	/// End-to-end TCP round trip against a real quiche `TuicheInbound`.
	///
	/// Gated on the `server` feature because the echo inbound lives in the
	/// server module; the outbound itself only needs `client` + `quiche`.
	#[cfg(feature = "server")]
	mod loopback {
		use std::{net::Ipv4Addr, sync::Arc, time::Duration};

		use async_trait::async_trait;
		use bytes::Bytes;
		use tokio::{
			io::{AsyncReadExt as _, AsyncWriteExt as _},
			sync::mpsc,
		};
		use uuid::Uuid;
		use wind_core::{
			AbstractInbound, Dispatcher, FlowContext, Outbound, RouteAction, Router,
			hooks::Protocol,
			rule::NetworkType,
			tcp::AbstractTcpStream,
			types::TargetAddr,
			udp::{UdpPacket, UdpStream as CoreUdpStream},
		};

		use super::super::{ConnectionOpts, ReconnectConfig, TuicheOutbound, TuicheOutboundBuilder};
		use crate::quiche::{TuicheInboundBuilder, UdpRelayMode as TransportUdpRelayMode};

		/// Forwards every routed connection by echoing TCP payloads back, so
		/// the client can verify a full TUIC TCP round trip without a separate
		/// echo server or resolver. UDP datagrams are echoed in place.
		struct EchoOutbound;

		async fn echo_stream(mut stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send) -> eyre::Result<()> {
			let mut buf = vec![0u8; 16 * 1024];
			loop {
				let n = stream.read(&mut buf).await?;
				if n == 0 {
					return Ok(());
				}
				stream.write_all(&buf[..n]).await?;
			}
		}

		#[async_trait]
		impl Outbound for EchoOutbound {
			async fn handle_tcp(&self, _ctx: FlowContext, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
				echo_stream(stream).await
			}

			async fn handle_udp(&self, _ctx: FlowContext, mut udp: CoreUdpStream) -> eyre::Result<()> {
				// Echo each datagram back to the client by re-sending it on the
				// association's outbound channel with the same attributed
				// target.
				while let Some(packet) = udp.rx.recv().await {
					let target = packet.target.clone();
					let reply = wind_core::udp::UdpPacket {
						source: Some(target.clone()),
						target,
						payload: packet.payload,
					};
					if udp.tx.send(reply).await.is_err() {
						break;
					}
				}
				Ok(())
			}
		}

		struct ForwardRouter;

		impl Router for ForwardRouter {
			#[allow(clippy::manual_async_fn)]
			fn route(&self, _ctx: &FlowContext) -> impl std::future::Future<Output = eyre::Result<RouteAction>> + Send {
				async { Ok(RouteAction::Forward("default".to_string())) }
			}
		}

		/// Start a quiche `TuicheInbound` that echoes, returning its bound
		/// address and the temp dir holding the certificate (kept alive by the
		/// caller). `server_opts` drives the server transport (notably the UDP
		/// relay mode / DATAGRAM support).
		async fn start_echo_server(
			uuid: Uuid,
			password: &str,
			server_opts: ConnectionOpts,
		) -> eyre::Result<(std::net::SocketAddr, tempfile::TempDir)> {
			let dir = tempfile::tempdir()?;
			let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
			let cert_path = dir.path().join("cert.pem");
			let key_path = dir.path().join("key.pem");
			std::fs::write(&cert_path, generated.cert.pem())?;
			std::fs::write(&key_path, generated.signing_key.serialize_pem())?;

			let (addr_tx, mut addr_rx) = tokio::sync::watch::channel(None::<std::net::SocketAddr>);
			let inbound = TuicheInboundBuilder::new()
				.listen_addr("127.0.0.1:0".parse()?)
				.bound_addr(addr_tx)
				.certificate_path(cert_path.to_string_lossy().into_owned())
				.private_key_path(key_path.to_string_lossy().into_owned())
				.user(uuid, password.to_string())
				.connection_opts(server_opts)
				.build()
				.await?;

			let mut dispatcher = Dispatcher::new(ForwardRouter);
			dispatcher.add_handler("default", Arc::new(EchoOutbound) as Arc<dyn Outbound>);
			tokio::spawn(async move {
				let _ = inbound.listen(&dispatcher).await;
			});

			let addr = addr_rx
				.wait_for(|a| a.is_some())
				.await?
				.ok_or_else(|| eyre::eyre!("listener exited before reporting its bound address"))?;
			Ok((addr, dir))
		}

		fn client_builder(
			addr: std::net::SocketAddr,
			uuid: Uuid,
			password: &str,
			relay_mode: TransportUdpRelayMode,
		) -> TuicheOutboundBuilder {
			let opts = ConnectionOpts {
				enable_0rtt: false,
				udp_relay_mode: relay_mode,
				..Default::default()
			};
			TuicheOutboundBuilder::new()
				.server_addr(addr)
				.server_name("localhost".to_string())
				.uuid(uuid)
				.password(password.to_string())
				.verify_certificate(false)
				.connection_opts(opts)
		}

		/// Deterministic failures for the UDP tests: a dropped connection must
		/// surface as a timeout in the test, not as a reconnect loop.
		fn no_reconnect() -> ReconnectConfig {
			ReconnectConfig {
				enabled: false,
				..Default::default()
			}
		}

		/// Send `payload` through `outbound`'s UDP association and return the
		/// echoed payload. The echo server mirrors the packet back on the same
		/// association, so this exercises the reply transport (datagram vs
		/// unidirectional stream) end to end.
		async fn udp_roundtrip(outbound: Arc<TuicheOutbound>, payload: &'static [u8]) -> eyre::Result<Bytes> {
			let target = TargetAddr::IPv4(Ipv4Addr::LOCALHOST, 9);
			let udp_ctx = FlowContext {
				target: target.clone(),
				network: NetworkType::Udp,
				source: None,
				inbound_tag: "quiche-loopback-test".into(),
				protocol: Protocol::Tunnel,
				user: None,
				inbound_port: None,
				inbound_type: None,
			};

			let (to_outbound_tx, to_outbound_rx) = mpsc::channel::<UdpPacket>(8);
			let (from_outbound_tx, mut from_outbound_rx) = mpsc::channel::<UdpPacket>(8);
			let client_stream = CoreUdpStream {
				tx: from_outbound_tx,
				rx: to_outbound_rx,
			};
			let client = outbound.clone();
			tokio::spawn(async move {
				let _ = client.handle_udp(udp_ctx, client_stream).await;
			});

			to_outbound_tx
				.send(UdpPacket {
					source: None,
					target,
					payload: Bytes::from_static(payload),
				})
				.await?;

			let echoed = tokio::time::timeout(Duration::from_secs(10), from_outbound_rx.recv())
				.await
				.map_err(|_| eyre::eyre!("UDP echo timed out — the reply never reached the client"))?
				.ok_or_else(|| eyre::eyre!("reply channel closed before the echo arrived"))?;
			Ok(echoed.payload)
		}

		#[tokio::test]
		async fn quiche_outbound_tcp_roundtrip() -> eyre::Result<()> {
			let uuid = Uuid::new_v4();
			let password = "test-password";
			let (addr, _dir) = start_echo_server(uuid, password, ConnectionOpts::default()).await?;

			let outbound = client_builder(addr, uuid, password, TransportUdpRelayMode::Datagram)
				.build()
				.await?;
			outbound.start_poll().await?;

			let target = TargetAddr::IPv4(Ipv4Addr::LOCALHOST, 80);
			let mut io = outbound.connect_tcp(&target).await?;

			for probe in [b"hello".as_slice(), b"tuic-over-quiche".as_slice()] {
				io.write_all(probe).await?;
				io.flush().await?;
				let mut buf = vec![0u8; probe.len()];
				io.read_exact(&mut buf).await?;
				assert_eq!(buf, probe);
			}

			outbound.close();
			Ok(())
		}

		/// Control: the default (datagram) relay mode still round-trips when
		/// both transports advertise DATAGRAM support.
		#[tokio::test]
		async fn quiche_outbound_udp_datagram_roundtrip() -> eyre::Result<()> {
			let uuid = Uuid::new_v4();
			let password = "test-password";
			let (addr, _dir) = start_echo_server(uuid, password, ConnectionOpts::default()).await?;

			let outbound = Arc::new(
				client_builder(addr, uuid, password, TransportUdpRelayMode::Datagram)
					.reconnect(no_reconnect())
					.build()
					.await?,
			);
			outbound.start_poll().await?;

			let echoed = udp_roundtrip(outbound.clone(), b"udp-datagram-probe").await?;
			assert_eq!(echoed, &b"udp-datagram-probe"[..]);

			outbound.close();
			Ok(())
		}

		/// F18: a client on stream relay (`udp_relay_mode = "quic"`) never
		/// advertises DATAGRAM support, so the server must answer on
		/// unidirectional streams. Before the fix the reply took the datagram
		/// path, quiche rejected it (`InvalidState`) and the client saw no echo
		/// at all.
		#[tokio::test]
		async fn quiche_outbound_udp_stream_relay_roundtrip() -> eyre::Result<()> {
			let uuid = Uuid::new_v4();
			let password = "test-password";
			let (addr, _dir) = start_echo_server(uuid, password, ConnectionOpts::default()).await?;

			let outbound = Arc::new(
				client_builder(addr, uuid, password, TransportUdpRelayMode::Stream)
					.reconnect(no_reconnect())
					.build()
					.await?,
			);
			outbound.start_poll().await?;

			let echoed = udp_roundtrip(outbound.clone(), b"udp-stream-relay-probe").await?;
			assert_eq!(echoed, &b"udp-stream-relay-probe"[..]);

			outbound.close();
			Ok(())
		}

		/// The mirror image: the server transport has DATAGRAM disabled while
		/// the client is in datagram mode. The client must fall back to
		/// unidirectional streams for its uplink, otherwise quiche closes the
		/// connection on receipt of a DATAGRAM frame it did not enable. The
		/// server still answers with datagrams (the client can receive them).
		#[tokio::test]
		async fn quiche_outbound_udp_with_server_stream_transport_roundtrip() -> eyre::Result<()> {
			let uuid = Uuid::new_v4();
			let password = "test-password";
			let server_opts = ConnectionOpts {
				udp_relay_mode: TransportUdpRelayMode::Stream,
				..Default::default()
			};
			let (addr, _dir) = start_echo_server(uuid, password, server_opts).await?;

			let outbound = Arc::new(
				client_builder(addr, uuid, password, TransportUdpRelayMode::Datagram)
					.reconnect(no_reconnect())
					.build()
					.await?,
			);
			outbound.start_poll().await?;

			let echoed = udp_roundtrip(outbound.clone(), b"udp-server-stream-transport-probe").await?;
			assert_eq!(echoed, &b"udp-server-stream-transport-probe"[..]);

			outbound.close();
			Ok(())
		}
	}
}
