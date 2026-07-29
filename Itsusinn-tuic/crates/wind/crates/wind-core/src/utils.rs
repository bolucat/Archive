use std::{net::IpAddr, str::FromStr};

use serde::{Deserialize, Serialize};

/// IP stack preference for address resolution.
///
/// Determines which IP version to prefer when resolving domain names.
///
/// # Variants
///
/// - `V4only`: Use only IPv4 addresses (alias: "v4", "only_v4")
/// - `V6only`: Use only IPv6 addresses (alias: "v6", "only_v6")
/// - `V4first`: Prefer IPv4, fallback to IPv6 (alias: "v4v6", "prefer_v4")
/// - `V6first`: Prefer IPv6, fallback to IPv4 (alias: "v6v4", "prefer_v6")
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StackPrefer {
	/// Use only IPv4 addresses
	#[serde(alias = "v4", alias = "only_v4")]
	#[default]
	V4only,
	/// Use only IPv6 addresses
	#[serde(alias = "v6", alias = "only_v6")]
	V6only,
	/// Prefer IPv4, fallback to IPv6
	#[serde(alias = "v4v6", alias = "prefer_v4", alias = "auto")]
	V4first,
	/// Prefer IPv6, fallback to IPv4
	#[serde(alias = "v6v4", alias = "prefer_v6")]
	V6first,
}

impl FromStr for StackPrefer {
	type Err = &'static str;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_ascii_lowercase().as_str() {
			"v4" | "v4only" | "only_v4" => Ok(StackPrefer::V4only),
			"v6" | "v6only" | "only_v6" => Ok(StackPrefer::V6only),
			"v4v6" | "v4first" | "prefer_v4" | "auto" => Ok(StackPrefer::V4first),
			"v6v4" | "v6first" | "prefer_v6" => Ok(StackPrefer::V6first),
			_ => Err("invalid stack preference"),
		}
	}
}

/// Check if an IP address is private (LAN address)
///
/// Returns `true` for:
/// - IPv4: 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 169.254.0.0/16
///   (Link-local), 100.64.0.0/10 (CGNAT)
/// - IPv6: fc00::/7 (Unique Local Address), fe80::/10 (Link-local)
///
/// IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d`) are canonicalized to their
/// IPv4 form before the check, so `::ffff:10.0.0.1` is treated as private.
/// Without this, an attacker could bypass a `drop_private` guard by writing an
/// internal target in mapped form.
#[inline]
pub fn is_private_ip(ip: &IpAddr) -> bool {
	match ip.to_canonical() {
		IpAddr::V4(ipv4) => {
			// 10.0.0.0/8
			ipv4.octets()[0] == 10
				// 172.16.0.0/12
				|| (ipv4.octets()[0] == 172 && (ipv4.octets()[1] >= 16 && ipv4.octets()[1] <= 31))
				// 192.168.0.0/16
				|| (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168)
				// 169.254.0.0/16 (Link-local)
				|| (ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254)
				// 100.64.0.0/10 (CGNAT, RFC 6598)
				|| (ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xc0) == 0x40)
		}
		IpAddr::V6(ipv6) => {
			// fc00::/7 (Unique Local Address)
			ipv6.octets()[0] & 0xfe == 0xfc
				// fe80::/10 (Link-local)
				|| (ipv6.octets()[0] == 0xfe && (ipv6.octets()[1] & 0xc0) == 0x80)
		}
	}
}

#[cfg(test)]
mod tests {
	use std::net::IpAddr;

	use super::*;

	fn ip(s: &str) -> IpAddr {
		s.parse().unwrap()
	}

	#[test]
	fn private_ipv4_ranges_are_private() {
		for s in [
			"10.0.0.1",
			"10.255.255.255",
			"172.16.0.0",
			"172.31.255.255",
			"192.168.0.1",
			"192.168.255.255",
			"169.254.0.1",
		] {
			assert!(is_private_ip(&ip(s)), "{s} should be private");
		}
	}

	#[test]
	fn public_ipv4_and_boundary_ranges_are_public() {
		for s in [
			"8.8.8.8",
			"1.1.1.1",
			"11.0.0.1",
			"172.15.255.255", // just below the 172.16/12 block
			"172.32.0.0",     // just above it
			"192.167.255.255",
			"169.253.0.1",
		] {
			assert!(!is_private_ip(&ip(s)), "{s} should be public");
		}
	}

	#[test]
	fn private_ipv6_ranges_are_private() {
		// fc00::/7 (fc.. and fd..) and fe80::/10 (fe80.. through febf..).
		for s in ["fc00::1", "fd00::1", "fe80::1", "febf::1"] {
			assert!(is_private_ip(&ip(s)), "{s} should be private");
		}
	}

	#[test]
	fn public_ipv6_ranges_are_public() {
		// 2001:db8 doc range, loopback, and fec0 (outside fe80::/10).
		for s in ["2001:db8::1", "::1", "fec0::1"] {
			assert!(!is_private_ip(&ip(s)), "{s} should be public");
		}
	}

	#[test]
	fn cgnat_range_is_private() {
		// 100.64.0.0/10 (RFC 6598).
		for s in ["100.64.0.1", "100.127.255.255"] {
			assert!(is_private_ip(&ip(s)), "{s} should be private");
		}
		// Just outside the /10 block.
		for s in ["100.63.255.255", "100.128.0.0"] {
			assert!(!is_private_ip(&ip(s)), "{s} should be public");
		}
	}

	#[test]
	fn ipv4_mapped_ipv6_is_canonicalized_before_check() {
		// A private IPv4 target written in IPv4-mapped IPv6 form must still be
		// classified as private, otherwise `drop_private` guards are bypassable.
		for s in ["::ffff:10.0.0.1", "::ffff:192.168.1.1", "::ffff:169.254.0.1"] {
			assert!(is_private_ip(&ip(s)), "{s} should be private");
		}
		// Mapped public addresses stay public.
		assert!(!is_private_ip(&ip("::ffff:8.8.8.8")), "mapped 8.8.8.8 should be public");
	}

	#[test]
	fn stack_prefer_parses_all_aliases_case_insensitively() {
		for s in ["v4", "v4only", "only_v4", "V4ONLY"] {
			assert_eq!(s.parse::<StackPrefer>(), Ok(StackPrefer::V4only), "{s}");
		}
		for s in ["v6", "v6only", "only_v6"] {
			assert_eq!(s.parse::<StackPrefer>(), Ok(StackPrefer::V6only), "{s}");
		}
		for s in ["v4v6", "v4first", "prefer_v4", "auto"] {
			assert_eq!(s.parse::<StackPrefer>(), Ok(StackPrefer::V4first), "{s}");
		}
		for s in ["v6v4", "v6first", "prefer_v6"] {
			assert_eq!(s.parse::<StackPrefer>(), Ok(StackPrefer::V6first), "{s}");
		}
	}

	#[test]
	fn stack_prefer_rejects_unknown() {
		assert!("nonsense".parse::<StackPrefer>().is_err());
		assert!("".parse::<StackPrefer>().is_err());
	}
}
