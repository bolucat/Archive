mod error;
pub use error::*;

mod header;
pub use header::*;

mod cmd;
pub use cmd::*;

mod addr;
pub use addr::*;
use bytes::Buf;
use eyre::eyre;
use nom::{
	IResult, Parser,
	bytes::streaming::take,
	number::{
		Endianness,
		streaming::{u8 as nom_u8, u16 as nom_u16},
	},
};
use wind_core::types::TargetAddr;

/// Local error alias for the free wire-helper decoders below.
///
/// The codecs return `Result<Option<Item>, ProtoError>` (the
/// `tokio_util::codec::Decoder` contract — `Ok(None)` means "need more bytes").
/// The free helpers return `Result<Item, eyre::Report>` instead: incomplete
/// input is an error rather than a request for more bytes.
pub type Error = eyre::Report;

pub const VER: u8 = 5;

// ---------------------------------------------------------------------------
// Nom-based streaming parsers (work on `&[u8]`).
//
// All parsers use the *streaming* variants (`nom::bytes::streaming`,
// `nom::number::streaming`) so that incomplete input yields
// `Err(Err::Incomplete(_))` instead of a hard error.  This makes them usable
// both for the free helpers (where `Incomplete` is mapped to an `eyre` error)
// and for the `tokio_util::Decoder` impls (where `Incomplete` is mapped to
// `Ok(None)`).
// ---------------------------------------------------------------------------

type ParseResult<'a, T> = IResult<&'a [u8], T>;

pub(crate) fn parse_header(input: &[u8]) -> ParseResult<'_, Header> {
	let (input, version) = nom_u8(input)?;
	let (input, cmd_byte) = nom_u8(input)?;

	if version != VER {
		return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify)));
	}
	let cmd = CmdType::from(cmd_byte);
	if matches!(cmd, CmdType::Other(_)) {
		return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify)));
	}
	Ok((input, Header::new(cmd)))
}

pub(crate) fn parse_command_body(cmd_type: CmdType, input: &[u8]) -> ParseResult<'_, Command> {
	match cmd_type {
		CmdType::Auth => {
			let (input, uuid_bytes) = take(16usize).parse(input)?;
			let (input, token_bytes) = take(32usize).parse(input)?;
			let mut uuid_arr = [0u8; 16];
			uuid_arr.copy_from_slice(uuid_bytes);
			let mut token = [0u8; 32];
			token.copy_from_slice(token_bytes);
			Ok((
				input,
				Command::Auth {
					uuid: uuid::Uuid::from_bytes(uuid_arr),
					token,
				},
			))
		}
		CmdType::Connect => Ok((input, Command::Connect)),
		CmdType::Packet => {
			let (input, assoc_id) = nom_u16(Endianness::Big).parse(input)?;
			let (input, pkt_id) = nom_u16(Endianness::Big).parse(input)?;
			let (input, frag_total) = nom_u8(input)?;
			let (input, frag_id) = nom_u8(input)?;
			let (input, size) = nom_u16(Endianness::Big).parse(input)?;
			Ok((
				input,
				Command::Packet {
					assoc_id,
					pkt_id,
					frag_total,
					frag_id,
					size,
				},
			))
		}
		CmdType::Dissociate => {
			let (input, assoc_id) = nom_u16(Endianness::Big).parse(input)?;
			Ok((input, Command::Dissociate { assoc_id }))
		}
		CmdType::Heartbeat => Ok((input, Command::Heartbeat)),
		CmdType::Other(_v) => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify))),
	}
}

pub(crate) fn parse_address(input: &[u8]) -> ParseResult<'_, Address> {
	let (input, addr_byte) = nom_u8(input)?;
	let addr_type = AddressType::from(addr_byte);

	match addr_type {
		AddressType::None => Ok((input, Address::None)),
		AddressType::IPv4 => {
			let (input, octets) = take(4usize).parse(input)?;
			let (input, port) = nom_u16(Endianness::Big).parse(input)?;
			let ip = std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]);
			Ok((input, Address::IPv4(ip, port)))
		}
		AddressType::IPv6 => {
			let (input, octets) = take(16usize).parse(input)?;
			let (input, port) = nom_u16(Endianness::Big).parse(input)?;
			let mut arr = [0u8; 16];
			arr.copy_from_slice(octets);
			let ip = std::net::Ipv6Addr::from(arr);
			Ok((input, Address::IPv6(ip, port)))
		}
		AddressType::Domain => {
			let (input, domain_len) = nom_u8(input)?;
			let (input, domain_bytes) = take(domain_len as usize).parse(input)?;
			let (input, port) = nom_u16(Endianness::Big).parse(input)?;
			let s = String::from_utf8(domain_bytes.to_vec())
				.map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify)))?;
			Ok((input, Address::Domain(s, port)))
		}
		AddressType::Other(_v) => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Verify))),
	}
}

// ---------------------------------------------------------------------------
// Thin wrapper: given a contiguous `Buf`, run a nom parser and advance the
// buffer.  All production callers use contiguous buffers (`Bytes`, `BytesMut`,
// `&[u8]`) so `buf.chunk()` returns the full remaining data.
// ---------------------------------------------------------------------------

fn nom_parse<T>(buf: &mut impl Buf, context: &str, parser: impl Fn(&[u8]) -> ParseResult<'_, T>) -> Result<T, Error> {
	let chunk = buf.chunk();
	match parser(chunk) {
		Ok((remaining, value)) => {
			let consumed = chunk.len() - remaining.len();
			buf.advance(consumed);
			Ok(value)
		}
		Err(nom::Err::Incomplete(_)) => Err(eyre!("Incomplete data in {}", context)),
		Err(_) => Err(eyre!("Malformed data in {}", context)),
	}
}

// ---------------------------------------------------------------------------
// Free helper decoders — the production hot path.
// ---------------------------------------------------------------------------

/// Decode a TUIC header from the buffer.
pub fn decode_header(buf: &mut impl Buf, context: &str) -> Result<Header, Error> {
	nom_parse(buf, context, parse_header)
}

/// Decode a TUIC command body from the buffer, given the command type.
pub fn decode_command(cmd_type: CmdType, buf: &mut impl Buf, context: &str) -> Result<Command, Error> {
	nom_parse(buf, context, |input| parse_command_body(cmd_type, input))
}

/// Decode a TUIC address from the buffer.
pub fn decode_address(buf: &mut impl Buf, context: &str) -> Result<Address, Error> {
	nom_parse(buf, context, parse_address)
}

/// Helper function to convert Address to TargetAddr
pub fn address_to_target(addr: Address) -> Result<TargetAddr, Error> {
	match addr {
		Address::Domain(domain, port) => Ok(TargetAddr::Domain(domain, port)),
		Address::IPv4(ip, port) => Ok(TargetAddr::IPv4(ip, port)),
		Address::IPv6(ip, port) => Ok(TargetAddr::IPv6(ip, port)),
		Address::None => Err(eyre!("Address::None cannot be converted to TargetAddr")),
	}
}

// ---------------------------------------------------------------------------
// Regression tests for the wire-helper decoders.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use bytes::{Buf, Bytes};

	use super::*;

	/// Build a SOCKS-style address frame for tests.
	fn ipv4_addr_frame() -> Vec<u8> {
		let mut v = Vec::new();
		v.push(u8::from(AddressType::IPv4));
		v.extend_from_slice(&[127, 0, 0, 1]);
		v.extend_from_slice(&80u16.to_be_bytes());
		v
	}

	fn domain_addr_frame(name: &[u8]) -> Vec<u8> {
		assert!(name.len() <= u8::MAX as usize);
		let mut v = Vec::new();
		v.push(u8::from(AddressType::Domain));
		v.push(name.len() as u8);
		v.extend_from_slice(name);
		v.extend_from_slice(&443u16.to_be_bytes());
		v
	}

	/// `decode_address` must accept contiguous `Bytes` and produce the correct
	/// result (zero-copy nom path).
	#[test]
	fn decode_address_contiguous_bytes() {
		let frame = domain_addr_frame(b"example.com");
		let mut buf = Bytes::from(frame);
		let parsed = decode_address(&mut buf, "contiguous").expect("contiguous parse must succeed");
		match parsed {
			Address::Domain(domain, port) => {
				assert_eq!(domain, "example.com");
				assert_eq!(port, 443);
			}
			other => panic!("expected Domain, got {other:?}"),
		}
		assert_eq!(buf.remaining(), 0, "all bytes should be consumed");
	}

	/// `decode_address` must handle IPv4 from contiguous `Bytes`.
	#[test]
	fn decode_address_ipv4_contiguous() {
		let frame = ipv4_addr_frame();
		let mut buf = Bytes::from(frame);
		let parsed = decode_address(&mut buf, "contiguous").expect("contiguous parse must succeed");
		match parsed {
			Address::IPv4(ip, port) => {
				assert_eq!(ip, std::net::Ipv4Addr::LOCALHOST);
				assert_eq!(port, 80);
			}
			other => panic!("expected IPv4, got {other:?}"),
		}
		assert_eq!(buf.remaining(), 0);
	}

	/// Truncated input must NOT panic — it must return an `Err` with the
	/// "Incomplete ..." context string from the caller.
	#[test]
	fn decode_address_truncated_returns_err() {
		// Just the ATYP byte for a domain — length byte and body are missing.
		let mut buf: &[u8] = &[u8::from(AddressType::Domain)];
		let err = decode_address(&mut buf, "ctx").expect_err("must error on truncated input");
		assert!(format!("{err}").contains("Incomplete"));

		// ATYP + length but no payload.
		let mut buf: &[u8] = &[u8::from(AddressType::Domain), 0x05];
		let err = decode_address(&mut buf, "ctx").expect_err("must error on truncated payload");
		assert!(format!("{err}").contains("Incomplete"));

		// IPv4 missing the last port byte.
		let mut v = ipv4_addr_frame();
		v.pop();
		let mut buf: &[u8] = &v;
		let err = decode_address(&mut buf, "ctx").expect_err("must error on truncated v4");
		assert!(format!("{err}").contains("Incomplete"));
	}
}
