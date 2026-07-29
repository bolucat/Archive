//! TUIC inbound server — quinn backend.
//!
//! This is a thin wrapper: it builds the quinn endpoint (TLS, transport, and
//! congestion config), accepts/handshakes connections, then hands each
//! established connection to the backend-agnostic
//! [`crate::server::serve_connection`] (wrapped as a
//! [`wind_quic::quinn::QuinnConnection`]). All TUIC protocol logic lives in the
//! shared server core.

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use eyre::Context;
use quinn::{Endpoint, EndpointConfig, IdleTimeout, ServerConfig, TokioRuntime, TransportConfig, VarInt};
use rustls::{
	ServerConfig as RustlsServerConfig,
	pki_types::{CertificateDer, PrivateKeyDer},
};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, error, info, warn};
use uuid::Uuid;
use wind_core::{AbstractInbound, AppContext, InboundCallback, InboundHooks};
use wind_quic::quinn::QuinnConnection;

use crate::quinn::CongestionControl;

async fn spawn_logged(label: &str, fut: impl std::future::Future<Output = eyre::Result<()>>) {
	if let Err(err) = fut.await {
		error!("{label} error: {err:?}");
	}
}

pub struct TuicInboundOpts {
	pub listen_addr: SocketAddr,

	pub certificate: Vec<CertificateDer<'static>>,

	pub private_key: PrivateKeyDer<'static>,

	pub cert_resolver: Option<Arc<dyn rustls::server::ResolvesServerCert>>,

	pub alpn: Vec<String>,

	pub users: HashMap<Uuid, String>,

	pub auth_timeout: Duration,

	pub max_idle_time: Duration,

	pub max_concurrent_bi_streams: u32,

	pub max_concurrent_uni_streams: u32,

	pub send_window: u64,

	/// Legacy single receive window; `max_stream_receive_window` /
	/// `max_conn_receive_window` take precedence when set.
	pub receive_window: u32,

	/// Per-stream receive window (bytes) → quinn's fixed
	/// `stream_receive_window`. `None` falls back to `receive_window`. (quinn
	/// has no separate "initial" flow-control window, so init overrides do not
	/// apply here.)
	pub max_stream_receive_window: Option<u64>,

	/// Connection receive window (bytes) → quinn's fixed connection
	/// `receive_window`. `None` leaves quinn's default.
	pub max_conn_receive_window: Option<u64>,

	pub zero_rtt: bool,

	pub initial_mtu: u16,

	pub min_mtu: u16,

	pub gso: bool,

	/// Congestion control algorithm for the QUIC transport.
	pub congestion_control: CongestionControl,

	/// Initial congestion window, in bytes. A larger value lets short-lived
	/// connections (e.g. one TCP-over-QUIC stream per browser request) ramp out
	/// of slow-start faster instead of trickling the first few round trips.
	pub initial_window: u64,

	/// NewReno loss-reduction factor (β). Only applies to the `newreno`
	/// controller; `None` keeps quinn's default. Other controllers ignore it.
	pub newreno_loss_reduction_factor: Option<f32>,

	/// HTTP/3 masquerade. When `Some`, connections that aren't TUIC (their
	/// first stream byte isn't `0x05`) are served as a reverse-proxy HTTP/3
	/// web server instead of being dropped.
	pub masquerade: Option<crate::server::MasqueradeConfig>,

	/// Downstream extensibility hooks (auth / traffic stats / connection
	/// management). Defaults to all-`None` (no behavior change).
	pub hooks: InboundHooks,

	/// Live-connection registry for per-user connection limits + active kick.
	/// Defaults to `None` (no registration). When set, each authenticated
	/// connection registers itself and `kick_user` can drop it.
	pub active: Option<crate::active::ActiveConnections>,
}

impl Default for TuicInboundOpts {
	fn default() -> Self {
		Self {
			listen_addr: "0.0.0.0:443".parse().unwrap(),
			certificate: Vec::new(),
			private_key: PrivateKeyDer::Pkcs8(vec![].into()),
			cert_resolver: None,
			alpn: vec!["h3".to_string()],
			users: HashMap::new(),
			auth_timeout: Duration::from_secs(3),
			max_idle_time: Duration::from_secs(15),
			max_concurrent_bi_streams: 32,
			max_concurrent_uni_streams: 32,
			send_window: 8 * 1024 * 1024,
			receive_window: 8 * 1024 * 1024,
			max_stream_receive_window: None,
			max_conn_receive_window: None,
			zero_rtt: false,
			initial_mtu: 1200,
			min_mtu: 1200,
			gso: true,
			congestion_control: CongestionControl::Bbr,
			initial_window: 1024 * 1024,
			newreno_loss_reduction_factor: None,
			masquerade: None,
			hooks: InboundHooks::default(),
			active: None,
		}
	}
}

pub struct TuicInbound {
	pub ctx: Arc<AppContext>,
	opts: TuicInboundOpts,
	cancel: CancellationToken,
}

impl TuicInbound {
	pub fn new(ctx: Arc<AppContext>, opts: TuicInboundOpts) -> Self {
		Self {
			opts,
			cancel: ctx.token.child_token(),
			ctx,
		}
	}

	fn create_server_config(&self) -> eyre::Result<ServerConfig> {
		let builder = RustlsServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13]).with_no_client_auth();

		let mut crypto = if let Some(resolver) = &self.opts.cert_resolver {
			builder.with_cert_resolver(resolver.clone())
		} else {
			builder
				.with_single_cert(self.opts.certificate.clone(), self.opts.private_key.clone_key())
				.wrap_err("Failed to configure TLS certificate")?
		};

		crypto.alpn_protocols = self.opts.alpn.iter().map(|alpn| alpn.as_bytes().to_vec()).collect();

		if self.opts.zero_rtt {
			// Operators wanting strict replay resistance should leave `zero_rtt`
			// disabled until application-layer nonce/anti-replay is implemented.
			warn!(
				"zero_rtt=true: 0-RTT early data is accepted. TUIC has no application-layer replay protection — \
				 Connect/Packet commands sent as 0-RTT can be replayed."
			);
			crypto.max_early_data_size = u32::MAX;
			crypto.send_half_rtt_data = false;
		} else {
			crypto.max_early_data_size = 0;
			crypto.send_half_rtt_data = false;
		}

		let mut config = ServerConfig::with_crypto(Arc::new(
			quinn::crypto::rustls::QuicServerConfig::try_from(crypto)
				.map_err(|e| eyre::eyre!("Failed to create QUIC server config: {}", e))?,
		));

		// Per-stream receive window: prefer the override, else the legacy single
		// window. quinn's windows are fixed (no init/max auto-tuning), so the
		// `max_*` values map straight onto them.
		let stream_window = self.opts.max_stream_receive_window.unwrap_or(self.opts.receive_window as u64);
		let stream_window = VarInt::from_u64(stream_window).map_err(|_| eyre::eyre!("stream receive window out of range"))?;

		let mut transport = TransportConfig::default();
		transport
			.max_concurrent_bidi_streams(VarInt::from(self.opts.max_concurrent_bi_streams))
			.max_concurrent_uni_streams(VarInt::from(self.opts.max_concurrent_uni_streams))
			.send_window(self.opts.send_window)
			.stream_receive_window(stream_window)
			.max_idle_timeout(Some(
				IdleTimeout::try_from(self.opts.max_idle_time).map_err(|_| eyre::eyre!("Invalid max idle time"))?,
			))
			.initial_mtu(self.opts.initial_mtu)
			.min_mtu(self.opts.min_mtu)
			.enable_segmentation_offload(self.opts.gso)
			.congestion_controller_factory(self.congestion_controller_factory());

		// Connection-level receive window (quinn leaves its default unless set).
		if let Some(conn_window) = self.opts.max_conn_receive_window {
			let conn_window =
				VarInt::from_u64(conn_window).map_err(|_| eyre::eyre!("connection receive window out of range"))?;
			transport.receive_window(conn_window);
		}

		config.transport_config(Arc::new(transport));

		Ok(config)
	}

	/// Build the QUIC congestion-controller factory selected by
	/// [`TuicInboundOpts::congestion_control`], applying the configured initial
	/// window.
	fn congestion_controller_factory(&self) -> Arc<dyn quinn::congestion::ControllerFactory + Send + Sync + 'static> {
		let iw = self.opts.initial_window;
		match self.opts.congestion_control {
			// `quinn-congestions` provides a single BBR implementation; both the
			// `bbr` and `bbr3` config aliases map to it.
			CongestionControl::Bbr | CongestionControl::Bbr3 => {
				let mut cfg = quinn_congestions::bbr::BbrConfig::default();
				cfg.initial_window(iw);
				Arc::new(cfg)
			}
			CongestionControl::Cubic => {
				let mut cfg = quinn::congestion::CubicConfig::default();
				cfg.initial_window(iw);
				Arc::new(cfg)
			}
			CongestionControl::NewReno => {
				let mut cfg = quinn::congestion::NewRenoConfig::default();
				cfg.initial_window(iw);
				if let Some(factor) = self.opts.newreno_loss_reduction_factor {
					cfg.loss_reduction_factor(factor);
				}
				Arc::new(cfg)
			}
		}
	}
}

impl AbstractInbound for TuicInbound {
	async fn listen(&self, cb: &impl InboundCallback) -> eyre::Result<()> {
		let config = self.create_server_config()?;

		let socket = std::net::UdpSocket::bind(self.opts.listen_addr)
			.with_context(|| format!("Failed to bind socket on {}", self.opts.listen_addr))?;

		let endpoint = Endpoint::new(EndpointConfig::default(), Some(config), socket, Arc::new(TokioRuntime))
			.wrap_err("Failed to create QUIC endpoint")?;

		info!("TUIC server listening on {}", endpoint.local_addr().unwrap());

		let users = Arc::new(self.opts.users.clone());

		loop {
			// `endpoint.accept()` returns `None` once the endpoint is shut down;
			// the `else =>` arm catches that as a normal shutdown so the
			// `tokio::select!` doesn't panic when every branch is disabled.
			tokio::select! {
				_ = self.cancel.cancelled() => {
					info!("TUIC server shutting down");
					break;
				}
				Some(incoming) = endpoint.accept() => {
					let opts = &self.opts;
					let users = users.clone();
					let auth_timeout = opts.auth_timeout;
					let zero_rtt = opts.zero_rtt;
					let masquerade = opts.masquerade.clone();
					let hooks = opts.hooks.clone();
					let active = opts.active.clone();
					let cb = cb.clone();
					let conn_cancel = self.cancel.child_token();
					let remote = incoming.remote_address();
					let span = tracing::info_span!(
						"conn",
						peer = %remote,
						id = tracing::field::Empty,
						user = tracing::field::Empty,
					);

					// Spawn into the shared TaskTracker so the context owner can
					// drain connection handlers on shutdown (e.g. wind's
					// `tasks.close()` + `tasks.wait()` after cancelling).
					self.ctx.tasks.spawn(spawn_logged(
						"Connection handler",
						handle_connection(incoming, users, auth_timeout, zero_rtt, masquerade, cb, conn_cancel, hooks, active),
					).instrument(span));
				}
				else => {
					info!("TUIC endpoint closed; shutting down listen loop");
					break;
				}
			}
		}

		// Close every remaining connection (CONNECTION_CLOSE, code 0) and wait
		// for the close packets to flush. Without this, returning here lets the
		// caller drop the runtime while close frames are still queued, so peers
		// only learn about the shutdown via idle timeout.
		endpoint.close(VarInt::from_u32(0), b"server shutdown");
		endpoint.wait_idle().await;

		Ok(())
	}
}

/// Complete the quinn handshake (incl. optional 0-RTT) for one incoming
/// connection, then drive it through the backend-agnostic server core.
#[allow(clippy::too_many_arguments)]
async fn handle_connection<C: InboundCallback>(
	incoming: quinn::Incoming,
	users: Arc<HashMap<Uuid, String>>,
	auth_timeout: Duration,
	zero_rtt: bool,
	masquerade: Option<crate::server::MasqueradeConfig>,
	callback: C,
	cancel: CancellationToken,
	hooks: InboundHooks,
	active: Option<crate::active::ActiveConnections>,
) -> eyre::Result<()> {
	let remote_addr = incoming.remote_address();

	let connecting = match incoming.accept() {
		Err(e) => {
			error!("Failed to accept connection: {:?}", e);
			return Ok(());
		}
		Ok(conn) => conn,
	};

	let handshake_timeout = auth_timeout.saturating_mul(2);

	let conn = if zero_rtt {
		match connecting.into_0rtt() {
			// This quinn rev's `into_0rtt` yields `(Connection, ZeroRttAccepted)`;
			// the accepted-future is unused here (we just proceed with the
			// connection, as the newer API's single-value form did).
			Ok((conn, _zero_rtt_accepted)) => {
				info!("accepted 0-RTT connection");
				conn
			}
			Err(connecting) => {
				let conn = tokio::time::timeout(handshake_timeout, connecting)
					.await
					.map_err(|_| eyre::eyre!("QUIC handshake timed out after {:?}", handshake_timeout))?
					.wrap_err("Failed to establish QUIC connection")?;
				info!("accepted 1-RTT connection");
				conn
			}
		}
	} else {
		let conn = tokio::time::timeout(handshake_timeout, connecting)
			.await
			.map_err(|_| eyre::eyre!("QUIC handshake timed out after {:?}", handshake_timeout))?
			.wrap_err("Failed to establish QUIC connection")?;
		info!("accepted connection");
		conn
	};

	crate::server::serve_connection(
		QuinnConnection::new(conn),
		remote_addr,
		users,
		auth_timeout,
		callback,
		cancel,
		masquerade,
		hooks,
		active,
	)
	.await;

	Ok(())
}
