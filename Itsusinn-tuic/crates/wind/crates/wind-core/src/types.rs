use std::{
	fmt::Display,
	net::{Ipv4Addr, Ipv6Addr, SocketAddr},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TargetAddr {
	Domain(String, u16),
	IPv4(Ipv4Addr, u16),
	IPv6(Ipv6Addr, u16),
}

impl From<SocketAddr> for TargetAddr {
	fn from(addr: SocketAddr) -> Self {
		match addr {
			SocketAddr::V4(addr) => TargetAddr::IPv4(*addr.ip(), addr.port()),
			SocketAddr::V6(addr) => TargetAddr::IPv6(*addr.ip(), addr.port()),
		}
	}
}

impl Display for TargetAddr {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			TargetAddr::Domain(domain, port) => write!(f, "{}:{}", domain, port),
			TargetAddr::IPv4(addr, port) => write!(f, "{}:{}", addr, port),
			TargetAddr::IPv6(addr, port) => write!(f, "[{}]:{}", addr, port),
		}
	}
}

impl Serialize for TargetAddr {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.to_string().serialize(serializer)
	}
}

impl<'de> Deserialize<'de> for TargetAddr {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		use serde::de::Error;

		let s = String::deserialize(deserializer)?;

		if s.starts_with('[') {
			let end_bracket = match s.find(']') {
				Some(pos) => pos,
				None => {
					return Err(Error::custom("Invalid IPv6 address format, missing closing bracket"));
				}
			};

			if end_bracket + 1 >= s.len() || !s[end_bracket + 1..].starts_with(':') {
				return Err(Error::custom("Invalid IPv6 address format, expected [IPv6]:port"));
			}

			let ipv6_str = &s[1..end_bracket];
			let port_str = &s[end_bracket + 2..];

			let ipv6_addr = match ipv6_str.parse::<Ipv6Addr>() {
				Ok(addr) => addr,
				Err(_) => return Err(Error::custom("Invalid IPv6 address")),
			};

			let port = match port_str.parse::<u16>() {
				Ok(p) => p,
				Err(_) => return Err(Error::custom("Invalid port number")),
			};

			return Ok(TargetAddr::IPv6(ipv6_addr, port));
		}

		// `rsplit_once` matches the LAST ':' so multi-colon inputs (which the
		// IPv6 branch above didn't catch) fail cleanly instead of silently
		// taking only the first segment.
		let (host, port_str) = s
			.rsplit_once(':')
			.ok_or_else(|| Error::custom("Invalid address format, expected host:port"))?;

		// Reject empty hosts. Previously `:80` parsed to `Domain("", 80)`
		// which then failed at DNS time with a confusing error.
		if host.is_empty() {
			return Err(Error::custom("Invalid address: host part is empty"));
		}

		let port = port_str.parse::<u16>().map_err(|_| Error::custom("Invalid port number"))?;

		if let Ok(ipv4) = host.parse::<Ipv4Addr>() {
			Ok(TargetAddr::IPv4(ipv4, port))
		} else {
			// Otherwise treat as a domain. Apply a minimal sanity check: no
			// whitespace, no embedded brackets/colons (those would indicate
			// a malformed IPv6 literal that escaped the earlier branch).
			if host.chars().any(|c| c.is_whitespace() || c == '[' || c == ']' || c == ':') {
				return Err(Error::custom(format!("Invalid domain literal: {host:?}")));
			}
			Ok(TargetAddr::Domain(host.to_string(), port))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_display_ipv4() {
		let addr = TargetAddr::IPv4("127.0.0.1".parse().unwrap(), 8080);
		assert_eq!(addr.to_string(), "127.0.0.1:8080");
	}

	#[test]
	fn test_display_ipv6() {
		let addr = TargetAddr::IPv6("::1".parse().unwrap(), 8080);
		assert_eq!(addr.to_string(), "[::1]:8080");
	}

	#[test]
	fn test_display_domain() {
		let addr = TargetAddr::Domain("example.com".to_string(), 443);
		assert_eq!(addr.to_string(), "example.com:443");
	}

	#[test]
	fn test_serialize_deserialize_ipv4() {
		let addr = TargetAddr::IPv4("192.168.1.1".parse().unwrap(), 1234);
		let serialized = serde_json::to_string(&addr).unwrap();
		assert_eq!(serialized, "\"192.168.1.1:1234\"");
		let deserialized: TargetAddr = serde_json::from_str(&serialized).unwrap();
		assert_eq!(deserialized, addr);
	}

	#[test]
	fn test_serialize_deserialize_ipv6() {
		let addr = TargetAddr::IPv6("2001:db8::1".parse().unwrap(), 5678);
		let serialized = serde_json::to_string(&addr).unwrap();
		assert_eq!(serialized, "\"[2001:db8::1]:5678\"");
		let deserialized: TargetAddr = serde_json::from_str(&serialized).unwrap();
		assert_eq!(deserialized, addr);
	}

	#[test]
	fn test_serialize_deserialize_domain() {
		let addr = TargetAddr::Domain("test.org".to_string(), 80);
		let serialized = serde_json::to_string(&addr).unwrap();
		assert_eq!(serialized, "\"test.org:80\"");
		let deserialized: TargetAddr = serde_json::from_str(&serialized).unwrap();
		assert_eq!(deserialized, addr);
	}

	#[test]
	fn test_deserialize_invalid_ipv6() {
		let s = "[invalid]:1234";
		let result: Result<TargetAddr, _> = serde_json::from_str(&format!("\"{}\"", s));
		assert!(result.is_err());
	}

	#[test]
	fn test_deserialize_invalid_port() {
		let s = "127.0.0.1:notaport";
		let result: Result<TargetAddr, _> = serde_json::from_str(&format!("\"{}\"", s));
		assert!(result.is_err());
	}

	#[test]
	fn test_deserialize_missing_bracket() {
		let s = "[::1:8080";
		let result: Result<TargetAddr, _> = serde_json::from_str(&format!("\"{}\"", s));
		assert!(result.is_err());
	}

	#[test]
	fn test_deserialize_invalid_format() {
		let s = "justastring";
		let result: Result<TargetAddr, _> = serde_json::from_str(&format!("\"{}\"", s));
		assert!(result.is_err());
	}

	// PR4-I regression tests: empty / malformed host parts must fail
	// deserialization, not silently produce a `Domain("", _)` etc.

	#[test]
	fn deserialize_rejects_empty_host() {
		let result: Result<TargetAddr, _> = serde_json::from_str("\":80\"");
		let err = result.expect_err("`:80` must not parse — empty host");
		assert!(err.to_string().to_lowercase().contains("empty"));
	}

	#[test]
	fn deserialize_rejects_whitespace_in_domain() {
		let result: Result<TargetAddr, _> = serde_json::from_str("\"x y:80\"");
		assert!(result.is_err(), "domain with whitespace must be rejected");
	}

	#[test]
	fn deserialize_uses_last_colon_for_split() {
		// Without an IPv6 bracket form, an embedded `:` is ambiguous. The
		// hardened parser uses `rsplit_once(':')`, so the host part keeps any
		// leading colons; the validation step then catches the malformed
		// IPv6-like literal.
		let result: Result<TargetAddr, _> = serde_json::from_str("\"a:b:80\"");
		assert!(result.is_err(), "ambiguous `a:b:80` must be rejected");
	}
}
