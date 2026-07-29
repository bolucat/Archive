//! Backend-neutral configuration inputs.
//!
//! These types carry the operator's intent without leaking any backend's native
//! configuration (rustls / boring / `quinn::TransportConfig` / `QuicSettings`).
//! Each backend maps them onto its own representation at endpoint-construction
//! time.

use std::time::Duration;

use bytesize::ByteSize;
use serde::{Deserialize, Serialize};
pub use wind_core::quic::QuicCongestionControl;

/// BBR bandwidth-`lo` reduction strategy on a congestion event.
///
/// Backend-neutral mirror of quiche's experimental `BbrBwLoReductionStrategy`;
/// the quiche backend maps it onto the native enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BbrBwLoReductionStrategy {
	/// Default strategy based on `BBRBeta`.
	Default,
	/// Use min-rtt to estimate bandwidth reduction.
	MinRtt,
	/// Use inflight data to estimate bandwidth reduction.
	Inflight,
	/// Use cwnd to estimate bandwidth reduction.
	Cwnd,
}

/// Custom BBR (`bbr2_gcongestion`) tuning, the full surface of quiche's
/// experimental `BbrParams`.
///
/// Every field is optional; `None` leaves quiche's built-in default in place.
/// These knobs are *experimental* (the upstream type is `#[doc(hidden)]` and
/// may be removed) and only take effect on the quiche backend with the BBR
/// congestion controller. Lives in this backend-neutral layer because it must
/// travel through [`TransportConfig`] to reach the quiche backend — the only
/// backend that consumes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct Bbr2gcConfig {
	/// BBR startup cwnd gain.
	pub startup_cwnd_gain: Option<f32>,
	/// BBR startup pacing gain.
	pub startup_pacing_gain: Option<f32>,
	/// BBR full-bandwidth threshold.
	pub full_bw_threshold: Option<f32>,
	/// Rounds to stay in STARTUP before exiting on a bandwidth plateau.
	pub startup_full_bw_rounds: Option<usize>,
	/// Loss count needed to exit STARTUP.
	pub startup_full_loss_count: Option<usize>,
	/// BBR drain cwnd gain.
	pub drain_cwnd_gain: Option<f32>,
	/// BBR drain pacing gain.
	pub drain_pacing_gain: Option<f32>,
	/// Respect Reno coexistence.
	pub enable_reno_coexistence: Option<bool>,
	/// Avoid overestimating bandwidth on ack compression.
	pub enable_overestimate_avoidance: Option<bool>,
	/// Enable the `a0` point fix in the bandwidth sampler.
	pub choose_a0_point_fix: Option<bool>,
	/// PROBE_BW up-phase pacing gain.
	pub probe_bw_probe_up_pacing_gain: Option<f32>,
	/// PROBE_BW down-phase pacing gain.
	pub probe_bw_probe_down_pacing_gain: Option<f32>,
	/// PROBE_BW DOWN/CRUISE/REFILL cwnd gain.
	pub probe_bw_cwnd_gain: Option<f32>,
	/// PROBE_BW UP cwnd gain.
	pub probe_bw_up_cwnd_gain: Option<f32>,
	/// PROBE_RTT pacing gain.
	pub probe_rtt_pacing_gain: Option<f32>,
	/// PROBE_RTT cwnd gain.
	pub probe_rtt_cwnd_gain: Option<f32>,
	/// Rounds to stay in PROBE_BW up if bytes-in-flight doesn't drop below
	/// target.
	pub max_probe_up_queue_rounds: Option<usize>,
	/// BBR loss threshold.
	pub loss_threshold: Option<f32>,
	/// Use bytes-delivered as the estimate for `inflight_hi`.
	pub use_bytes_delivered_for_inflight_hi: Option<bool>,
	/// Decrease startup pacing at round end.
	pub decrease_startup_pacing_at_end_of_round: Option<bool>,
	/// Bandwidth-`lo` reduction strategy.
	pub bw_lo_reduction_strategy: Option<BbrBwLoReductionStrategy>,
	/// Count app-limited rounds with no bandwidth growth toward the
	/// exit-startup rounds threshold.
	pub ignore_app_limited_for_no_bandwidth_growth: Option<bool>,
	/// Initial pacing rate before an RTT estimate is available. Accepts a
	/// human-readable size like `"1.25 MB"` (interpreted as bytes/sec).
	pub initial_pacing_rate_bytes_per_second: Option<ByteSize>,
	/// Scale the pacing rate when the MSS changes during PMTUD.
	pub scale_pacing_rate_by_mss: Option<bool>,
	/// Disable the `has_stayed_long_enough_in_probe_down` early exit.
	pub disable_probe_down_early_exit: Option<bool>,
	/// Set the expected packet send time to `now` instead of the computed
	/// next-release time.
	pub time_sent_set_to_now: Option<bool>,
}

/// Backend-neutral congestion-control tuning carried by [`TransportConfig`],
/// alongside the algorithm selector ([`TransportConfig::congestion`]).
///
/// Each backend applies the subset it understands. Today only the quiche
/// backend consumes these; the quinn backend reads
/// [`TransportConfig::initial_window`] and ignores the rest.
#[derive(Clone, Debug, Default)]
pub struct CongestionTuning {
	/// Initial congestion window, in packets (quiche). `None` = backend
	/// default.
	pub initial_cwnd_packets: Option<usize>,
	/// Enable pacing of outgoing packets (quiche).
	pub pacing: Option<bool>,
	/// Maximum pacing rate, in bytes/sec (quiche). `None` = unlimited.
	pub max_pacing_rate: Option<u64>,
	/// Enable HyStart++ — only with cubic/reno (quiche).
	pub hystart: Option<bool>,
	/// Enable the CUBIC idle-restart fix — only with cubic (quiche).
	pub cubic_idle_restart_fix: Option<bool>,
	/// Custom BBR parameters — only with the BBR controller (quiche).
	pub bbr: Option<Bbr2gcConfig>,
}

/// QUIC transport tuning, shared by both backends.
#[derive(Clone, Debug)]
pub struct TransportConfig {
	/// Maximum number of concurrent peer-initiated bidirectional streams.
	pub max_concurrent_bidi_streams: u64,
	/// Maximum number of concurrent peer-initiated unidirectional streams.
	pub max_concurrent_uni_streams: u64,
	/// Connection-level send window, in bytes.
	pub send_window: u64,
	/// Connection / per-stream receive window, in bytes. Legacy single knob;
	/// the per-direction overrides below take precedence when set.
	pub receive_window: u64,
	/// Per-stream initial receive window (quiche `initial_max_stream_data_*`).
	/// `None` falls back to [`receive_window`](Self::receive_window). The quinn
	/// backend has no separate "initial" window and ignores this.
	pub init_stream_receive_window: Option<u64>,
	/// Per-stream maximum receive window — quiche's flow-control auto-tuning
	/// ceiling (`max_stream_window`); quinn's fixed `stream_receive_window`.
	/// `None` leaves the backend default (quiche) / falls back to
	/// [`receive_window`](Self::receive_window) (quinn).
	pub max_stream_receive_window: Option<u64>,
	/// Connection initial receive window (quiche `initial_max_data`). `None`
	/// falls back to [`receive_window`](Self::receive_window). The quinn
	/// backend has no separate "initial" window and ignores this.
	pub init_conn_receive_window: Option<u64>,
	/// Connection maximum receive window — quiche's flow-control auto-tuning
	/// ceiling (`max_connection_window`); quinn's fixed connection
	/// `receive_window`. `None` leaves the backend default.
	pub max_conn_receive_window: Option<u64>,
	/// Idle timeout. `None` disables it.
	pub max_idle_timeout: Option<Duration>,
	/// Initial MTU guess.
	pub initial_mtu: u16,
	/// Lower bound on the MTU.
	pub min_mtu: u16,
	/// Enable generic segmentation offload (quinn only; ignored by quiche).
	pub gso: bool,
	/// Congestion-control algorithm.
	pub congestion: QuicCongestionControl,
	/// Initial congestion window in bytes. `None` uses the backend default.
	/// (quinn; the quiche backend uses
	/// [`CongestionTuning::initial_cwnd_packets`]).
	pub initial_window: Option<u64>,
	/// Per-algorithm congestion-control tuning. Each backend applies the subset
	/// it supports.
	pub cc: CongestionTuning,
	/// Advertise QUIC DATAGRAM (RFC 9221) support.
	pub enable_datagram: bool,
	/// Allow 0-RTT early data (resumption).
	pub enable_0rtt: bool,
	/// ALPN protocols to advertise, most-preferred first.
	pub alpn: Vec<Vec<u8>>,
}

impl Default for TransportConfig {
	fn default() -> Self {
		Self {
			max_concurrent_bidi_streams: 100,
			max_concurrent_uni_streams: 100,
			send_window: 8 * 1024 * 1024,
			receive_window: 8 * 1024 * 1024,
			init_stream_receive_window: None,
			max_stream_receive_window: None,
			init_conn_receive_window: None,
			max_conn_receive_window: None,
			max_idle_timeout: Some(Duration::from_secs(30)),
			initial_mtu: 1200,
			min_mtu: 1200,
			gso: false,
			congestion: QuicCongestionControl::default(),
			initial_window: None,
			cc: CongestionTuning::default(),
			enable_datagram: true,
			enable_0rtt: false,
			alpn: vec![b"h3".to_vec()],
		}
	}
}

/// Where a server's certificate + private key come from.
#[derive(Clone, Debug)]
pub enum CertSource {
	/// Paths to PEM files on disk. The only form the quiche backend accepts
	/// (tokio-quiche loads credentials from file paths).
	PemPaths { cert: String, key: String },
	/// In-memory PEM bytes. Supported by the quinn backend only.
	PemBytes { cert: Vec<u8>, key: Vec<u8> },
}

/// Server-side TLS configuration.
#[derive(Clone, Debug)]
pub struct ServerTlsConfig {
	/// The certificate chain + key to present.
	pub cert: CertSource,
}

impl ServerTlsConfig {
	/// Convenience constructor for PEM file paths.
	pub fn from_pem_paths(cert: impl Into<String>, key: impl Into<String>) -> Self {
		Self {
			cert: CertSource::PemPaths {
				cert: cert.into(),
				key: key.into(),
			},
		}
	}
}

/// Client-side TLS configuration.
#[derive(Clone, Debug)]
pub struct ClientTlsConfig {
	/// Server name (SNI) to send and verify against.
	pub server_name: String,
	/// Verify the server certificate. Disabling it is insecure (MITM-able) and
	/// intended only for tests / explicitly-trusted setups.
	pub verify_certificate: bool,
	/// ALPN protocols to advertise, most-preferred first.
	pub alpn: Vec<Vec<u8>>,
}

impl ClientTlsConfig {
	/// A verifying client config for `server_name` advertising the `h3` ALPN.
	pub fn new(server_name: impl Into<String>) -> Self {
		Self {
			server_name: server_name.into(),
			verify_certificate: true,
			alpn: vec![b"h3".to_vec()],
		}
	}
}
