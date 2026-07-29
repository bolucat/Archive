//! Provider-agnostic QUIC configuration types.

/// Congestion-control algorithm for QUIC connections.
///
/// This is a transport-agnostic selector: it carries the user's intent without
/// depending on any particular QUIC backend. Outbounds map it onto whatever
/// representation their underlying engine expects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum QuicCongestionControl {
	#[default]
	Default,
	Bbr,
	BbrV2,
	Cubic,
	Reno,
}

/// Parse a QUIC congestion-control algorithm name (case-insensitive).
///
/// Accepts `default` (or empty), `bbr`, `bbrv2`/`bbr2`, `cubic`, `reno`.
/// Exposed so configuration front-ends can map a string field onto the
/// [`QuicCongestionControl`] enum without depending on a concrete QUIC backend.
pub fn parse_congestion_control(s: &str) -> eyre::Result<QuicCongestionControl> {
	Ok(match s.trim().to_ascii_lowercase().as_str() {
		"" | "default" => QuicCongestionControl::Default,
		"bbr" => QuicCongestionControl::Bbr,
		"bbr2" | "bbrv2" => QuicCongestionControl::BbrV2,
		"cubic" => QuicCongestionControl::Cubic,
		"reno" => QuicCongestionControl::Reno,
		other => eyre::bail!("unknown quic_congestion_control {other:?} (expected default|bbr|bbrv2|cubic|reno)"),
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parses_known_names_case_insensitively() {
		assert_eq!(parse_congestion_control("").unwrap(), QuicCongestionControl::Default);
		assert_eq!(parse_congestion_control("default").unwrap(), QuicCongestionControl::Default);
		assert_eq!(parse_congestion_control("BBR").unwrap(), QuicCongestionControl::Bbr);
		assert_eq!(parse_congestion_control("bbr2").unwrap(), QuicCongestionControl::BbrV2);
		assert_eq!(parse_congestion_control(" BbrV2 ").unwrap(), QuicCongestionControl::BbrV2);
		assert_eq!(parse_congestion_control("cubic").unwrap(), QuicCongestionControl::Cubic);
		assert_eq!(parse_congestion_control("reno").unwrap(), QuicCongestionControl::Reno);
	}

	#[test]
	fn rejects_unknown_names() {
		assert!(parse_congestion_control("bogus").is_err());
	}
}
