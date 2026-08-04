use bytes::{Buf, BufMut};
use num_enum::{FromPrimitive, IntoPrimitive};
use tokio_util::codec::{Decoder, Encoder};

use super::Command;
use crate::proto::{BytesRemainingSnafu, VER};

#[derive(Debug, Clone, Copy)]
pub struct HeaderCodec;

#[derive(Debug, Clone, PartialEq)]
pub struct Header {
	pub version: u8,
	pub command: CmdType,
}

#[derive(IntoPrimitive, FromPrimitive, Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum CmdType {
	Auth = 0,
	Connect = 1,
	Packet = 2,
	Dissociate = 3,
	Heartbeat = 4,
	#[num_enum(catch_all)]
	Other(u8),
}

impl Header {
	pub fn new(command: CmdType) -> Self {
		Self { version: VER, command }
	}
}

#[cfg(feature = "decode")]
impl Decoder for HeaderCodec {
	type Error = crate::proto::ProtoError;
	type Item = Header;

	fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
		let input: &[u8] = &src[..];
		match super::parse_header(input) {
			Ok((remaining, header)) => {
				let consumed = input.len() - remaining.len();
				src.advance(consumed);
				Ok(Some(header))
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
impl Encoder<Header> for HeaderCodec {
	type Error = crate::proto::ProtoError;

	fn encode(&mut self, item: Header, dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
		dst.reserve(2);
		dst.put_u8(item.version);
		dst.put_u8(item.command.into());
		Ok(())
	}
}

impl From<&Command> for CmdType {
	fn from(value: &Command) -> Self {
		match value {
			Command::Auth { .. } => CmdType::Auth,
			Command::Connect => CmdType::Connect,
			Command::Packet { .. } => CmdType::Packet,
			Command::Dissociate { .. } => CmdType::Dissociate,
			Command::Heartbeat => CmdType::Heartbeat,
		}
	}
}

#[cfg(test)]
mod test {
	use futures_util::SinkExt as _;
	use tokio_stream::StreamExt as _;
	use tokio_util::codec::{FramedRead, FramedWrite};

	use crate::proto::{CmdType, Header, HeaderCodec, ProtoError, VER};

	/// Usual test
	#[test_log::test(tokio::test)]
	async fn test_header_1() -> eyre::Result<()> {
		let header = Header {
			version: VER,
			command: CmdType::Connect,
		};
		let buffer = Vec::with_capacity(2);
		let mut writer = FramedWrite::new(buffer, HeaderCodec);
		let expect_len = 2;

		writer.send(header.clone()).await?;
		assert_eq!(writer.get_ref().len(), expect_len);
		let buffer = writer.get_ref();
		let mut reader = FramedRead::new(buffer.as_slice(), HeaderCodec);

		let frame = reader.next().await.unwrap()?;
		assert_eq!(header, frame);

		Ok(())
	}
	/// Data not fully arrive
	#[test_log::test(tokio::test)]
	async fn test_header_2() -> eyre::Result<()> {
		let header = Header {
			version: VER,
			command: CmdType::Auth,
		};
		let buffer = Vec::with_capacity(2);
		let mut writer = FramedWrite::new(buffer, HeaderCodec);
		writer.send(header.clone()).await?;
		let mut buffer = writer.into_inner();
		let full_len = buffer.len();
		let mut half_b = buffer.split_off(full_len / 2_usize);
		let mut half_a = buffer;
		{
			let mut reader = FramedRead::new(half_a.as_slice(), HeaderCodec);
			assert!(matches!(
				reader.next().await.unwrap().unwrap_err(),
				ProtoError::BytesRemaining
			));
		}
		half_a.append(&mut half_b);
		let mut reader = FramedRead::new(half_a.as_slice(), HeaderCodec);
		assert_eq!(reader.next().await.unwrap()?, header);

		Ok(())
	}
}
