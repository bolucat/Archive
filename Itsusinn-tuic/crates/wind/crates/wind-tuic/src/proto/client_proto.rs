//! Connection-coupled TUIC senders, generic over [`wind_quic::QuicConnection`].
//!
//! Gated on the `encode` feature (it builds wire frames via the `tuic_core`
//! encoders). Both the client outbound and the server's UDP response path use
//! [`ClientProtoExt`].

use std::future::Future;

use bytes::BytesMut;
use eyre::eyre;
use tokio::io::AsyncWriteExt as _;
use tokio_util::codec::Encoder;
use wind_core::{tcp::AbstractTcpStream, types::TargetAddr};
use wind_quic::{QuicConnection, QuicSendStream as _};

use crate::{
	Error,
	proto::{Address, AddressCodec, CmdCodec, CmdType, Command, Header, HeaderCodec},
};

/// Encode a single command (+ optional address) and ship it on a fresh
/// unidirectional stream, finishing cleanly so the peer observes EOF.
pub async fn encode_and_send_uni<C: QuicConnection>(
	conn: &C,
	cmd_type: CmdType,
	command: Command,
	address: Option<Address>,
) -> Result<(), Error> {
	let mut buf = BytesMut::with_capacity(64);
	HeaderCodec.encode(Header::new(cmd_type), &mut buf)?;
	CmdCodec(cmd_type).encode(command, &mut buf)?;
	if let Some(addr) = address {
		AddressCodec.encode(addr, &mut buf)?;
	}
	let mut send = conn.open_uni().await?;
	send.write_all(&buf).await?;
	// Finish the stream so the peer's `read_to_end` (or equivalent EOF detector)
	// observes a clean end-of-stream marker. Without this, dropping `send` resets
	// the stream and the receiver sees a RESET_STREAM frame racing the payload,
	// which intermittently breaks auth/dissociate/uni-UDP paths.
	send.finish()?;
	Ok(())
}

/// Client-side TUIC senders, available on any [`QuicConnection`]. Despite the
/// name the server's UDP response path uses
/// [`send_udp`](ClientProtoExt::send_udp)
/// too (via [`UdpStream`](super::UdpStream)).
pub trait ClientProtoExt: QuicConnection {
	fn send_auth(&self, uuid: &uuid::Uuid, secret: &[u8]) -> impl Future<Output = Result<(), Error>> + Send;
	fn send_heartbeat(&self) -> impl Future<Output = Result<(), Error>> + Send;
	fn open_tcp(
		&self,
		addr: &TargetAddr,
		stream: impl AbstractTcpStream,
	) -> impl Future<Output = Result<(usize, usize), Error>> + Send;
	fn send_udp(
		&self,
		assoc_id: u16,
		pkt_id: u16,
		addr: &TargetAddr,
		packet: bytes::Bytes,
		datagram: bool,
	) -> impl Future<Output = Result<(), Error>> + Send;
	fn drop_udp(&self, assoc_id: u16) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<C: QuicConnection> ClientProtoExt for C {
	async fn send_auth(&self, uuid: &uuid::Uuid, secret: &[u8]) -> Result<(), Error> {
		// Generate the authentication token from the TLS keying-material exporter
		// (RFC 5705): label = UUID bytes, context = password.
		let mut token = [0u8; 32];
		self.export_keying_material(&mut token, uuid.as_bytes(), secret)
			.await
			.map_err(|e| eyre!("export_keying_material failed: {e}"))?;

		let auth_cmd = Command::Auth { uuid: *uuid, token };

		// 2 bytes header + 16 bytes UUID + 32 bytes token.
		let mut buf = BytesMut::with_capacity(2 + 16 + 32);
		HeaderCodec.encode(Header::new(CmdType::Auth), &mut buf)?;
		CmdCodec(CmdType::Auth).encode(auth_cmd, &mut buf)?;

		let mut send = self.open_uni().await?;
		send.write_all(&buf).await?;
		// Clean EOF — see note on `encode_and_send_uni`.
		send.finish()?;
		Ok(())
	}

	async fn open_tcp(&self, addr: &TargetAddr, mut stream: impl AbstractTcpStream) -> Result<(usize, usize), Error> {
		let (mut send, recv) = self.open_bi().await?;
		let mut buf = BytesMut::with_capacity(9);
		HeaderCodec.encode(Header::new(CmdType::Connect), &mut buf)?;
		CmdCodec(CmdType::Connect).encode(Command::Connect, &mut buf)?;
		AddressCodec.encode(addr.to_owned().into(), &mut buf)?;
		send.write_all(&buf).await?;
		// Join the recv/send halves into one duplex stream for the bidirectional
		// relay (replaces the quinn-specific `QuinnCompat`).
		let mut duplex = tokio::io::join(recv, send);
		let (a, b, err) = wind_core::io::copy_io(&mut stream, &mut duplex).await;
		if let Some(e) = err {
			return Err(e.into());
		}
		Ok((a, b))
	}

	async fn send_udp(
		&self,
		assoc_id: u16,
		pkt_id: u16,
		addr: &TargetAddr,
		payload: bytes::Bytes,
		datagram: bool,
	) -> Result<(), Error> {
		// Pre-size for header (2) + Packet command (8) + address + payload so the
		// datagram branch ships a single `Bytes` without a second allocation.
		let addr_size = match addr {
			TargetAddr::IPv4(..) => 1 + 4 + 2,
			TargetAddr::IPv6(..) => 1 + 16 + 2,
			TargetAddr::Domain(d, _) => 1 + 1 + d.len() + 2,
		};
		let header_overhead = 2 + 8 + addr_size;
		let mut buf = BytesMut::with_capacity(header_overhead + if datagram { payload.len() } else { 0 });
		HeaderCodec.encode(Header::new(CmdType::Packet), &mut buf)?;
		CmdCodec(CmdType::Packet).encode(
			Command::Packet {
				assoc_id,
				pkt_id,
				frag_total: 1,
				frag_id: 0,
				size: payload.len() as u16,
			},
			&mut buf,
		)?;
		AddressCodec.encode(addr.to_owned().into(), &mut buf)?;
		if datagram {
			buf.extend_from_slice(&payload);
			self.send_datagram(buf.freeze())?;
		} else {
			let mut send = self.open_uni().await?;
			send.write_all(&buf).await?;
			send.write_all(&payload).await?;
			// Clean EOF — see note on `encode_and_send_uni`.
			send.finish()?;
		}
		Ok(())
	}

	async fn drop_udp(&self, assoc_id: u16) -> Result<(), Error> {
		let mut send = self.open_uni().await?;
		let mut buf = BytesMut::with_capacity(4);
		HeaderCodec.encode(Header::new(CmdType::Dissociate), &mut buf)?;
		CmdCodec(CmdType::Dissociate).encode(Command::Dissociate { assoc_id }, &mut buf)?;
		send.write_all(&buf).await?;
		// Clean EOF — see note on `encode_and_send_uni`.
		send.finish()?;
		Ok(())
	}

	async fn send_heartbeat(&self) -> Result<(), Error> {
		// 2 bytes: version + command. Sent as a datagram for lowest latency.
		let mut buf = BytesMut::with_capacity(2);
		HeaderCodec.encode(Header::new(CmdType::Heartbeat), &mut buf)?;
		self.send_datagram(buf.freeze())?;
		Ok(())
	}
}
