use std::{
	fmt::{Display, Formatter, Result as FmtResult},
	str::FromStr,
};

use serde::{Deserialize, Serialize};

/// UDP relay mode for TUIC protocol
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UdpRelayMode {
	Native,
	Quic,
}

impl Display for UdpRelayMode {
	fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
		match self {
			Self::Native => write!(f, "native"),
			Self::Quic => write!(f, "quic"),
		}
	}
}

impl FromStr for UdpRelayMode {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.eq_ignore_ascii_case("native") {
			Ok(Self::Native)
		} else if s.eq_ignore_ascii_case("quic") {
			Ok(Self::Quic)
		} else {
			Err("invalid UDP relay mode")
		}
	}
}

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
