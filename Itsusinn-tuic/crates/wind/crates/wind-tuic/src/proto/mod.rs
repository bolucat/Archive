//! TUIC wire protocol codecs and connection-coupled glue, backend-agnostic
//! over [`wind_quic::QuicConnection`].
//!
//! The on-wire codecs ([`HeaderCodec`], [`CmdCodec`], [`AddressCodec`]),
//! [`ProtoError`], and the pure decode helpers ([`decode_header`],
//! [`decode_command`], [`decode_address`], [`address_to_target`]) are defined
//! here and shared by both the quinn and quiche backends.
//! The connection-coupled glue ([`ClientProtoExt`], [`encode_and_send_uni`],
//! [`UdpStream`]) is gated on the `encode` feature.

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

type ParseResult<'a, T> = IResult<&'a [u8], T, ParseError>;

pub(crate) fn parse_header(input: &[u8]) -> ParseResult<'_, Header> {
	let (input, version) = nom_u8(input)?;
	let (input, cmd_byte) = nom_u8(input)?;

	if version != VER {
		return Err(nom::Err::Error(ParseError::VersionMismatch {
			expect: VER,
			current: version,
		}));
	}
	let cmd = CmdType::from(cmd_byte);
	if let CmdType::Other(value) = cmd {
		return Err(nom::Err::Error(ParseError::UnknownCommandType { value }));
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
		CmdType::Other(value) => Err(nom::Err::Error(ParseError::UnknownCommandType { value })),
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
			let s = String::from_utf8(domain_bytes.to_vec()).map_err(|e| {
				nom::Err::Error(ParseError::FailParseDomain {
					raw: hex_encode(domain_bytes),
					source: e.utf8_error(),
				})
			})?;
			Ok((input, Address::Domain(s, port)))
		}
		AddressType::Other(value) => Err(nom::Err::Error(ParseError::UnknownAddressType { value })),
	}
}

/// Hex dump for [`ParseError::FailParseDomain`]'s `raw` field.
///
/// `const-hex` is only a dev-dependency of this crate, so the (rare) failure
/// path spells the two nibbles out instead of taking a runtime dependency.
fn hex_encode(bytes: &[u8]) -> String {
	const HEX: &[u8; 16] = b"0123456789abcdef";
	let mut out = String::with_capacity(bytes.len() * 2);
	for byte in bytes {
		out.push(HEX[usize::from(byte >> 4)] as char);
		out.push(HEX[usize::from(byte & 0x0f)] as char);
	}
	out
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
		Err(nom::Err::Error(err) | nom::Err::Failure(err)) => Err(eyre!("Malformed data in {context}: {err}")),
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

/// How outgoing `Packet` commands travel on the wire.
///
/// * `Native` — QUIC DATAGRAM frames (RFC 9221), fragmented to fit
///   `max_datagram_size`.
/// * `Quic` — one unidirectional QUIC stream per packet.
///
/// Defined in the backend-neutral `proto` module so both the quinn and quiche
/// front-ends can share it (the quinn module re-exports it at its historical
/// `quinn::UdpRelayMode` path).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UdpRelayMode {
	Native,
	Quic,
}

impl std::fmt::Display for UdpRelayMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Native => write!(f, "native"),
			Self::Quic => write!(f, "quic"),
		}
	}
}

impl std::str::FromStr for UdpRelayMode {
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

// ---------------------------------------------------------------------------
// Connection-coupled glue (gated on `encode`).
// ---------------------------------------------------------------------------

#[cfg(feature = "encode")]
mod client_proto;
#[cfg(feature = "encode")]
mod udp_stream;

#[cfg(feature = "encode")]
pub use client_proto::*;
#[cfg(feature = "encode")]
pub use udp_stream::*;

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

	/// Regression (W36/W37): the free helpers are the production hot path, so
	/// `Malformed data in <context>` must also say *what* was malformed. Before
	/// the fix every parse failure was reported as `ErrorKind::Verify` and the
	/// reason never reached the caller.
	#[test]
	fn decode_header_reports_a_version_mismatch() {
		let mut buf: &[u8] = &[0x04, u8::from(CmdType::Connect)];

		let err = decode_header(&mut buf, "ctx").expect_err("a foreign version must fail");

		let message = format!("{err}");
		assert!(message.contains("Malformed data in ctx"), "unexpected message: {message}");
		assert!(
			message.contains("Version mismatch: expected 5, got 4"),
			"unexpected message: {message}"
		);
	}

	/// Regression (W36/W37): an unknown command type must be named, with the
	/// offending byte, on the production decode path as well.
	#[test]
	fn decode_header_reports_an_unknown_command_type() {
		let mut buf: &[u8] = &[VER, 0x09];

		let err = decode_header(&mut buf, "ctx").expect_err("an unknown command must fail");

		let message = format!("{err}");
		assert!(message.contains("Unknown command type 9"), "unexpected message: {message}");
	}

	/// Regression (W36/W37): an unknown address type must be named, with the
	/// offending byte, on the production decode path as well.
	#[test]
	fn decode_address_reports_an_unknown_address_type() {
		let mut buf: &[u8] = &[0x07];

		let err = decode_address(&mut buf, "ctx").expect_err("an unknown address type must fail");

		let message = format!("{err}");
		assert!(
			message.contains("Unable to decode address due to type 7"),
			"unexpected message: {message}"
		);
	}

	/// Regression (W36): a non-UTF-8 domain keeps a hex dump of the raw bytes
	/// in the error, so a malformed peer frame can still be diagnosed.
	#[test]
	fn decode_address_reports_a_non_utf8_domain() {
		let mut frame = vec![u8::from(AddressType::Domain), 2, 0xff, 0xfe];
		frame.extend_from_slice(&443u16.to_be_bytes());
		let mut buf: &[u8] = &frame;

		let err = decode_address(&mut buf, "ctx").expect_err("a non-UTF-8 domain must fail");

		let message = format!("{err}");
		assert!(
			message.contains("Unable to decode domain fffe as UTF-8"),
			"unexpected message: {message}"
		);
	}
}
