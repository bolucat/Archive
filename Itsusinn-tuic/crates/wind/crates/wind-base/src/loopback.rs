//! Loopback detection for outbound connections.
//!
//! Prevents the proxy from connecting to itself by checking whether the
//! resolved destination address is a loopback address or a local listening
//! address.  Mirrors mihomo's `loopback.Detector`.

use std::net::{IpAddr, SocketAddr};

/// Check whether `addr` is a loopback destination.
///
/// Returns `Ok(())` if the address is safe to dial, `Err` with a
/// descriptive message if it looks like a self-connection.
///
/// Currently rejects:
/// - IPv4 loopback (`127.0.0.0/8`)
/// - IPv6 loopback (`::1`)
///
/// Future extensions may cross-reference against the process's own
/// listening sockets (analogous to mihomo's `loopback.Detector` which
/// compares against `net.Listen` addresses).
pub fn check_loopback(addr: &SocketAddr) -> eyre::Result<()> {
	match addr.ip() {
		IpAddr::V4(ip) if ip.is_loopback() => {
			eyre::bail!("loopback connection rejected: {addr}")
		}
		IpAddr::V6(ip) if ip.is_loopback() => {
			eyre::bail!("loopback connection rejected: {addr}")
		}
		_ => Ok(()),
	}
}

#[cfg(test)]
mod tests {
	use std::net::{Ipv4Addr, Ipv6Addr};

	use super::*;

	#[test]
	fn rejects_ipv4_loopback() {
		let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();
		assert!(check_loopback(&addr).is_err());
		// 127.0.0.2 is also loopback
		let addr2: SocketAddr = ([127, 1, 2, 3], 443).into();
		assert!(check_loopback(&addr2).is_err());
	}

	#[test]
	fn rejects_ipv6_loopback() {
		let addr: SocketAddr = (Ipv6Addr::LOCALHOST, 8080).into();
		assert!(check_loopback(&addr).is_err());
	}

	#[test]
	fn allows_public_and_private_ips() {
		for ip_str in ["8.8.8.8", "192.168.1.1", "10.0.0.1", "::1", "2001:db8::1"] {
			let ip: IpAddr = ip_str.parse().unwrap();
			let addr = SocketAddr::new(ip, 80);
			// Only ::1 is loopback in this list
			let expected = ip_str != "::1";
			assert_eq!(check_loopback(&addr).is_ok(), expected, "unexpected result for {ip_str}");
		}
	}
}
