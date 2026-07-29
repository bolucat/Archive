//! quinn backend.
//!
//! quinn already exposes a handle-based async API that lines up 1:1 with the
//! [`crate::traits`] surface, so the adapter is a set of thin newtype wrappers
//! plus endpoint/connect construction (mapping the backend-neutral
//! [`TransportConfig`] / TLS configs onto quinn + rustls).

mod tls;

use std::{
	io,
	net::{Ipv4Addr, SocketAddr},
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use bytes::Bytes;
use quinn::{
	ClientConfig, Endpoint, EndpointConfig, IdleTimeout, ServerConfig, TokioRuntime, TransportConfig as QuinnTransport, VarInt,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

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
	let endpoint = Endpoint::new(EndpointConfig::default(), Some(server_config), socket, Arc::new(TokioRuntime))
		.map_err(|e| QuicError::Endpoint(format!("create endpoint: {e}")))?;
	Ok(QuinnAcceptor { endpoint })
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

	// Bind an ephemeral local socket on the unspecified address.
	let bind_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
	let socket = std::net::UdpSocket::bind(bind_addr).map_err(|e| QuicError::Endpoint(format!("bind client: {e}")))?;
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

fn build_transport(t: &TransportConfig) -> Result<QuinnTransport, QuicError> {
	let mut tr = QuinnTransport::default();
	let bidi = VarInt::from_u64(t.max_concurrent_bidi_streams)
		.map_err(|_| QuicError::Other("max_concurrent_bidi_streams out of range".into()))?;
	let uni = VarInt::from_u64(t.max_concurrent_uni_streams)
		.map_err(|_| QuicError::Other("max_concurrent_uni_streams out of range".into()))?;
	let recv_window = VarInt::from_u64(t.receive_window).map_err(|_| QuicError::Other("receive_window out of range".into()))?;

	tr.max_concurrent_bidi_streams(bidi)
		.max_concurrent_uni_streams(uni)
		.send_window(t.send_window)
		.stream_receive_window(recv_window)
		.initial_mtu(t.initial_mtu)
		.min_mtu(t.min_mtu)
		.enable_segmentation_offload(t.gso);

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
