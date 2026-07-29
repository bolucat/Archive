//! UDP-over-TCP (UoT), protocol **version 2**.
//!
//! NaiveProxy's classic protocol only tunnels TCP (an HTTP `CONNECT` byte
//! stream). To carry UDP datagrams through it we layer sing-box's
//! "UDP over TCP" v2 framing on top of a single CONNECT tunnel: the tunnel is
//! opened to a *magic* authority ([`MAGIC_ADDRESS`]) that tells a compatible
//! server to treat the stream as a multiplexed UDP relay rather than a literal
//! TCP connection.
//!
//! Wire format (matches `github.com/sagernet/sing/common/uot`, version 2):
//!
//! ```text
//! request header (client → server, once):
//!     [1]   is_connect          (0x00 = false: per-packet addressing)
//!     [..]  destination addr    (SOCKS5 address, see below)
//!
//! packet (both directions, repeated), non-connect mode:
//!     [..]  address             (SOCKS5 address: remote peer)
//!     [2]   length              (u16, big-endian, = payload length)
//!     [..]  payload
//! ```
//!
//! SOCKS5 address encoding (same `ATYP` codepoints as RFC 1928):
//!
//! ```text
//!     [1]   ATYP   0x01 IPv4 | 0x03 domain | 0x04 IPv6
//!     IPv4:   [4] addr            + [2] port (big-endian)
//!     IPv6:   [16] addr           + [2] port (big-endian)
//!     domain: [1] len + [len] str + [2] port (big-endian)
//! ```
//!
//! We always use **non-connect** mode (`is_connect = false`) so a single tunnel
//! can fan out to many destinations: every packet carries its own address.

use std::io::{self, Read};

use wind_core::types::TargetAddr;

/// Magic CONNECT authority that signals UoT **v2** to a compatible server.
pub const MAGIC_ADDRESS: &str = "sp.v2.udp-over-tcp.arpa";

/// Protocol version implemented by this module.
pub const VERSION: u8 = 2;

// SOCKS5 `ATYP` codepoints.
const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

/// Append a SOCKS5-encoded address (type + addr + 2-byte big-endian port).
///
/// Returns an error for domains longer than 255 bytes, which cannot be encoded
/// in the single length octet.
fn write_addr(buf: &mut Vec<u8>, addr: &TargetAddr) -> io::Result<()> {
	match addr {
		TargetAddr::IPv4(ip, port) => {
			buf.push(ATYP_IPV4);
			buf.extend_from_slice(&ip.octets());
			buf.extend_from_slice(&port.to_be_bytes());
		}
		TargetAddr::IPv6(ip, port) => {
			buf.push(ATYP_IPV6);
			buf.extend_from_slice(&ip.octets());
			buf.extend_from_slice(&port.to_be_bytes());
		}
		TargetAddr::Domain(host, port) => {
			let bytes = host.as_bytes();
			let len: u8 = bytes
				.len()
				.try_into()
				.map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "UoT domain longer than 255 bytes"))?;
			buf.push(ATYP_DOMAIN);
			buf.push(len);
			buf.extend_from_slice(bytes);
			buf.extend_from_slice(&port.to_be_bytes());
		}
	}
	Ok(())
}

/// Read a SOCKS5-encoded address from `r`.
fn read_addr<R: Read>(r: &mut R) -> io::Result<TargetAddr> {
	let mut atyp = [0u8; 1];
	r.read_exact(&mut atyp)?;
	match atyp[0] {
		ATYP_IPV4 => {
			let mut b = [0u8; 6];
			r.read_exact(&mut b)?;
			let ip = std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3]);
			let port = u16::from_be_bytes([b[4], b[5]]);
			Ok(TargetAddr::IPv4(ip, port))
		}
		ATYP_IPV6 => {
			let mut b = [0u8; 18];
			r.read_exact(&mut b)?;
			let mut ip = [0u8; 16];
			ip.copy_from_slice(&b[..16]);
			let port = u16::from_be_bytes([b[16], b[17]]);
			Ok(TargetAddr::IPv6(std::net::Ipv6Addr::from(ip), port))
		}
		ATYP_DOMAIN => {
			let mut len = [0u8; 1];
			r.read_exact(&mut len)?;
			let len = len[0] as usize;
			let mut buf = vec![0u8; len + 2];
			r.read_exact(&mut buf)?;
			let host = String::from_utf8(buf[..len].to_vec())
				.map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "UoT domain is not valid UTF-8"))?;
			let port = u16::from_be_bytes([buf[len], buf[len + 1]]);
			Ok(TargetAddr::Domain(host, port))
		}
		other => Err(io::Error::new(
			io::ErrorKind::InvalidData,
			format!("UoT unknown address type {other:#04x}"),
		)),
	}
}

/// Encode the one-shot request header (`is_connect = false` + `destination`).
///
/// In non-connect mode the destination is informational only — the server
/// routes each packet by its own per-packet address — but it must still be a
/// well-formed address, so we pass the first datagram's target.
pub fn encode_request(destination: &TargetAddr) -> io::Result<Vec<u8>> {
	let mut buf = Vec::with_capacity(1 + 1 + 16 + 2);
	buf.push(0u8); // is_connect = false
	write_addr(&mut buf, destination)?;
	Ok(buf)
}

/// Encode a single packet frame: `address || u16 length || payload`.
///
/// Appends to `buf` so callers can coalesce the request header and the first
/// packet into one write.
pub fn encode_packet_into(buf: &mut Vec<u8>, destination: &TargetAddr, payload: &[u8]) -> io::Result<()> {
	let len: u16 = payload
		.len()
		.try_into()
		.map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "UoT payload exceeds 65535 bytes"))?;
	write_addr(buf, destination)?;
	buf.extend_from_slice(&len.to_be_bytes());
	buf.extend_from_slice(payload);
	Ok(())
}

/// Encode a single packet frame into a fresh buffer.
pub fn encode_packet(destination: &TargetAddr, payload: &[u8]) -> io::Result<Vec<u8>> {
	let mut buf = Vec::with_capacity(19 + payload.len());
	encode_packet_into(&mut buf, destination, payload)?;
	Ok(buf)
}

/// Read a single packet frame: returns the remote peer address and payload.
///
/// A clean end-of-stream surfaces as [`io::ErrorKind::UnexpectedEof`].
pub fn read_packet<R: Read>(r: &mut R) -> io::Result<(TargetAddr, Vec<u8>)> {
	let addr = read_addr(r)?;
	let mut len = [0u8; 2];
	r.read_exact(&mut len)?;
	let len = u16::from_be_bytes(len) as usize;
	let mut payload = vec![0u8; len];
	r.read_exact(&mut payload)?;
	Ok((addr, payload))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn roundtrip(addr: TargetAddr, payload: &[u8]) {
		let frame = encode_packet(&addr, payload).unwrap();
		let mut cursor = io::Cursor::new(frame);
		let (got_addr, got_payload) = read_packet(&mut cursor).unwrap();
		assert_eq!(got_addr, addr);
		assert_eq!(got_payload, payload);
		// The cursor must be fully consumed — no trailing bytes.
		assert_eq!(cursor.position() as usize, cursor.get_ref().len());
	}

	#[test]
	fn packet_roundtrip_ipv4() {
		roundtrip(TargetAddr::IPv4("8.8.8.8".parse().unwrap(), 53), b"\x00\x01hello");
	}

	#[test]
	fn packet_roundtrip_ipv6() {
		roundtrip(TargetAddr::IPv6("2001:db8::1".parse().unwrap(), 443), b"quic");
	}

	#[test]
	fn packet_roundtrip_domain() {
		roundtrip(TargetAddr::Domain("dns.example.com".into(), 853), b"dot");
	}

	#[test]
	fn packet_roundtrip_empty_payload() {
		roundtrip(TargetAddr::IPv4("1.1.1.1".parse().unwrap(), 53), b"");
	}

	#[test]
	fn request_header_is_non_connect() {
		let req = encode_request(&TargetAddr::IPv4("8.8.8.8".parse().unwrap(), 53)).unwrap();
		assert_eq!(req[0], 0, "is_connect must be false");
		assert_eq!(req[1], ATYP_IPV4);
		assert_eq!(&req[2..6], &[8, 8, 8, 8]);
		assert_eq!(u16::from_be_bytes([req[6], req[7]]), 53);
	}

	#[test]
	fn two_packets_stream_back_to_back() {
		let a = TargetAddr::IPv4("9.9.9.9".parse().unwrap(), 53);
		let b = TargetAddr::Domain("example.org".into(), 80);
		let mut stream = encode_packet(&a, b"first").unwrap();
		encode_packet_into(&mut stream, &b, b"second").unwrap();

		let mut cursor = io::Cursor::new(stream);
		let (a1, p1) = read_packet(&mut cursor).unwrap();
		let (b1, p2) = read_packet(&mut cursor).unwrap();
		assert_eq!((a1, p1.as_slice()), (a, b"first".as_slice()));
		assert_eq!((b1, p2.as_slice()), (b, b"second".as_slice()));
	}

	#[test]
	fn read_packet_reports_eof() {
		let mut empty = io::Cursor::new(Vec::<u8>::new());
		let err = read_packet(&mut empty).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
	}

	#[test]
	fn oversized_payload_rejected() {
		let big = vec![0u8; 65536];
		let err = encode_packet(&TargetAddr::IPv4("1.1.1.1".parse().unwrap(), 1), &big).unwrap_err();
		assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
	}
}
