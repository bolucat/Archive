use std::{
	fmt::{Display, Formatter, Result as FmtResult},
	str::FromStr,
	sync::Arc,
};

use serde::{Deserialize, Serialize};

/// UDP relay mode for TUIC protocol.
///
/// The enum itself lives in the backend-neutral [`crate::proto`] module so the
/// shared client `UdpStream` can use it; re-exported here to preserve the
/// historical `quinn::utils::UdpRelayMode` path.
pub use crate::proto::UdpRelayMode;

/// Congestion control algorithm for QUIC
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CongestionControl {
	#[default]
	Bbr,
	Bbr3,
	Cubic,
	NewReno,
}

impl CongestionControl {
	/// Return the effective algorithm name, as quinn will use it.
	pub fn effective_name(&self) -> &'static str {
		match self {
			Self::Bbr => "bbr",
			Self::Bbr3 => "bbr3",
			Self::Cubic => "cubic",
			Self::NewReno => "new_reno",
		}
	}

	/// Parse a config string and return the effective algorithm name,
	/// applying the same fallback (`Bbr`) that the inbound uses.
	pub fn effective_from_str(s: &str) -> &'static str {
		s.parse().unwrap_or(Self::Bbr).effective_name()
	}
}

impl Display for CongestionControl {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		f.write_str(self.effective_name())
	}
}

impl FromStr for CongestionControl {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.eq_ignore_ascii_case("cubic") {
			Ok(Self::Cubic)
		} else if s.eq_ignore_ascii_case("new_reno") || s.eq_ignore_ascii_case("newreno") {
			Ok(Self::NewReno)
		} else if s.eq_ignore_ascii_case("bbr") {
			Ok(Self::Bbr)
		} else if s.eq_ignore_ascii_case("bbr3") {
			Ok(Self::Bbr3)
		} else {
			Err("invalid congestion control")
		}
	}
}

/// Build the quinn congestion-controller factory for `cc`.
///
/// `quinn-congestions` provides a single BBR implementation; both the `bbr`
/// and `bbr3` config aliases map to it.
pub fn congestion_controller_factory(
	cc: CongestionControl,
) -> Arc<dyn quinn::congestion::ControllerFactory + Send + Sync + 'static> {
	match cc {
		CongestionControl::Bbr | CongestionControl::Bbr3 => Arc::new(quinn_congestions::bbr::BbrConfig::default()),
		CongestionControl::Cubic => Arc::new(quinn::congestion::CubicConfig::default()),
		CongestionControl::NewReno => Arc::new(quinn::congestion::NewRenoConfig::default()),
	}
}
