use std::{
	future::Future,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	pin::Pin,
	sync::{Arc, atomic::AtomicU16},
	time::Duration,
};

use arc_swap::ArcSwap;
use async_trait::async_trait;
use bytes::Buf;
use moka::future::Cache;
use quinn::TokioRuntime;
use tokio::{io::AsyncReadExt as _, net::UdpSocket};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument as _, info, warn};
use uuid::Uuid;
use wind_core::{AppContext, FlowContext, Outbound, tcp::AbstractTcpStream, types::TargetAddr};
use wind_quic::{
	QuicConnection as _,
	quinn::{QuinnConnection, QuinnRecv, QuinnSend},
};

use crate::{
	Error,
	client::ClientTaskExt,
	proto::{ClientProtoExt, UdpRelayMode, UdpStream as TuicUdpStream, open_connect_stream},
	quinn::utils::{CongestionControl, congestion_controller_factory},
};

/// Upper bound on a single TUIC UDP command frame read from a uni stream:
/// header (2) + command (8) + longest address (259) + max payload (`u16`).
const MAX_PACKET_FRAME: usize = 10 + 259 + u16::MAX as usize;

/// Creates the local UDP socket the QUIC endpoint binds to.
///
/// Wind calls the factory with the peer address the endpoint will dial, so
/// the caller can select the matching address family and skip interface
/// binding for loopback peers. This lets a consumer apply platform socket
/// options (for example Linux `SO_MARK` or bind-to-device) or bind a chosen
/// interface before quinn takes ownership of the socket.
pub type UdpSocketFactory = Arc<dyn Fn(SocketAddr) -> std::io::Result<std::net::UdpSocket> + Send + Sync + 'static>;

/// Resolves the server's current socket address before a reconnect attempt.
///
/// Supplied by a consumer that owns DNS resolution (e.g. clash-rs) so a
/// domain-configured peer follows DNS rotation, failover, or record changes
/// instead of pinning the address resolved when the outbound was built. It is
/// only consulted on reconnect; the initial connection uses `peer_addr`. The
/// error is only logged, so it is intentionally dependency-free.
pub type PeerResolver =
	Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<SocketAddr, String>> + Send>> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct TuicOutboundOpts {
	pub peer_addr: SocketAddr,
	/// Optional resolver consulted before each reconnect attempt so a
	/// domain-configured peer can follow DNS changes / address rotation.
	///
	/// When `None` (or when it returns an error), the last known peer address
	/// is reused. The initial connection always uses `peer_addr`.
	pub peer_resolver: Option<PeerResolver>,
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
	/// Prebuilt rustls client configuration. When `Some` it is used verbatim
	/// instead of the built-in [`super::tls::tls_config`], so a consumer can
	/// keep its own certificate verifier, mTLS client certificate, ALPN list,
	/// and SNI policy.
	pub client_config: Option<Arc<rustls::ClientConfig>>,
	/// QUIC congestion controller.
	pub congestion_control: CongestionControl,
	/// Maximum concurrent bidirectional streams; `None` keeps quinn's default.
	pub max_concurrent_bi_streams: Option<u32>,
	/// Maximum concurrent unidirectional streams; `None` keeps quinn's
	/// default.
	pub max_concurrent_uni_streams: Option<u32>,
	/// Transport send window (bytes); `None` keeps quinn's default.
	pub send_window: Option<u64>,
	/// Per-stream receive window (bytes); `None` keeps quinn's default.
	pub stream_receive_window: Option<u64>,
	/// Maximum idle time before the connection is closed; `None` keeps quinn's
	/// default.
	pub max_idle_time: Option<Duration>,
	/// How outgoing `Packet` commands are relayed (datagram vs uni stream).
	pub udp_relay_mode: UdpRelayMode,
	/// Optional factory for the local UDP socket used by the QUIC endpoint.
	///
	/// When `Some`, Wind hands the returned socket to quinn instead of binding
	/// its own, letting the caller pre-apply platform socket options or bind a
	/// specific interface. When `None`, Wind binds an ephemeral socket on the
	/// unspecified address matching the peer's address family.
	pub socket_factory: Option<UdpSocketFactory>,
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
			// A caller-supplied config wins: it may carry a custom verifier,
			// mTLS material, ALPN list, or SNI policy the built-in config
			// cannot express.
			let tls_config = match &opts.client_config {
				Some(cfg) => (**cfg).clone(),
				None => super::tls::tls_config(&server_name, &opts)?,
			};

			let mut client_config = quinn::ClientConfig::new(Arc::new(
				quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
					.map_err(|e| eyre::eyre!("Failed to build QUIC client config: {e}"))?,
			));
			let mut transport_config = quinn::TransportConfig::default();
			transport_config
				.congestion_controller_factory(congestion_controller_factory(opts.congestion_control))
				.keep_alive_interval(None);
			if let Some(v) = opts.max_concurrent_bi_streams {
				transport_config.max_concurrent_bidi_streams(v.into());
			}
			if let Some(v) = opts.max_concurrent_uni_streams {
				transport_config.max_concurrent_uni_streams(v.into());
			}
			if let Some(v) = opts.send_window {
				transport_config.send_window(v);
			}
			if let Some(v) = opts.stream_receive_window {
				transport_config.stream_receive_window(
					quinn::VarInt::from_u64(v)
						.map_err(|_| eyre::eyre!("stream_receive_window {v} exceeds the QUIC VarInt range"))?,
				);
			}
			if let Some(v) = opts.max_idle_time {
				transport_config.max_idle_timeout(Some(
					quinn::IdleTimeout::try_from(v).map_err(|_| eyre::eyre!("invalid max_idle_time {v:?}"))?,
				));
			}

			client_config.transport_config(Arc::new(transport_config));
			client_config
		};
		// The caller may supply the socket (e.g. to set Linux `SO_MARK` or bind
		// an interface) so policy routing and TUN setups keep working.
		// Otherwise bind the local socket in the same address family as the
		// peer: a quinn endpoint bound to 0.0.0.0 cannot dial an IPv6 peer --
		// `connect` returns `InvalidRemoteAddress` -- so an IPv6 server
		// (`[::1]:8444`) was unreachable when we always bound IPv4.
		let socket = bind_endpoint_socket(peer_addr, opts.socket_factory.as_ref())
			.await
			.map_err(|e| eyre::eyre!("Failed to create UDP socket for {}: {}", peer_addr, e))?;

		let endpoint = quinn::Endpoint::new(quinn::EndpointConfig::default(), None, socket, Arc::new(TokioRuntime))?;
		endpoint.set_default_client_config(client_config);

		// Establish and authenticate the initial connection. The reconnect
		// supervisor reuses the same configured endpoint via
		// `connect_and_auth`.
		let connection = connect_and_auth(&endpoint, peer_addr, &server_name, &opts.auth, opts.zero_rtt_handshake).await?;

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
		let zero_rtt = self.opts.zero_rtt_handshake;
		let reconnect = self.opts.reconnect.clone();
		let peer_resolver = self.opts.peer_resolver.clone();
		let socket_factory = self.opts.socket_factory.clone();
		let ctx = self.ctx.clone();
		let udp_session = self.udp_session.clone();

		// Connection supervisor: run one session (heartbeat + incoming
		// handling) over the live connection until it drops or we shut down.
		// On an unexpected drop, reconnect with backoff and swap the fresh
		// connection into `connection_cell` so `handle_tcp` / `handle_udp`
		// transparently use it. In-flight streams on the old connection are
		// NOT resurrected — callers see them close and retry, getting new
		// streams on the new connection.
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
						// connection immediately instead of waiting out its
						// idle timeout.
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
				// Reconnect disabled: close and stop, mirroring the
				// pre-reconnect behaviour where a dropped connection ended
				// the poll task.
				if !reconnect.enabled {
					warn!(target: "tuic_out", "Reconnect disabled; connection to {} will not be re-established", peer_addr);
					conn.close(0, b"client connection lost");
					return eyre::Ok(());
				}
				// Abandon the dead connection explicitly so the server reaps
				// it.
				conn.close(0, b"reconnecting");

				match reconnect_loop(
					&endpoint,
					peer_addr,
					peer_resolver.as_ref(),
					&sni,
					&auth,
					zero_rtt,
					socket_factory.as_ref(),
					&reconnect,
					&shutdown,
				)
				.await
				{
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

	/// Open a TUIC TCP stream to `target`.
	///
	/// Opens a bidirectional QUIC stream, writes the `Connect` command header,
	/// and returns the joined stream for the caller to relay. This is the raw
	/// half of [`Outbound::handle_tcp`], exposed so a consumer that owns its
	/// own dispatcher (e.g. clash-rs) can reuse the connection supervisor
	/// without adopting wind's `Outbound` control flow.
	pub async fn connect_tcp(&self, target: &TargetAddr) -> Result<tokio::io::Join<QuinnRecv, QuinnSend>, Error> {
		let connection = self.connection.load_full();
		let (send, recv) = open_connect_stream(connection.as_ref(), target).await?;
		Ok(tokio::io::join(recv, send))
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

/// Bind the local UDP socket the QUIC endpoint uses to reach `peer`.
///
/// Prefers the caller's [`UdpSocketFactory`] so platform socket options
/// (interface binding, Linux `SO_MARK`) are applied. Otherwise binds an
/// ephemeral socket on the unspecified address matching the peer's family:
/// a quinn endpoint bound to `0.0.0.0` cannot dial an IPv6 peer.
async fn bind_endpoint_socket(peer: SocketAddr, factory: Option<&UdpSocketFactory>) -> std::io::Result<std::net::UdpSocket> {
	match factory {
		Some(factory) => factory(peer),
		None => {
			let socket_addr = if peer.is_ipv6() {
				SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))
			} else {
				SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))
			};
			Ok(UdpSocket::bind(socket_addr).await?.into_std()?)
		}
	}
}

/// Whether `peer`'s address family differs from the endpoint's local socket,
/// meaning the endpoint must be rebound before dialing `peer` (quinn rejects
/// an IPv6 peer on an IPv4 socket).
fn peer_family_differs(local: SocketAddr, peer: SocketAddr) -> bool {
	local.is_ipv6() != peer.is_ipv6()
}

/// Open a fresh QUIC connection on `endpoint` and complete the TUIC auth
/// handshake. Shared by initial connect ([`TuicOutbound::new`]) and reconnect.
async fn connect_and_auth(
	endpoint: &quinn::Endpoint,
	peer_addr: SocketAddr,
	sni: &str,
	auth: &(Uuid, Arc<[u8]>),
	zero_rtt: bool,
) -> Result<QuinnConnection, Error> {
	let connecting = endpoint
		.connect(peer_addr, sni)
		.map_err(|e| eyre::eyre!("Failed to connect to {} ({}): {}", peer_addr, sni, e))?;

	// A resumed session lets quinn send 0-RTT early data. The TUIC auth token
	// is derived from the TLS keying-material exporter, which rustls only
	// exposes once the handshake completes, so the auth command itself cannot
	// be early data: take the 0-RTT connection handle, wait for the handshake,
	// then authenticate over 1-RTT. A server that rejects 0-RTT still completes
	// the handshake, so the fallback is transparent.
	let raw = if zero_rtt {
		match connecting.into_0rtt() {
			Ok(conn) => {
				conn.authenticated().await?;
				info!(target: "tuic_out", "0-RTT resumption accepted");
				conn
			}
			// No cached session ticket (e.g. the very first connection) — fall
			// back to a full 1-RTT handshake.
			Err(connecting) => connecting.await?,
		}
	} else {
		connecting.await?
	};

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
///
/// When a [`PeerResolver`] is supplied it is consulted before every attempt so
/// DNS rotation / failover is followed; a resolution error reuses the last
/// known address. If the peer's address family no longer matches the
/// endpoint's local socket, the endpoint is rebound (via the caller's socket
/// factory when present) so the new family is reachable.
#[allow(clippy::too_many_arguments)]
async fn reconnect_loop(
	endpoint: &quinn::Endpoint,
	peer_addr: SocketAddr,
	peer_resolver: Option<&PeerResolver>,
	sni: &str,
	auth: &(Uuid, Arc<[u8]>),
	zero_rtt: bool,
	socket_factory: Option<&UdpSocketFactory>,
	reconnect: &ReconnectConfig,
	shutdown: &CancellationToken,
) -> Option<QuinnConnection> {
	let mut backoff = reconnect.initial_backoff;
	let mut current_peer = peer_addr;

	loop {
		if let Some(resolver) = peer_resolver {
			match resolver().await {
				Ok(addr) => current_peer = addr,
				Err(e) => {
					warn!(target: "tuic_out", "Peer re-resolution for {sni} failed: {e}; reusing {current_peer}");
				}
			}
		}

		// A peer that changed address family cannot be reached from the
		// endpoint's current local socket (quinn rejects an IPv6 peer on an
		// IPv4 socket). Rebind before dialing so the new family is reachable.
		let family_changed = endpoint
			.local_addr()
			.map(|local| peer_family_differs(local, current_peer))
			.unwrap_or(false);
		if family_changed {
			match bind_endpoint_socket(current_peer, socket_factory).await {
				Ok(socket) => {
					if let Err(e) = endpoint.rebind(socket) {
						warn!(target: "tuic_out", "Failed to rebind endpoint socket for {current_peer}: {e}");
					}
				}
				Err(e) => {
					warn!(target: "tuic_out", "Failed to create endpoint socket for {current_peer}: {e}");
				}
			}
		}

		// Race the connect attempt against shutdown so a hung handshake (e.g.
		// the server is still down) doesn't delay a graceful exit.
		let attempt = tokio::select! {
			_ = shutdown.cancelled() => return None,
			r = connect_and_auth(endpoint, current_peer, sni, auth, zero_rtt) => r,
		};
		match attempt {
			Ok(conn) => return Some(conn),
			Err(e) => {
				warn!(target: "tuic_out", "Reconnect to {} failed: {e}; retrying in {:?}", current_peer, backoff);
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
async fn dispatch_incoming_udp(udp_session: &Cache<u16, Arc<TuicUdpStream<QuinnConnection>>>, mut buf: bytes::Bytes) {
	let header = match crate::proto::decode_header(&mut buf, "datagram") {
		Ok(h) => h,
		Err(e) => {
			warn!(target: "tuic_out", "Failed to decode header: {}", e);
			return;
		}
	};

	let cmd = match crate::proto::decode_command(header.command, &mut buf, "datagram") {
		Ok(c) => c,
		Err(e) => {
			warn!(target: "tuic_out", "Failed to decode command: {}", e);
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
		warn!(target: "tuic_out", "Received non-Packet command in datagram: {:?}", cmd);
		return;
	};

	let addr = match crate::proto::decode_address(&mut buf, "UDP packet") {
		Ok(a) => a,
		Err(e) => {
			warn!(target: "tuic_out", "Failed to decode address: {}", e);
			return;
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
			size,
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
		info!(target: "tuic_out", "Received UDP packet: assoc={:#06x}, pkt={}, frag={}/{}, size={}, target={}",
			assoc_id, pkt_id, frag_id + 1, frag_total, size, target);
	} else {
		info!(target: "tuic_out", "Received UDP fragment: assoc={:#06x}, pkt={}, frag={}/{}, size={} (no address - non-first fragment)",
			assoc_id, pkt_id, frag_id + 1, frag_total, size);
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
			warn!(target: "tuic_out", "Failed to send packet to UDP session {:#06x}: {}", assoc_id, e);
		}
	} else {
		warn!(target: "tuic_out", "Received UDP packet for unknown association {:#06x}", assoc_id);
	}
}

pub struct TuicTcpStream;

#[async_trait]
impl Outbound for TuicOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, mut stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let mut tcp = self.connect_tcp(&ctx.target).await?;
		let (_, _, err) = wind_core::io::copy_io(&mut stream, &mut tcp).await;
		if let Some(e) = err {
			return Err(e.into());
		}
		Ok(())
	}

	async fn handle_udp(&self, _ctx: FlowContext, client_stream: wind_core::udp::UdpStream) -> eyre::Result<()> {
		use std::sync::atomic::Ordering;
		let cancel = self.token.child_token();

		// Allocate a u16 association id, skipping ids that already have a live
		// session. Plain `fetch_add` would wrap silently into an active slot
		// and `udp_session.insert` would overwrite the previous
		// Arc<UdpStream>, dropping any in-flight packets for the original
		// session and confusing the peer (which still routes by the
		// now-stolen id). We probe up to `u16::MAX` candidates and refuse to
		// allocate if every slot is in use.
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

		// Snapshot the live connection for this association. If a reconnect
		// swaps the connection later, this session's streams die and the
		// caller retries.
		let connection = self.connection.load_full().as_ref().clone();
		// Separate handle for the task: it lets the bridge observe the
		// association's own connection closing (on reconnect or any loss) and
		// end the session, while `connection` stays available for teardown.
		let assoc_connection = connection.clone();
		let (receive_tx, receive_rx) = crossfire::mpmc::bounded_async(256);
		let tuic_stream = Arc::new(
			crate::proto::UdpStream::new(connection.clone(), assoc_id, receive_tx)
				.with_relay_mode(self.opts.udp_relay_mode)
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
						info!(target: "tuic_out", "UDP stream sender for association {:#06x} cancelled", assoc_id);
						break;
					}

					// The connection this association was created on is gone
					// (reconnect, peer close, or idle timeout). Its streams can
					// no longer carry traffic, so end the session: this drops
					// `client_tx`/`client_rx` and, once teardown removes the
					// `udp_session` entry, `receive_tx`, which closes the
					// caller's stream and lets it recreate the association on
					// the new connection instead of blackholing silently.
					_ = assoc_connection.closed() => {
						info!(target: "tuic_out", "Connection lost; ending UDP association {:#06x}", assoc_id);
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

		// Wait for the session to end: either the bridge task finished (the
		// local UDP channel closed, the remote errored, or `cancel` fired) or
		// the process is shutting down. The previous version looped forever
		// on the global token only, so once a session's bridge task exited on
		// its own, `handle_udp` kept spinning — pinning the `udp_session`
		// cache entry and never sending a `Dissociate`. A long-lived client
		// that churns UDP associations thus leaked one task slot and one
		// assoc id per dead session until the u16 assoc space was exhausted.
		tokio::select! {
			_ = handle => {}
			_ = self.ctx.token.cancelled() => {}
		}

		// Tear down: stop the bridge task if it is still running
		// (global-shutdown path), release the association id, and tell the
		// peer to dissociate. `Dissociate` goes on the association's own
		// connection: after a reconnect the live connection never knew this
		// assoc id, so loading the current one here would target the wrong
		// peer state (or a reused id).
		cancel.cancel();
		self.udp_session.remove(&assoc_id).await;
		if let Err(err) = connection.drop_udp(assoc_id).await {
			info!(target: "tuic_out", "Error dropping UDP association {:#06x}: {}", assoc_id, err);
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use std::{net::SocketAddr, time::Duration};

	use super::{ReconnectConfig, next_backoff, peer_family_differs};

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
	fn peer_family_differs_only_across_families() {
		let v4: SocketAddr = "127.0.0.1:1".parse().unwrap();
		let v6: SocketAddr = "[::1]:1".parse().unwrap();
		assert!(!peer_family_differs(v4, v4));
		assert!(!peer_family_differs(v6, v6));
		assert!(peer_family_differs(v4, v6));
		assert!(peer_family_differs(v6, v4));
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
