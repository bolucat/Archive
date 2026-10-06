//! Connection-coupled TUIC senders, generic over [`wind_quic::QuicConnection`].
//!
//! Gated on the `encode` feature (it builds wire frames via the codec
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

/// Refuse a payload that does not fit the `u16` `size` field of a TUIC
/// `Packet` command.
///
/// Every `Packet` send path shares this check: the frame carries `size` as a
/// `u16` while the payload bytes that follow it are length-delimited by the
/// stream or datagram, so a larger payload would be *silently* truncated in
/// `size` and the receiver would account for fewer bytes than the frame
/// actually carried.
pub(crate) fn check_packet_payload_size(payload_len: usize) -> Result<(), Error> {
	// Both send paths report the same field name, so the error does not depend
	// on which route built the frame.
	if payload_len > u16::MAX as usize {
		return Err(crate::proto::NumericOverflowSnafu {
			field: "UDP payload size",
			num: payload_len.to_string(),
		}
		.build()
		.into());
	}
	Ok(())
}

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
	// Finish the stream so the peer's `read_to_end` (or equivalent EOF
	// detector) observes a clean end-of-stream marker. Without this, dropping
	// `send` resets the stream and the receiver sees a RESET_STREAM frame
	// racing the payload, which intermittently breaks auth/dissociate/uni-UDP
	// paths.
	send.finish()?;
	Ok(())
}

/// Open a bidirectional stream and write the TUIC `Connect` command header
/// for `addr`.
///
/// Shared by the generic [`ClientProtoExt::open_tcp`] relay and the
/// quinn-specific `TuicOutbound::connect_tcp`, which hands the raw stream to
/// its caller instead of relaying it internally.
pub async fn open_connect_stream<C: QuicConnection>(
	conn: &C,
	addr: &TargetAddr,
) -> Result<(C::SendStream, C::RecvStream), Error> {
	let (mut send, recv) = conn.open_bi().await?;
	let mut buf = BytesMut::with_capacity(9);
	HeaderCodec.encode(Header::new(CmdType::Connect), &mut buf)?;
	CmdCodec(CmdType::Connect).encode(Command::Connect, &mut buf)?;
	AddressCodec.encode(addr.to_owned().into(), &mut buf)?;
	send.write_all(&buf).await?;
	Ok((send, recv))
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
		// Generate the authentication token from the TLS keying-material
		// exporter (RFC 5705): label = UUID bytes, context = password.
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
		let (send, recv) = open_connect_stream(self, addr).await?;
		// Join the recv/send halves into one duplex stream for the
		// bidirectional relay (replaces the quinn-specific `QuinnCompat`).
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
		// The `size` field below is a `u16`: refuse anything larger instead of
		// truncating it and writing a frame whose `size` disagrees with the
		// payload that follows (`UdpStream::send_packet` applies the same guard
		// for its own callers; this trait method is public and can be reached
		// directly).
		check_packet_payload_size(payload.len())?;
		// Pre-size for header (2) + Packet command (8) + address + payload so
		// the datagram branch ships a single `Bytes` without a second
		// allocation.
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

#[cfg(test)]
mod tests {
	use std::{
		net::{Ipv4Addr, SocketAddr},
		pin::Pin,
		sync::{Arc, Mutex},
		task::{Context as TaskContext, Poll},
	};

	use bytes::Bytes;
	use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
	use wind_quic::{QuicConnection, QuicError, QuicRecvStream, QuicSendStream};

	use super::*;

	/// A [`QuicConnection`] double that records what reached the two UDP send
	/// paths, so a test can assert that a refused packet wrote nothing.
	#[derive(Clone)]
	struct RecordingUdpConn {
		/// `Some` when the peer advertises DATAGRAM support (the `Native` relay
		/// path); `None` forces the unidirectional-stream path.
		max_datagram: Option<usize>,
		datagrams: Arc<Mutex<Vec<Bytes>>>,
		uni_streams: Arc<Mutex<Vec<Vec<u8>>>>,
	}

	impl RecordingUdpConn {
		fn new(max_datagram: Option<usize>) -> Self {
			Self {
				max_datagram,
				datagrams: Arc::new(Mutex::new(Vec::new())),
				uni_streams: Arc::new(Mutex::new(Vec::new())),
			}
		}

		fn datagrams(&self) -> Vec<Bytes> {
			self.datagrams.lock().unwrap().clone()
		}

		fn uni_streams(&self) -> Vec<Vec<u8>> {
			self.uni_streams.lock().unwrap().clone()
		}
	}

	/// Accumulates one unidirectional stream's writes and publishes them on
	/// `finish`, mirroring the `write_all` + `finish` sequence `send_udp` uses.
	struct RecordingSend {
		sink: Arc<Mutex<Vec<Vec<u8>>>>,
		buf: Vec<u8>,
	}

	impl AsyncWrite for RecordingSend {
		fn poll_write(mut self: Pin<&mut Self>, _cx: &mut TaskContext<'_>, data: &[u8]) -> Poll<std::io::Result<usize>> {
			self.buf.extend_from_slice(data);
			Poll::Ready(Ok(data.len()))
		}

		fn poll_flush(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
			Poll::Ready(Ok(()))
		}

		fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
			Poll::Ready(Ok(()))
		}
	}

	impl QuicSendStream for RecordingSend {
		fn finish(&mut self) -> Result<(), QuicError> {
			self.sink.lock().unwrap().push(std::mem::take(&mut self.buf));
			Ok(())
		}

		fn reset(&mut self, _code: u64) {}

		fn id(&self) -> u64 {
			0
		}
	}

	/// Receive half placeholder: the send path never accepts streams.
	struct StubRecv;

	impl AsyncRead for StubRecv {
		fn poll_read(self: Pin<&mut Self>, _cx: &mut TaskContext<'_>, _buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
			Poll::Ready(Ok(()))
		}
	}

	impl QuicRecvStream for StubRecv {
		fn stop(&mut self, _code: u64) {}

		fn id(&self) -> u64 {
			0
		}
	}

	impl QuicConnection for RecordingUdpConn {
		type RecvStream = StubRecv;
		type SendStream = RecordingSend;

		async fn open_bi(&self) -> Result<(Self::SendStream, Self::RecvStream), QuicError> {
			Err(QuicError::Other("open_bi is not part of the UDP send path".into()))
		}

		async fn accept_bi(&self) -> Result<(Self::SendStream, Self::RecvStream), QuicError> {
			Err(QuicError::Other("accept_bi is not part of the UDP send path".into()))
		}

		async fn open_uni(&self) -> Result<Self::SendStream, QuicError> {
			Ok(RecordingSend {
				sink: self.uni_streams.clone(),
				buf: Vec::new(),
			})
		}

		async fn accept_uni(&self) -> Result<Self::RecvStream, QuicError> {
			Err(QuicError::Other("accept_uni is not part of the UDP send path".into()))
		}

		fn send_datagram(&self, data: Bytes) -> Result<(), QuicError> {
			self.datagrams.lock().unwrap().push(data);
			Ok(())
		}

		async fn read_datagram(&self) -> Result<Bytes, QuicError> {
			Err(QuicError::Other("read_datagram is not part of the UDP send path".into()))
		}

		fn max_datagram_size(&self) -> Option<usize> {
			self.max_datagram
		}

		async fn export_keying_material(&self, _out: &mut [u8], _label: &[u8], _context: &[u8]) -> Result<(), QuicError> {
			Ok(())
		}

		fn close(&self, _code: u32, _reason: &[u8]) {}

		async fn closed(&self) {
			std::future::pending::<()>().await;
		}

		fn peer_addr(&self) -> Option<SocketAddr> {
			Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
		}
	}

	/// The `size` field of a `Packet` command is a `u16`; a larger payload must
	/// be refused with the numeric overflow named rather than truncated into a
	/// frame whose `size` disagrees with the bytes that follow it. Regression
	/// for the `send_udp` route, which every public UDP send path reaches.
	#[tokio::test]
	async fn send_udp_refuses_a_payload_that_does_not_fit_the_size_field() {
		for max_datagram in [Some(1200usize), None] {
			let conn = RecordingUdpConn::new(max_datagram);
			let payload_len = u16::MAX as usize + 1;

			let err = conn
				.send_udp(
					7,
					1,
					&TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080),
					Bytes::from(vec![0xABu8; payload_len]),
					true,
				)
				.await
				.expect_err("a payload that does not fit `size` must be refused");

			match err.downcast_ref::<crate::proto::ProtoError>() {
				Some(crate::proto::ProtoError::NumericOverflow { field, num, .. }) => {
					assert_eq!(field, "UDP payload size");
					assert_eq!(num, &payload_len.to_string());
				}
				other => panic!("expected NumericOverflow, got {other:?}"),
			}
			assert!(
				conn.datagrams().is_empty(),
				"a refused packet must not be sent as a datagram (max_datagram={max_datagram:?})"
			);
			assert!(
				conn.uni_streams().is_empty(),
				"a refused packet must not be sent on a uni stream (max_datagram={max_datagram:?})"
			);
		}
	}

	/// Boundary control: the largest payload that does fit `size` still takes
	/// the requested send path, so the guard cannot silently drop valid
	/// traffic.
	#[tokio::test]
	async fn send_udp_accepts_the_largest_payload_that_fits_the_size_field() {
		let conn = RecordingUdpConn::new(Some(1200));
		let payload = vec![0xCDu8; u16::MAX as usize];

		conn.send_udp(
			7,
			1,
			&TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080),
			Bytes::from(payload.clone()),
			true,
		)
		.await
		.expect("a payload that fits `size` must be sent");

		let datagrams = conn.datagrams();
		assert_eq!(datagrams.len(), 1, "the accepted packet reached the wire");
		assert!(conn.uni_streams().is_empty(), "datagram mode must not open a uni stream");
		// 2 header + 8 command + 7 IPv4 address = 17 bytes of framing, then the
		// whole payload.
		let frame = &datagrams[0];
		assert_eq!(frame.len(), 17 + payload.len());
		assert_eq!(
			u16::from_be_bytes([frame[8], frame[9]]),
			u16::MAX,
			"`size` must name the payload"
		);
		assert_eq!(&frame[17..], &payload[..]);
	}
}
