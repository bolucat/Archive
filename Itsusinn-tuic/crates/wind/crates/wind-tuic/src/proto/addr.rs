use std::net::{Ipv4Addr, Ipv6Addr};

use bytes::{Buf, BufMut};
use num_enum::{FromPrimitive, IntoPrimitive};
use tokio_util::codec::{Decoder, Encoder};
use wind_core::types::TargetAddr;

use crate::proto::{BytesRemainingSnafu, DomainTooLongSnafu, ProtoError};

/// Codec for TUIC address encoding and decoding
#[derive(Debug, Clone, Copy)]
pub struct AddressCodec;

/// TUIC Address representation
#[derive(Debug, Clone, PartialEq)]
pub enum Address {
	/// No address
	None,
	/// Domain name and port
	Domain(String, u16),
	/// IPv4 address and port
	IPv4(Ipv4Addr, u16),
	/// IPv6 address and port
	IPv6(Ipv6Addr, u16),
}

/// Address type indicators as defined in TUIC protocol
#[derive(IntoPrimitive, FromPrimitive, Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum AddressType {
	None = u8::MAX,
	Domain = 0,
	IPv4 = 1,
	IPv6 = 2,
	#[num_enum(catch_all)]
	Other(u8),
}

impl From<TargetAddr> for Address {
	fn from(value: TargetAddr) -> Self {
		match value {
			TargetAddr::Domain(s, port) => Self::Domain(s, port),
			TargetAddr::IPv4(addr, port) => Self::IPv4(addr, port),
			TargetAddr::IPv6(addr, port) => Self::IPv6(addr, port),
		}
	}
}

/// Implementation according to TUIC specification:
/// https://github.com/proxy-rs/wind/blob/main/crates/wind-tuic/SPEC.md#6-address-encoding
#[cfg(feature = "decode")]
impl Decoder for AddressCodec {
	type Error = ProtoError;
	type Item = Address;

	fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		let input: &[u8] = &src[..];
		match super::parse_address(input) {
			Ok((remaining, address)) => {
				let consumed = input.len() - remaining.len();
				src.advance(consumed);
				Ok(Some(address))
			}
			Err(nom::Err::Incomplete(_)) => Ok(None),
			Err(_) => BytesRemainingSnafu.fail(),
		}
	}

	fn decode_eof(&mut self, buf: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		match self.decode(buf) {
			Ok(None) => BytesRemainingSnafu.fail(),
			v => v,
		}
	}
}

#[cfg(feature = "encode")]
impl Encoder<Address> for AddressCodec {
	type Error = ProtoError;

	fn encode(&mut self, item: Address, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
		match item {
			Address::None => {
				dst.reserve(1);
				dst.put_u8(AddressType::None.into());
			}
			Address::IPv4(ip, port) => {
				// Type (1) + IPv4 (4) + Port (2)
				dst.reserve(1 + 4 + 2);
				dst.put_u8(AddressType::IPv4.into());
				dst.put_slice(&ip.octets());
				dst.put_u16(port);
			}
			Address::IPv6(ip, port) => {
				// Type (1) + IPv6 (16) + Port (2)
				dst.reserve(1 + 16 + 2);
				dst.put_u8(AddressType::IPv6.into());
				dst.put_slice(&ip.octets());
				dst.put_u16(port);
			}
			Address::Domain(domain, port) => {
				if domain.len() > u8::MAX as usize {
					return DomainTooLongSnafu { domain }.fail();
				}

				// Type (1) + Length (1) + Domain + Port (2)
				dst.reserve(1 + 1 + domain.len() + 2);
				dst.put_u8(AddressType::Domain.into());
				dst.put_u8(domain.len() as u8);
				dst.put_slice(domain.as_bytes());
				dst.put_u16(port);
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use std::net::{Ipv4Addr, Ipv6Addr};

	use futures_util::SinkExt as _;
	use tokio_stream::StreamExt as _;
	use tokio_util::codec::{FramedRead, FramedWrite};

	use super::{Address, AddressCodec};
	use crate::proto::ProtoError;

	/// Test complete encoding and decoding cycle for all address types
	#[test_log::test(tokio::test)]
	async fn test_addr_1() -> eyre::Result<()> {
		let buffer = Vec::with_capacity(128);
		let vars = vec![
			Address::None,
			Address::IPv4(Ipv4Addr::LOCALHOST, 80),
			Address::IPv6(Ipv6Addr::UNSPECIFIED, 12),
			Address::Domain(String::from("www.google.com"), 443),
		];

		let mut writer = FramedWrite::new(buffer, AddressCodec);
		let mut expect_len = 0;
		for var in &vars {
			match var {
				Address::None => expect_len += 1,
				Address::Domain(domain, _) => expect_len += 1 + 1 + domain.len() + 2,
				Address::IPv4(..) => expect_len += 1 + 4 + 2,
				Address::IPv6(..) => expect_len += 1 + 16 + 2,
			}
			writer.send(var.clone()).await?;
			assert_eq!(writer.get_ref().len(), expect_len);
		}

		let buffer = writer.get_ref();
		let mut reader = FramedRead::new(buffer.as_slice(), AddressCodec);
		for var in vars {
			let frame = reader.next().await.unwrap()?;
			assert_eq!(var, frame);
		}
		Ok(())
	}

	/// Test behavior with partial data (simulating streaming data arrival)
	#[test_log::test(tokio::test)]
	async fn test_addr_2() -> eyre::Result<()> {
		let vars = vec![
			Address::IPv4(Ipv4Addr::LOCALHOST, 80),
			Address::IPv6(Ipv6Addr::UNSPECIFIED, 12),
			Address::Domain(String::from("www.google.com"), 443),
		];

		for addr in vars {
			let buffer = Vec::with_capacity(128);
			let mut writer = FramedWrite::new(buffer, AddressCodec);
			writer.send(addr.clone()).await?;
			let mut buffer = writer.into_inner();

			// Split the encoded data in half to simulate partial data arrival
			let full_len = buffer.len();
			let mut half_b = buffer.split_off(full_len / 2_usize);
			let mut half_a = buffer;

			// First half should result in BytesRemaining error
			{
				let mut reader = FramedRead::new(half_a.as_slice(), AddressCodec);
				assert!(matches!(
					reader.next().await.unwrap().unwrap_err(),
					ProtoError::BytesRemaining
				));
			}

			// Recombined buffer should decode properly
			half_a.append(&mut half_b);
			let mut reader = FramedRead::new(half_a.as_slice(), AddressCodec);
			assert_eq!(reader.next().await.unwrap()?, addr);
		}

		Ok(())
	}

	/// Test to generate and inspect hex encoding (useful for debugging)
	#[test_log::test(tokio::test)]
	async fn hex_check() -> eyre::Result<()> {
		let mut buffer = Vec::new();
		let vars = [
			Address::None,
			Address::IPv4(Ipv4Addr::LOCALHOST, 80),
			Address::IPv6(Ipv6Addr::LOCALHOST, 12),
			Address::Domain(String::from("www.google.com"), 443),
		];

		// Encode the second address and print its hex representation
		FramedWrite::new(&mut buffer, AddressCodec).send(vars[1].clone()).await?;
		tracing::info!("{}", hex::encode(buffer));
		Ok(())
	}
}
