//! Configuration types for the quiche backend (mirrors the former
//! `wind-tuiche` surface).

use std::{
	fmt::{Display, Formatter, Result as FmtResult},
	time::Duration,
};

use wind_quic::{CongestionTuning, QuicCongestionControl};

/// Congestion control algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CongestionControl {
	#[default]
	Cubic,
	Bbr,
	Reno,
}

impl CongestionControl {
	/// Return the effective algorithm name that quiche actually uses.
	/// \`Bbr\` resolves to `Bbr2Gcongestion` internally.
	pub fn effective_name(&self) -> &'static str {
		match self {
			Self::Cubic => "cubic",
			Self::Bbr => "bbr2_gcongestion",
			Self::Reno => "reno",
		}
	}

	/// Parse a config string into the effective quiche algorithm name,
	/// matching the same mapping as the inbound's \`quiche_cc()\`.
	pub fn effective_from_str(s: &str) -> &'static str {
		match s.to_ascii_lowercase().as_str() {
			"bbr" | "bbr3" => Self::Bbr,
			"reno" | "newreno" | "new_reno" => Self::Reno,
			_ => Self::Cubic,
		}
		.effective_name()
	}
}

impl Display for CongestionControl {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		f.write_str(self.effective_name())
	}
}

impl From<CongestionControl> for QuicCongestionControl {
	fn from(cc: CongestionControl) -> Self {
		match cc {
			CongestionControl::Cubic => QuicCongestionControl::Cubic,
			CongestionControl::Bbr => QuicCongestionControl::Bbr,
			CongestionControl::Reno => QuicCongestionControl::Reno,
		}
	}
}

/// UDP relay mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UdpRelayMode {
	#[default]
	Datagram,
	Stream,
}

/// Connection options.
#[derive(Debug, Clone)]
pub struct ConnectionOpts {
	/// Maximum idle timeout.
	pub max_idle_timeout: Duration,
	/// Maximum concurrent bidirectional streams.
	pub max_concurrent_bi_streams: u64,
	/// Maximum concurrent unidirectional streams.
	pub max_concurrent_uni_streams: u64,
	/// Send window size.
	pub send_window: u64,
	/// Receive window size. Legacy single knob; the per-direction overrides
	/// below take precedence when set.
	pub receive_window: u64,
	/// Per-stream initial receive window (bytes). `None` falls back to
	/// [`receive_window`](Self::receive_window).
	pub init_stream_receive_window: Option<u64>,
	/// Per-stream maximum receive window — flow-control auto-tuning ceiling
	/// (bytes). `None` leaves quiche's default.
	pub max_stream_receive_window: Option<u64>,
	/// Connection initial receive window (bytes). `None` falls back to
	/// [`receive_window`](Self::receive_window).
	pub init_conn_receive_window: Option<u64>,
	/// Connection maximum receive window — flow-control auto-tuning ceiling
	/// (bytes). `None` leaves quiche's default.
	pub max_conn_receive_window: Option<u64>,
	/// Congestion control algorithm.
	pub congestion_control: CongestionControl,
	/// Per-algorithm congestion-control tuning (initial window, pacing,
	/// HyStart++, CUBIC idle-restart-fix, custom BBR params).
	pub cc: CongestionTuning,
	/// UDP relay mode.
	pub udp_relay_mode: UdpRelayMode,
	/// Enable 0-RTT.
	pub enable_0rtt: bool,
}

impl Default for ConnectionOpts {
	fn default() -> Self {
		Self {
			max_idle_timeout: Duration::from_secs(30),
			max_concurrent_bi_streams: 100,
			max_concurrent_uni_streams: 100,
			send_window: 8 * 1024 * 1024,    // 8 MB
			receive_window: 8 * 1024 * 1024, // 8 MB
			init_stream_receive_window: None,
			max_stream_receive_window: None,
			init_conn_receive_window: None,
			max_conn_receive_window: None,
			congestion_control: CongestionControl::default(),
			cc: CongestionTuning::default(),
			udp_relay_mode: UdpRelayMode::default(),
			enable_0rtt: true,
		}
	}
}

impl ConnectionOpts {
	/// Translate into a backend-neutral [`wind_quic::TransportConfig`].
	pub(crate) fn to_transport(&self) -> wind_quic::TransportConfig {
		wind_quic::TransportConfig {
			max_concurrent_bidi_streams: self.max_concurrent_bi_streams,
			max_concurrent_uni_streams: self.max_concurrent_uni_streams,
			send_window: self.send_window,
			receive_window: self.receive_window,
			init_stream_receive_window: self.init_stream_receive_window,
			max_stream_receive_window: self.max_stream_receive_window,
			init_conn_receive_window: self.init_conn_receive_window,
			max_conn_receive_window: self.max_conn_receive_window,
			max_idle_timeout: Some(self.max_idle_timeout),
			congestion: self.congestion_control.into(),
			// TUIC's native UDP relay uses QUIC DATAGRAM frames (RFC 9221).
			enable_datagram: matches!(self.udp_relay_mode, UdpRelayMode::Datagram),
			enable_0rtt: self.enable_0rtt,
			alpn: vec![b"h3".to_vec()],
			cc: self.cc.clone(),
			..Default::default()
		}
	}
}
