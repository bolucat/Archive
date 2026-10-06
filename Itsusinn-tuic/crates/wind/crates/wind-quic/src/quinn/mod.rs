//! quinn backend.
//!
//! quinn already exposes a handle-based async API that lines up 1:1 with the
//! [`crate::traits`] surface, so the adapter is a set of thin newtype wrappers
//! plus endpoint/connect construction (mapping the backend-neutral
//! [`TransportConfig`] / TLS configs onto quinn + rustls).

mod tls;
mod udp;

use std::{
	io,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use bytes::Bytes;
use quinn::{
	ClientConfig, Endpoint, EndpointConfig, IdleTimeout, ServerConfig, TokioRuntime, TransportConfig as QuinnTransport, VarInt,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
pub use udp::wrap_server_socket;

use crate::{
	config::{ClientTlsConfig, ServerTlsConfig, TransportConfig},
	error::QuicError,
	traits::{QuicConnection, QuicRecvStream, QuicSendStream},
};

/// quinn send half.
pub struct QuinnSend(quinn::SendStream);

/// quinn recv half.
pub struct QuinnRecv(quinn::RecvStream);

impl AsyncWrite for QuinnSend {
	fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
		Pin::new(&mut self.0).poll_write(cx, buf).map_err(io::Error::other)
	}

	fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.0).poll_flush(cx)
	}

	fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.0).poll_shutdown(cx)
	}
}

impl QuicSendStream for QuinnSend {
	fn finish(&mut self) -> Result<(), QuicError> {
		// `finish` only errors if the stream was already finished/reset, which
		// is a no-op from the caller's perspective.
		let _ = self.0.finish();
		Ok(())
	}

	fn reset(&mut self, code: u64) {
		let _ = self.0.reset(VarInt::from_u64(code).unwrap_or(VarInt::MAX));
	}

	fn id(&self) -> u64 {
		self.0.id().into()
	}
}

impl AsyncRead for QuinnRecv {
	fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.0).poll_read(cx, buf)
	}
}

impl QuicRecvStream for QuinnRecv {
	fn stop(&mut self, code: u64) {
		let _ = self.0.stop(VarInt::from_u64(code).unwrap_or(VarInt::MAX));
	}

	fn id(&self) -> u64 {
		self.0.id().into()
	}
}

/// A [`QuicConnection`] backed by quinn.
///
/// For client connections an `Arc<Endpoint>` is kept alive alongside the
/// connection: quinn drives connection I/O from the endpoint's task, so the
/// endpoint must outlive every connection it owns.
#[derive(Clone)]
pub struct QuinnConnection {
	conn: quinn::Connection,
	_endpoint: Option<Arc<Endpoint>>,
}

impl QuinnConnection {
	/// Wrap an existing quinn connection (its endpoint is kept alive elsewhere,
	/// e.g. by a [`QuinnAcceptor`]).
	pub fn new(conn: quinn::Connection) -> Self {
		Self { conn, _endpoint: None }
	}

	/// The underlying quinn connection.
	pub fn inner(&self) -> &quinn::Connection {
		&self.conn
	}
}

impl QuicConnection for QuinnConnection {
	type RecvStream = QuinnRecv;
	type SendStream = QuinnSend;

	async fn open_bi(&self) -> Result<(QuinnSend, QuinnRecv), QuicError> {
		let (s, r) = self.conn.open_bi().await?;
		Ok((QuinnSend(s), QuinnRecv(r)))
	}

	async fn accept_bi(&self) -> Result<(QuinnSend, QuinnRecv), QuicError> {
		let (s, r) = self.conn.accept_bi().await?;
		Ok((QuinnSend(s), QuinnRecv(r)))
	}

	async fn open_uni(&self) -> Result<QuinnSend, QuicError> {
		let s = self.conn.open_uni().await?;
		Ok(QuinnSend(s))
	}

	async fn accept_uni(&self) -> Result<QuinnRecv, QuicError> {
		let r = self.conn.accept_uni().await?;
		Ok(QuinnRecv(r))
	}

	fn send_datagram(&self, data: Bytes) -> Result<(), QuicError> {
		self.conn.send_datagram(data).map_err(Into::into)
	}

	async fn read_datagram(&self) -> Result<Bytes, QuicError> {
		self.conn.read_datagram().await.map_err(Into::into)
	}

	fn max_datagram_size(&self) -> Option<usize> {
		self.conn.max_datagram_size()
	}

	async fn export_keying_material<'a>(
		&'a self,
		out: &'a mut [u8],
		label: &'a [u8],
		context: &'a [u8],
	) -> Result<(), QuicError> {
		self.conn
			.export_keying_material(out, label, context)
			.map_err(|_| QuicError::Tls("export_keying_material: output length too large".into()))
	}

	fn close(&self, code: u32, reason: &[u8]) {
		self.conn.close(VarInt::from_u32(code), reason);
	}

	async fn closed(&self) {
		self.conn.closed().await;
	}

	async fn authenticated(&self) -> Result<(), QuicError> {
		self.conn.authenticated().await.map_err(Into::into)
	}

	fn peer_addr(&self) -> Option<SocketAddr> {
		Some(self.conn.remote_address())
	}

	async fn byte_stats(&self) -> Option<(u64, u64)> {
		let stats = self.conn.stats();
		Some((stats.udp_tx.bytes, stats.udp_rx.bytes))
	}
}

/// A quinn server endpoint that yields [`QuinnConnection`]s.
pub struct QuinnAcceptor {
	endpoint: Endpoint,
}

impl QuinnAcceptor {
	/// The local address the endpoint is bound to.
	pub fn local_addr(&self) -> io::Result<SocketAddr> {
		self.endpoint.local_addr()
	}

	/// Accept the next inbound connection (after its handshake completes).
	///
	/// Returns `None` once the endpoint is shut down.
	pub async fn accept(&self) -> Option<Result<QuinnConnection, QuicError>> {
		let incoming = self.endpoint.accept().await?;
		Some(match incoming.accept() {
			Ok(connecting) => connecting.await.map(QuinnConnection::new).map_err(Into::into),
			Err(e) => Err(QuicError::ConnectionLost(e.to_string())),
		})
	}
}

/// Bind a quinn server endpoint on `addr` with the given TLS + transport
/// config.
pub fn bind_server(
	addr: SocketAddr,
	tls_cfg: &ServerTlsConfig,
	transport: &TransportConfig,
) -> Result<QuinnAcceptor, QuicError> {
	tls::ensure_provider();
	let crypto = tls::server_crypto(tls_cfg, transport)?;
	let mut server_config = ServerConfig::with_crypto(Arc::new(
		quinn::crypto::rustls::QuicServerConfig::try_from(crypto)
			.map_err(|e| QuicError::Tls(format!("quinn server config: {e}")))?,
	));
	server_config.transport_config(Arc::new(build_transport(transport)?));

	let socket = std::net::UdpSocket::bind(addr).map_err(|e| QuicError::Endpoint(format!("bind {addr}: {e}")))?;
	let socket = wrap_server_socket(socket).map_err(|e| QuicError::Endpoint(format!("wrap server socket: {e}")))?;
	let endpoint =
		Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), socket, Arc::new(TokioRuntime))
			.map_err(|e| QuicError::Endpoint(format!("create endpoint: {e}")))?;
	Ok(QuinnAcceptor { endpoint })
}

/// The local address a client socket binds before dialing `peer`.
///
/// The family must match `peer`: a socket bound to `0.0.0.0` cannot dial an
/// IPv6 peer (and vice versa), so the dial fails before the handshake starts.
///
/// A single wildcard socket is *not* a portable substitute. The unspecified
/// IPv6 address is IPv6-only on Windows — an IPv4 dial on it fails with
/// `WSAEADDRNOTAVAIL` even when `IPV6_V6ONLY` is clear — so an endpoint meant
/// for both families must still be rebuilt per peer family.
pub fn client_bind_addr(peer: SocketAddr) -> SocketAddr {
	if peer.is_ipv6() {
		SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0))
	} else {
		SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))
	}
}

/// Connect to `peer` as a client, returning an established [`QuinnConnection`].
pub async fn connect(
	peer: SocketAddr,
	tls_cfg: &ClientTlsConfig,
	transport: &TransportConfig,
) -> Result<QuinnConnection, QuicError> {
	tls::ensure_provider();
	let crypto = tls::client_crypto(tls_cfg)?;
	let mut client_config = ClientConfig::new(Arc::new(
		quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
			.map_err(|e| QuicError::Tls(format!("quinn client config: {e}")))?,
	));
	client_config.transport_config(Arc::new(build_transport(transport)?));

	// Bind an ephemeral local socket on the unspecified address that matches
	// `peer`'s family.
	let bind_addr = client_bind_addr(peer);
	let socket =
		std::net::UdpSocket::bind(bind_addr).map_err(|e| QuicError::Endpoint(format!("bind client {bind_addr}: {e}")))?;
	let endpoint = Endpoint::new(EndpointConfig::default(), None, socket, Arc::new(TokioRuntime))
		.map_err(|e| QuicError::Endpoint(format!("create client endpoint: {e}")))?;
	endpoint.set_default_client_config(client_config);

	let connection = endpoint
		.connect(peer, &tls_cfg.server_name)
		.map_err(|e| QuicError::ConnectionLost(format!("connect {peer}: {e}")))?
		.await?;
	// Keep the endpoint alive alongside the connection: quinn drives connection
	// I/O from the endpoint's task, so dropping it here would tear the
	// connection down.
	Ok(QuinnConnection {
		conn: connection,
		_endpoint: Some(Arc::new(endpoint)),
	})
}

/// A quinn client endpoint that reuses the same TLS [`ClientConfig`] across
/// multiple connections.
///
/// Unlike [`connect`] — which builds a fresh endpoint *and* client config per
/// call, discarding the in-memory session-ticket store — this struct keeps the
/// config alive, so a second `connect` can resume the TLS session established
/// by the first and replay 0-RTT early data (observable via
/// `Connecting::into_0rtt` / `ZeroRttAccepted`).
///
/// The endpoint binds once, when the struct is built, so it cannot pick a
/// socket family from the peer the way [`connect`] does. The default is
/// `0.0.0.0:0` (IPv4-only, the historical behavior); call
/// [`QuinnClient::with_bind_addr`] to reach IPv6 peers. See
/// [`client_bind_addr`] for why one wildcard socket cannot serve both
/// families.
pub struct QuinnClient {
	endpoint: Endpoint,
	client_config: ClientConfig,
	server_name: String,
}

impl QuinnClient {
	/// Create a client endpoint bound to an ephemeral IPv4 local socket.
	pub async fn new(tls_cfg: &ClientTlsConfig, transport: &TransportConfig) -> Result<Self, QuicError> {
		Self::new_bound(tls_cfg, transport, SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await
	}

	/// Create a client endpoint bound to an ephemeral local socket on
	/// `bind_addr`.
	///
	/// Pass [`client_bind_addr`]`(peer)` to dial a specific peer, or the
	/// unspecified address of the family you need. The address family cannot be
	/// changed later: [`QuinnClient::connect`] fails with
	/// [`QuicError::Endpoint`] when `peer` belongs to the other family.
	pub async fn new_bound(
		tls_cfg: &ClientTlsConfig,
		transport: &TransportConfig,
		bind_addr: SocketAddr,
	) -> Result<Self, QuicError> {
		tls::ensure_provider();
		let crypto = tls::client_crypto(tls_cfg)?;
		let mut client_config = ClientConfig::new(Arc::new(
			quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
				.map_err(|e| QuicError::Tls(format!("quinn client config: {e}")))?,
		));
		client_config.transport_config(Arc::new(build_transport(transport)?));

		let socket =
			std::net::UdpSocket::bind(bind_addr).map_err(|e| QuicError::Endpoint(format!("bind client {bind_addr}: {e}")))?;
		let endpoint = Endpoint::new(EndpointConfig::default(), None, socket, Arc::new(TokioRuntime))
			.map_err(|e| QuicError::Endpoint(format!("create client endpoint: {e}")))?;

		Ok(Self {
			endpoint,
			client_config,
			server_name: tls_cfg.server_name.clone(),
		})
	}

	/// The local socket address this endpoint is bound to.
	pub fn local_addr(&self) -> Result<SocketAddr, QuicError> {
		self.endpoint.local_addr().map_err(|e| QuicError::Endpoint(e.to_string()))
	}

	/// Begin connecting to `peer`, returning the raw quinn `Connecting` so
	/// callers can observe 0-RTT via `into_0rtt` / `ZeroRttAccepted`.
	pub fn connecting(&self, peer: SocketAddr) -> Result<quinn::Connecting, QuicError> {
		// quinn reports a cross-family dial as a generic UDP send error; name
		// the real cause here instead.
		if let Ok(local) = self.endpoint.local_addr()
			&& local.is_ipv6() != peer.is_ipv6()
		{
			return Err(QuicError::Endpoint(format!(
				"endpoint is bound to {local}, which cannot dial the {} peer {peer}",
				if peer.is_ipv6() { "IPv6" } else { "IPv4" }
			)));
		}
		self.endpoint
			.connect_with(self.client_config.clone(), peer, &self.server_name)
			.map_err(|e| QuicError::ConnectionLost(format!("connect {peer}: {e}")))
	}

	/// Connect to `peer` and wait for the handshake to complete.
	pub async fn connect(&self, peer: SocketAddr) -> Result<QuinnConnection, QuicError> {
		let conn = self.connecting(peer)?.await?;
		Ok(QuinnConnection {
			conn,
			_endpoint: Some(Arc::new(self.endpoint.clone())),
		})
	}

	/// The underlying quinn endpoint.
	pub fn endpoint(&self) -> &Endpoint {
		&self.endpoint
	}
}

fn build_transport(t: &TransportConfig) -> Result<QuinnTransport, QuicError> {
	let mut tr = QuinnTransport::default();
	let bidi = VarInt::from_u64(t.max_concurrent_bidi_streams)
		.map_err(|_| QuicError::Other("max_concurrent_bidi_streams out of range".into()))?;
	let uni = VarInt::from_u64(t.max_concurrent_uni_streams)
		.map_err(|_| QuicError::Other("max_concurrent_uni_streams out of range".into()))?;
	// quinn's windows are fixed — it has no init/max auto-tuning — so the
	// per-direction `max_*` overrides map straight onto them and each falls
	// back to the legacy single `receive_window`.
	let stream_window = VarInt::from_u64(t.max_stream_receive_window.unwrap_or(t.receive_window))
		.map_err(|_| QuicError::Other("stream receive window out of range".into()))?;

	tr.max_concurrent_bidi_streams(bidi)
		.max_concurrent_uni_streams(uni)
		.send_window(t.send_window)
		.stream_receive_window(stream_window)
		.initial_mtu(t.initial_mtu)
		.min_mtu(t.min_mtu)
		.enable_segmentation_offload(t.gso);

	// Connection-level receive window. Only set when explicitly configured, so
	// the default keeps quinn's built-in ceiling in place.
	if let Some(max_conn) = t.max_conn_receive_window {
		let conn_window =
			VarInt::from_u64(max_conn).map_err(|_| QuicError::Other("connection receive window out of range".into()))?;
		tr.receive_window(conn_window);
	}

	if let Some(idle) = t.max_idle_timeout {
		let idle = IdleTimeout::try_from(idle).map_err(|_| QuicError::Other("max_idle_timeout out of range".into()))?;
		tr.max_idle_timeout(Some(idle));
	}

	if let Some(factory) = congestion_factory(t) {
		tr.congestion_controller_factory(factory);
	}

	Ok(tr)
}

fn congestion_factory(t: &TransportConfig) -> Option<Arc<dyn quinn::congestion::ControllerFactory + Send + Sync + 'static>> {
	use crate::config::QuicCongestionControl::*;
	let iw = t.initial_window;
	match t.congestion {
		// Use quinn's built-in default controller.
		Default => None,
		// `quinn-congestions` ships a single BBR implementation shared by both
		// the bbr and bbrv2 selectors.
		Bbr | BbrV2 => {
			let mut cfg = quinn_congestions::bbr::BbrConfig::default();
			if let Some(w) = iw {
				cfg.initial_window(w);
			}
			Some(Arc::new(cfg))
		}
		Cubic => {
			let mut cfg = quinn::congestion::CubicConfig::default();
			if let Some(w) = iw {
				cfg.initial_window(w);
			}
			Some(Arc::new(cfg))
		}
		Reno => {
			let mut cfg = quinn::congestion::NewRenoConfig::default();
			if let Some(w) = iw {
				cfg.initial_window(w);
			}
			Some(Arc::new(cfg))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const MIB: u64 = 1024 * 1024;

	/// Read one numeric field out of quinn's `TransportConfig` debug rendering.
	///
	/// quinn keeps every transport field private and exposes only setters, but
	/// its `Debug` impl deliberately lists `stream_receive_window` and
	/// `receive_window` for diagnostics, so that rendering is the only way to
	/// observe what `build_transport` actually applied. The leading space in
	/// the key keeps the lookup for `receive_window` from matching the tail of
	/// `stream_receive_window`.
	fn window_field(tr: &QuinnTransport, field: &str) -> u64 {
		let rendered = format!("{tr:?}");
		let key = format!(" {field}: ");
		let start = rendered
			.find(&key)
			.unwrap_or_else(|| panic!("quinn's TransportConfig debug output has no `{field}` field: {rendered}"))
			+ key.len();
		let rest = &rendered[start..];
		let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
		rest[..end]
			.parse()
			.unwrap_or_else(|e| panic!("`{field}` is not a plain integer in quinn's TransportConfig ({e}): {rendered}"))
	}

	/// `max_conn_receive_window` is documented as quinn's fixed
	/// connection-level `receive_window`, so configuring it must reach
	/// quinn.
	#[test]
	fn configures_the_connection_receive_window() {
		let conn_window = 20 * MIB;
		let t = TransportConfig {
			max_conn_receive_window: Some(conn_window),
			..Default::default()
		};
		let tr = build_transport(&t).expect("transport config");
		assert_eq!(
			window_field(&tr, "receive_window"),
			conn_window,
			"max_conn_receive_window must reach quinn's connection receive_window"
		);
	}

	/// `max_stream_receive_window` is documented as quinn's fixed
	/// `stream_receive_window` and must win over the legacy single knob.
	#[test]
	fn configures_the_per_stream_receive_window() {
		let t = TransportConfig {
			receive_window: 5 * MIB,
			max_stream_receive_window: Some(9 * MIB),
			..Default::default()
		};
		let tr = build_transport(&t).expect("transport config");
		assert_eq!(
			window_field(&tr, "stream_receive_window"),
			9 * MIB,
			"max_stream_receive_window must win over the legacy receive_window"
		);
	}

	/// Without the per-direction overrides the legacy knob keeps sizing the
	/// per-stream window and quinn's own connection ceiling stays untouched —
	/// the documented fallback, and no change to existing defaults.
	#[test]
	fn leaves_unset_windows_at_their_quinn_defaults() {
		let t = TransportConfig {
			receive_window: 5 * MIB,
			..Default::default()
		};
		let tr = build_transport(&t).expect("transport config");
		let default_conn = window_field(&QuinnTransport::default(), "receive_window");
		assert_eq!(window_field(&tr, "receive_window"), default_conn);
		assert_eq!(window_field(&tr, "stream_receive_window"), 5 * MIB);
	}

	/// A window that cannot be represented as a QUIC varint must fail loudly
	/// instead of being dropped on the floor.
	#[test]
	fn rejects_windows_outside_the_varint_range() {
		let conn = TransportConfig {
			max_conn_receive_window: Some(u64::MAX),
			..Default::default()
		};
		assert!(
			build_transport(&conn).is_err(),
			"an out-of-range connection receive window must be rejected"
		);

		let stream = TransportConfig {
			max_stream_receive_window: Some(u64::MAX),
			..Default::default()
		};
		assert!(
			build_transport(&stream).is_err(),
			"an out-of-range stream receive window must be rejected"
		);
	}
}
