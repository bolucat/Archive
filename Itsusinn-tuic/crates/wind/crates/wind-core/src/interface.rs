use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Iface {
	pub name: String,
	pub ipv4: Option<Ipv4Addr>,
	pub ipv6: Option<Ipv6Addr>,
	pub index: u32,
}

#[derive(Debug, Clone, Default)]
pub enum Stack {
	#[default]
	V4,
	V6,
}
impl From<&SocketAddr> for Stack {
	fn from(value: &SocketAddr) -> Self {
		match value {
			SocketAddr::V4(..) => Self::V4,
			SocketAddr::V6(..) => Self::V6,
		}
	}
}
impl From<&IpAddr> for Stack {
	fn from(value: &IpAddr) -> Self {
		match value {
			IpAddr::V4(..) => Self::V4,
			IpAddr::V6(..) => Self::V6,
		}
	}
}

/// Network protocol of a flow.
///
/// Both derives are present. This type is re-exported from the crate root for
/// interface and flow metadata that callers persist or ship to another process,
/// so a `Serialize`-only derive would make that surface write-only: it could
/// emit `"TCP"` but never read it back. Nothing in this workspace encodes a
/// `Network` today, so the asymmetry had no live failure; the derive only adds
/// an implementation on a `Copy`-able fieldless enum, with no custom
/// `Deserialize` to conflict with and no behavior to change.
///
/// The variant names *are* the wire form, so they are pinned by the tests
/// below: renaming a variant, or adding `#[serde(rename)]`, would silently
/// change already-emitted payloads. Note that this is unrelated to
/// [`wind_rule::NetworkType`], which spells the same idea `"tcp"`/`"udp"` in
/// ACL syntax and is not interchangeable with it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum Network {
	TCP,
	UDP,
	ICMPv4,
	ICMPv6,
}

// `StackPrefer` used to be defined here as well, parallel to the one in
// `utils.rs`. The two had different serde aliases (`V4`/`V6`/`V4V6`/`V6V4`
// vs `V4only`/`V6only`/`V4first`/`V6first`), and `lib.rs` did
// `pub use utils::StackPrefer` AFTER this module's exports, so external
// callers got the utils-version while this in-crate one stayed in the
// namespace and silently shadowed it for some internal uses. The
// duplicate was deleted; the single canonical type now lives in `utils`.

#[cfg(test)]
mod tests {
	use super::*;

	/// The exact `Network` wire strings; asserted literally so a variant rename
	/// or an added `#[serde(rename)]` cannot pass unnoticed.
	const WIRE_FORMS: [(&str, Network); 4] = [
		("TCP", Network::TCP),
		("UDP", Network::UDP),
		("ICMPv4", Network::ICMPv4),
		("ICMPv6", Network::ICMPv6),
	];

	#[test]
	fn network_serializes_to_its_variant_name() {
		for (wire, network) in WIRE_FORMS {
			assert_eq!(serde_json::to_string(&network).unwrap(), format!("\"{wire}\""));
		}
	}

	#[test]
	fn every_network_round_trips_through_serde() {
		for (wire, network) in WIRE_FORMS {
			let encoded = serde_json::to_string(&network).unwrap();
			let decoded: Network = serde_json::from_str(&encoded).unwrap();
			assert!(same_variant(decoded, network), "{wire} decoded to a different variant");
			// Re-encoding the decoded value must reproduce the exact
			// original payload.
			assert_eq!(serde_json::to_string(&decoded).unwrap(), encoded);
		}
	}

	#[test]
	fn unknown_network_names_are_rejected() {
		// A silently-defaulting decode would turn a typo into a
		// valid-looking flow, so unknown variants must stay an error.
		for bad in ["\"tcp\"", "\"SCTP\"", "\"\"", "1", "null"] {
			assert!(serde_json::from_str::<Network>(bad).is_err(), "{bad} should not decode");
		}
	}

	fn same_variant(lhs: Network, rhs: Network) -> bool {
		matches!(
			(lhs, rhs),
			(Network::TCP, Network::TCP)
				| (Network::UDP, Network::UDP)
				| (Network::ICMPv4, Network::ICMPv4)
				| (Network::ICMPv6, Network::ICMPv6)
		)
	}
}
