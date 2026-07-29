//! quiche / tokio-quiche backend.
//!
//! tokio-quiche is sans-IO and callback-driven, so this backend runs a generic
//! [`BridgeDriver`] in the per-connection worker and exposes the handle-based
//! [`QuicConnection`](crate::traits::QuicConnection) surface via channels. See
//! [`driver`] for the bridge mechanics.

pub mod conn;
pub mod driver;
pub mod stream;
pub mod tls;

use std::{
	net::{Ipv4Addr, SocketAddr},
	sync::Arc,
};

pub use conn::QuicheConnection;
use futures_util::StreamExt as _;
pub use stream::{QuicheRecv, QuicheSend};
pub use tls::CertStore;
use tokio::{net::UdpSocket, sync::mpsc};
use tokio_quiche::{
	ConnectionParams,
	metrics::DefaultMetrics,
	quic::connect_with_config,
	settings::{BbrParamsField, CertificateKind, Hooks, QuicSettings, TlsCertificatePaths},
	socket::Socket,
};
use tracing::warn;

use crate::{
	config::{CertSource, ClientTlsConfig, ServerTlsConfig, TransportConfig},
	error::QuicError,
	quiche::{driver::BridgeDriver, tls::CertReloadHook},
};

/// Map the shared congestion-control selector onto a quiche cc-algorithm name.
fn cc_name(cc: crate::config::QuicCongestionControl) -> &'static str {
	use crate::config::QuicCongestionControl::*;
	match cc {
		Default | Cubic => "cubic",
		Bbr => "bbr",
		BbrV2 => "bbr2",
		Reno => "reno",
	}
}

/// Convert the backend-neutral [`Bbr2gcConfig`](crate::config::Bbr2gcConfig)
/// onto quiche's native experimental `BbrParams`.
fn quiche_bbr_params(c: &crate::config::Bbr2gcConfig) -> tokio_quiche::quiche::BbrParams {
	use tokio_quiche::quiche::BbrBwLoReductionStrategy as QuicheStrategy;

	use crate::config::BbrBwLoReductionStrategy as Strategy;
	tokio_quiche::quiche::BbrParams {
		startup_cwnd_gain: c.startup_cwnd_gain,
		startup_pacing_gain: c.startup_pacing_gain,
		full_bw_threshold: c.full_bw_threshold,
		startup_full_bw_rounds: c.startup_full_bw_rounds,
		startup_full_loss_count: c.startup_full_loss_count,
		drain_cwnd_gain: c.drain_cwnd_gain,
		drain_pacing_gain: c.drain_pacing_gain,
		enable_reno_coexistence: c.enable_reno_coexistence,
		enable_overestimate_avoidance: c.enable_overestimate_avoidance,
		choose_a0_point_fix: c.choose_a0_point_fix,
		probe_bw_probe_up_pacing_gain: c.probe_bw_probe_up_pacing_gain,
		probe_bw_probe_down_pacing_gain: c.probe_bw_probe_down_pacing_gain,
		probe_bw_cwnd_gain: c.probe_bw_cwnd_gain,
		probe_bw_up_cwnd_gain: c.probe_bw_up_cwnd_gain,
		probe_rtt_pacing_gain: c.probe_rtt_pacing_gain,
		probe_rtt_cwnd_gain: c.probe_rtt_cwnd_gain,
		max_probe_up_queue_rounds: c.max_probe_up_queue_rounds,
		loss_threshold: c.loss_threshold,
		use_bytes_delivered_for_inflight_hi: c.use_bytes_delivered_for_inflight_hi,
		decrease_startup_pacing_at_end_of_round: c.decrease_startup_pacing_at_end_of_round,
		bw_lo_reduction_strategy: c.bw_lo_reduction_strategy.map(|s| match s {
			Strategy::Default => QuicheStrategy::Default,
			Strategy::MinRtt => QuicheStrategy::MinRttReduction,
			Strategy::Inflight => QuicheStrategy::InflightReduction,
			Strategy::Cwnd => QuicheStrategy::CwndReduction,
		}),
		ignore_app_limited_for_no_bandwidth_growth: c.ignore_app_limited_for_no_bandwidth_growth,
		initial_pacing_rate_bytes_per_second: c.initial_pacing_rate_bytes_per_second.map(|b| b.0),
		scale_pacing_rate_by_mss: c.scale_pacing_rate_by_mss,
		disable_probe_down_early_exit: c.disable_probe_down_early_exit,
		time_sent_set_to_now: c.time_sent_set_to_now,
	}
}

/// Translate the backend-neutral [`TransportConfig`] into a [`QuicSettings`].
fn quic_settings(t: &TransportConfig) -> QuicSettings {
	let mut s = QuicSettings::default();
	s.max_idle_timeout = t.max_idle_timeout;
	s.initial_max_streams_bidi = t.max_concurrent_bidi_streams;
	s.initial_max_streams_uni = t.max_concurrent_uni_streams;
	s.cc_algorithm = cc_name(t.congestion).to_string();
	// Size the flow-control windows. The per-direction overrides take precedence;
	// each falls back to the legacy single `receive_window`. The `max_*` windows
	// are quiche's auto-tuning ceilings — only set when explicitly configured so
	// the default leaves quiche's built-in ceiling in place.
	let conn_init = t.init_conn_receive_window.unwrap_or(t.receive_window);
	let stream_init = t.init_stream_receive_window.unwrap_or(t.receive_window);
	s.initial_max_data = conn_init;
	s.initial_max_stream_data_bidi_local = stream_init;
	s.initial_max_stream_data_bidi_remote = stream_init;
	s.initial_max_stream_data_uni = stream_init;
	if let Some(max_conn) = t.max_conn_receive_window {
		s.max_connection_window = max_conn;
	}
	if let Some(max_stream) = t.max_stream_receive_window {
		s.max_stream_window = max_stream;
	}
	s.enable_dgram = t.enable_datagram;
	s.enable_early_data = t.enable_0rtt;
	s.alpn = t.alpn.clone();

	// Per-algorithm congestion-control tuning. `None` leaves quiche's default.
	let cc = &t.cc;
	if let Some(packets) = cc.initial_cwnd_packets {
		s.initial_congestion_window_packets = packets;
	}
	if let Some(pacing) = cc.pacing {
		s.enable_pacing = pacing;
	}
	if let Some(rate) = cc.max_pacing_rate {
		s.max_pacing_rate = Some(rate);
	}
	if let Some(hystart) = cc.hystart {
		s.enable_hystart = hystart;
	}
	if let Some(fix) = cc.cubic_idle_restart_fix {
		s.enable_cubic_idle_restart_fix = fix;
	}
	if let Some(bbr) = &cc.bbr {
		s.custom_bbr_params = BbrParamsField(Some(quiche_bbr_params(bbr)));
	}
	s
}

/// A quiche server acceptor yielding established [`QuicheConnection`]s.
///
/// The listener loop runs in a background task; handshakes complete off-loop
/// and established handles arrive on this channel.
pub struct QuicheAcceptor {
	rx: mpsc::UnboundedReceiver<QuicheConnection>,
	local_addr: SocketAddr,
}

impl QuicheAcceptor {
	/// The local address the listener is bound to.
	pub fn local_addr(&self) -> SocketAddr {
		self.local_addr
	}

	/// Accept the next established connection. Returns `None` once the listener
	/// has stopped.
	pub async fn accept(&mut self) -> Option<QuicheConnection> {
		self.rx.recv().await
	}
}

/// Bind a quiche server listener on `addr`.
///
/// The quiche backend loads TLS credentials from file paths, so `tls_cfg` must
/// use [`CertSource::PemPaths`].
///
/// When `cert_store` is `Some`, a per-handshake [`ConnectionHook`] serves the
/// store's *current* certificate, enabling live rotation (e.g. ACME renewal)
/// without restarting the listener. The certificate files in `tls_cfg` are
/// still required by tokio-quiche's API, but the hook supersedes them for the
/// actual TLS context.
///
/// [`ConnectionHook`]: tokio_quiche::quic::ConnectionHook
pub async fn bind_server(
	addr: SocketAddr,
	tls_cfg: &ServerTlsConfig,
	transport: &TransportConfig,
	cert_store: Option<&CertStore>,
) -> Result<QuicheAcceptor, QuicError> {
	let (cert, key) = match &tls_cfg.cert {
		CertSource::PemPaths { cert, key } => (cert.clone(), key.clone()),
		CertSource::PemBytes { .. } => {
			return Err(QuicError::Tls(
				"quiche backend requires certificate file paths (CertSource::PemPaths)".into(),
			));
		}
	};

	let socket = UdpSocket::bind(addr)
		.await
		.map_err(|e| QuicError::Endpoint(format!("bind {addr}: {e}")))?;
	let local_addr = socket
		.local_addr()
		.map_err(|e| QuicError::Endpoint(format!("local_addr: {e}")))?;

	let hooks = Hooks {
		connection_hook: cert_store.map(|s| Arc::new(CertReloadHook::new(s.clone())) as _),
	};

	let params = ConnectionParams::new_server(
		quic_settings(transport),
		TlsCertificatePaths {
			cert: &cert,
			private_key: &key,
			kind: CertificateKind::X509,
		},
		hooks,
	);

	let mut listeners = tokio_quiche::listen([socket], params, DefaultMetrics)
		.map_err(|e| QuicError::Endpoint(format!("tokio-quiche listen: {e}")))?;
	let mut stream = listeners.remove(0);

	let (conn_tx, conn_rx) = mpsc::unbounded_channel();

	tokio::spawn(async move {
		loop {
			let item = tokio::select! {
				// All `QuicheAcceptor`s dropped — stop accepting and drop the
				// listener stream so the underlying socket/router shut down
				// instead of accepting connections nobody will ever serve.
				_ = conn_tx.closed() => break,
				item = stream.next() => match item {
					Some(item) => item,
					None => break,
				},
			};
			match item {
				Ok(conn) => {
					let peer = conn.peer_addr();
					let span = tracing::info_span!("quiche-conn", peer = %peer);
					let (driver, est_rx) = BridgeDriver::new(true, peer, span);
					conn.start(driver);
					let conn_tx = conn_tx.clone();
					// Forward the handle once the handshake completes, without
					// blocking the accept loop.
					tokio::spawn(async move {
						if let Ok(handle) = est_rx.await {
							let _ = conn_tx.send(handle);
						}
					});
				}
				Err(e) => warn!("wind-quic quiche listener error: {e}"),
			}
		}
	});

	Ok(QuicheAcceptor { rx: conn_rx, local_addr })
}

/// Connect to `peer` as a client, returning an established
/// [`QuicheConnection`].
pub async fn connect(
	peer: SocketAddr,
	tls_cfg: &ClientTlsConfig,
	transport: &TransportConfig,
) -> Result<QuicheConnection, QuicError> {
	let bind_addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0));
	let udp = UdpSocket::bind(bind_addr)
		.await
		.map_err(|e| QuicError::Endpoint(format!("bind client: {e}")))?;
	udp.connect(peer)
		.await
		.map_err(|e| QuicError::Endpoint(format!("connect socket {peer}: {e}")))?;
	let socket: Socket<_, _> = udp
		.try_into()
		.map_err(|e: std::io::Error| QuicError::Endpoint(format!("socket: {e}")))?;

	let mut settings = quic_settings(transport);
	settings.verify_peer = tls_cfg.verify_certificate;
	settings.alpn = tls_cfg.alpn.clone();
	let params = ConnectionParams::new_client(settings, None, Hooks { connection_hook: None });

	let span = tracing::info_span!("quiche-conn", peer = %peer);
	let (driver, est_rx) = BridgeDriver::new(false, peer, span);

	// `connect_with_config` resolves once the handshake completes; the driver's
	// `on_conn_established` delivers the handle through `est_rx`.
	let _qconn = connect_with_config(socket, Some(&tls_cfg.server_name), &params, driver)
		.await
		.map_err(|e| QuicError::ConnectionLost(format!("quiche connect: {e}")))?;

	est_rx
		.await
		.map_err(|_| QuicError::ConnectionLost("handshake aborted before handle delivery".into()))
}

#[cfg(test)]
mod tests {
	use super::*;

	const MIB: u64 = 1024 * 1024;

	#[test]
	fn quic_settings_window_overrides() {
		let t = TransportConfig {
			receive_window: 8 * MIB,
			init_stream_receive_window: Some(8 * MIB),
			max_stream_receive_window: Some(8 * MIB),
			init_conn_receive_window: Some(20 * MIB),
			max_conn_receive_window: Some(20 * MIB),
			..Default::default()
		};
		let s = quic_settings(&t);
		// Connection window from the conn overrides; stream windows from the stream
		// overrides — independently.
		assert_eq!(s.initial_max_data, 20 * MIB);
		assert_eq!(s.initial_max_stream_data_bidi_local, 8 * MIB);
		assert_eq!(s.initial_max_stream_data_bidi_remote, 8 * MIB);
		assert_eq!(s.initial_max_stream_data_uni, 8 * MIB);
		assert_eq!(s.max_connection_window, 20 * MIB);
		assert_eq!(s.max_stream_window, 8 * MIB);
	}

	#[test]
	fn quic_settings_window_fallback_to_receive_window() {
		// No per-direction override: init windows fall back to `receive_window`;
		// the auto-tuning ceilings stay at quiche's built-in defaults.
		let defaults = QuicSettings::default();
		let t = TransportConfig {
			receive_window: 5 * MIB,
			..Default::default()
		};
		let s = quic_settings(&t);
		assert_eq!(s.initial_max_data, 5 * MIB);
		assert_eq!(s.initial_max_stream_data_bidi_local, 5 * MIB);
		assert_eq!(s.initial_max_stream_data_uni, 5 * MIB);
		assert_eq!(s.max_connection_window, defaults.max_connection_window);
		assert_eq!(s.max_stream_window, defaults.max_stream_window);
	}
}
