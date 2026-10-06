use std::{
	ops::ControlFlow,
	sync::atomic::{AtomicU16, Ordering},
	time::Duration,
};

use bytes::{BufMut, Bytes, BytesMut};
use crossfire::{MAsyncTx, mpmc};
use tokio_util::codec::Encoder;
use wind_core::{types::TargetAddr, udp::UdpPacket};

type UdpPacketTx = MAsyncTx<mpmc::Array<UdpPacket>>;

use wind_quic::QuicConnection;

use crate::{
	proto::{
		Address, AddressCodec, ClientProtoExt as _, CmdCodec, CmdType, Command, Header, HeaderCodec, UdpRelayMode,
		check_packet_payload_size,
	},
	udp::{DEFAULT_FRAGMENT_TIMEOUT, FragmentInfo, FragmentReassemblyBuffer, MAX_FRAGMENTS},
};

/// A TUIC UDP association over any [`QuicConnection`].
///
/// The fragment **reassembly** state machine lives in
/// [`crate::udp::FragmentReassemblyBuffer`]; this type owns the
/// connection-coupled send path (datagram sizing, fragmentation, dispatch) and
/// bridges reassembled packets to a receive channel.
pub struct UdpStream<C: QuicConnection> {
	connection: C,
	assoc_id: u16,
	receive_tx: UdpPacketTx,
	next_pkt_id: AtomicU16,
	/// Fragment reassembly state machine (backend-agnostic).
	fragment_buffer: FragmentReassemblyBuffer,
	/// How outgoing `Packet` commands are carried (datagrams or uni streams).
	relay_mode: UdpRelayMode,
}

impl<C: QuicConnection> UdpStream<C> {
	pub fn new(connection: C, assoc_id: u16, receive_tx: UdpPacketTx) -> Self {
		Self {
			connection,
			assoc_id,
			receive_tx,
			next_pkt_id: AtomicU16::new(0),
			fragment_buffer: FragmentReassemblyBuffer::new(DEFAULT_FRAGMENT_TIMEOUT),
			relay_mode: UdpRelayMode::Native,
		}
	}

	/// Select how outgoing packets are relayed: `Native` (the default) sends
	/// QUIC datagrams when the peer supports them, `Quic` opens one
	/// unidirectional stream per packet.
	pub fn with_relay_mode(mut self, mode: UdpRelayMode) -> Self {
		self.relay_mode = mode;
		self
	}

	/// Whether outgoing `Packet` commands can travel as QUIC DATAGRAM frames.
	///
	/// `Native` is a *preference*: the peer must also advertise DATAGRAM
	/// support. quiche rejects `dgram_send` without the peer's
	/// `max_datagram_frame_size` (and closes the connection on a DATAGRAM frame
	/// it did not enable), while quinn fails `send_datagram` outright.
	/// `max_datagram_size` is the backend-neutral capability probe, so a peer
	/// that reports `None` gets one unidirectional stream per packet instead —
	/// otherwise every reply to a peer configured for stream relay
	/// (`udp_relay_mode = "quic"`) is silently discarded.
	fn datagram_relay_available(&self) -> bool {
		self.relay_mode == UdpRelayMode::Native && self.connection.max_datagram_size().is_some()
	}

	/// Configure how long incomplete fragment groups are retained before
	/// [`collect_garbage`](Self::collect_garbage) evicts them.
	///
	/// The lifetime is applied by rebuilding the reassembly buffer: it is fixed
	/// when the buffer is created, so a lifetime that only reached
	/// `collect_garbage` could never take effect. Callers set it during
	/// construction, before any fragment arrives.
	pub fn with_gc_lifetime(mut self, lifetime: Duration) -> Self {
		self.fragment_buffer = FragmentReassemblyBuffer::new(lifetime);
		self
	}

	pub async fn send_packet(&self, packet: UdpPacket) -> eyre::Result<()> {
		let payload_len = packet.payload.len();

		// The `size` field of a `Packet` command is a `u16`; refuse to send
		// anything that would silently truncate (reachable in QUIC relay mode,
		// where a stream carries an arbitrary-length payload). `send_udp`
		// applies the same guard, so both send routes fail identically.
		check_packet_payload_size(payload_len)?;

		// QUIC relay mode — or a peer that cannot receive DATAGRAM frames —
		// carries one unidirectional stream per packet, no fragmentation
		// (streams are flow-controlled, not MTU-bounded).
		if !self.datagram_relay_available() {
			let pkt_id = self.next_pkt_id.fetch_add(1, Ordering::Relaxed);
			self.connection
				.send_udp(self.assoc_id, pkt_id, &packet.target, packet.payload, false)
				.await?;
			return Ok(());
		}

		let addr_size = match packet.target {
			TargetAddr::IPv4(..) => 1 + 4 + 2,  // Type (1) + IPv4 (4) + Port (2)
			TargetAddr::IPv6(..) => 1 + 16 + 2, // Type (1) + IPv6 (16) + Port (2)
			TargetAddr::Domain(ref domain, _) => {
				let domain_len = domain.len();
				if domain_len > 255 {
					return Err(eyre::eyre!("Domain name too long"));
				}
				1 + 1 + domain_len + 2 // Type (1) + Length (1) + Domain + Port (2)
			}
		};

		// Header (2 bytes) + Command (8 bytes) + Address
		let header_overhead = 10 + addr_size;
		// `saturating_sub` so that a transiently tiny `max_datagram_size`
		// (well below the 10+addr_size header overhead) cannot underflow into
		// `usize::MAX` and let an arbitrarily large payload sneak through the
		// single-datagram branch (where it would then exceed
		// `send_datagram`'s size limit).
		let max_datagram_size = self.connection.max_datagram_size().unwrap_or(1200);
		let single_dg_payload_max = max_datagram_size.saturating_sub(header_overhead);
		if payload_len <= single_dg_payload_max {
			// Allocate the packet id atomically — `load` then `fetch_add` is
			// racy: two concurrent send_packet calls could read the same id
			// and emit two datagrams with identical (assoc_id, pkt_id), which
			// collides with the receiver's fragment-reassembly state.
			let pkt_id = self.next_pkt_id.fetch_add(1, Ordering::Relaxed);
			self.connection
				.send_udp(self.assoc_id, pkt_id, &packet.target, packet.payload, true)
				.await?;
			return Ok(());
		}

		self.send_fragmented_packet(packet).await
	}

	async fn send_fragmented_packet(&self, packet: UdpPacket) -> eyre::Result<()> {
		let payload_len = packet.payload.len();

		let first_frag_addr_size = match packet.target {
			TargetAddr::IPv4(..) => 1 + 4 + 2,
			TargetAddr::IPv6(..) => 1 + 16 + 2,
			TargetAddr::Domain(ref domain, _) => 1 + 1 + domain.len() + 2,
		};
		// Subsequent fragments use Address::None which is only 1 byte
		let subsequent_frag_addr_size = 1;

		// Header (2 bytes) + Command (8 bytes) + Address
		let max_datagram_size = self.connection.max_datagram_size().unwrap_or(1200);
		let first_frag_header_overhead = 10 + first_frag_addr_size;
		let subsequent_frag_header_overhead = 10 + subsequent_frag_addr_size;
		let first_frag_max_payload = max_datagram_size.saturating_sub(first_frag_header_overhead);
		let subsequent_frag_max_payload = max_datagram_size.saturating_sub(subsequent_frag_header_overhead);

		// Guard against pathological `max_datagram_size` values where the
		// header overhead consumes the entire datagram. In that case both
		// `saturating_sub`s yield 0 and `div_ceil(0)` would panic; refuse the
		// send instead. This was previously reachable both by an adversarial
		// peer advertising a tiny max_datagram_size and by an unusually long
		// domain target that inflated the first-fragment overhead past the
		// datagram size.
		if first_frag_max_payload == 0 || subsequent_frag_max_payload == 0 {
			return Err(eyre::eyre!(
				"max_datagram_size ({}) is too small for header overhead ({} first / {} subsequent) — cannot fragment",
				max_datagram_size,
				first_frag_header_overhead,
				subsequent_frag_header_overhead,
			));
		}

		tracing::debug!(
			target: "udp",
			"Fragmentation params: payload={}, first_frag_overhead={}, subsequent_frag_overhead={}, max_datagram={}, first_frag_max={}, subsequent_frag_max={}",
			payload_len,
			first_frag_header_overhead,
			subsequent_frag_header_overhead,
			max_datagram_size,
			first_frag_max_payload,
			subsequent_frag_max_payload,
		);

		// First fragment can hold first_frag_max_payload bytes
		// Each subsequent fragment can hold subsequent_frag_max_payload bytes
		let mut remaining_payload = payload_len;
		let fragment_count = if remaining_payload <= first_frag_max_payload {
			1
		} else {
			remaining_payload -= first_frag_max_payload;
			1 + remaining_payload.div_ceil(subsequent_frag_max_payload)
		};
		if fragment_count > MAX_FRAGMENTS as usize {
			return Err(eyre::eyre!(
				"Packet too large for fragmentation, exceeds maximum fragment count"
			));
		}

		// Assign a packet ID for all fragments in this packet.
		// `try_from` guards against future changes that might weaken the
		// bounds check above from silently truncating the fragment count.
		let pkt_id = self.next_pkt_id.fetch_add(1, Ordering::Relaxed);
		let frag_total =
			u8::try_from(fragment_count).map_err(|_| eyre::eyre!("Fragment count {} exceeds u8 range", fragment_count))?;

		let mut offset = 0;
		for frag_id in 0..fragment_count {
			let max_frag_payload = if frag_id == 0 {
				first_frag_max_payload
			} else {
				subsequent_frag_max_payload
			};

			let remaining = payload_len - offset;
			let fragment_size = remaining.min(max_frag_payload);
			let end = offset + fragment_size;

			let fragment_payload = packet.payload.slice(offset..end);

			// Allocate one buffer sized for header + payload so we can append
			// without reallocation or an intermediate concat.
			let header_overhead = if frag_id == 0 {
				first_frag_header_overhead
			} else {
				subsequent_frag_header_overhead
			};
			let mut buf = BytesMut::with_capacity(header_overhead + fragment_payload.len());

			HeaderCodec.encode(Header::new(CmdType::Packet), &mut buf)?;
			CmdCodec(CmdType::Packet).encode(
				Command::Packet {
					assoc_id: self.assoc_id,
					pkt_id,
					frag_total,
					frag_id: frag_id as u8,
					size: fragment_payload.len() as u16,
				},
				&mut buf,
			)?;

			// Add target address (only in first fragment)
			if frag_id == 0 {
				AddressCodec.encode(packet.target.to_owned().into(), &mut buf)?;
			} else {
				AddressCodec.encode(Address::None, &mut buf)?;
			}

			buf.put_slice(&fragment_payload);
			let combined_payload = buf.freeze();

			// Per-packet diagnostics are kept at `trace`/`debug` — emitting at
			// `info` for every fragment on a busy UDP path was expensive
			// (string formatting + I/O cost per packet), and the size is
			// recoverable from the warn-level error if it ever overflows.
			let datagram_size = combined_payload.len();
			let max_allowed = self.connection.max_datagram_size().unwrap_or(1200);
			if datagram_size > max_allowed {
				tracing::warn!(
					target: "udp",
					"Fragment too large: {} bytes > {} bytes max (frag {}/{})",
					datagram_size, max_allowed, frag_id + 1, frag_total,
				);
			} else {
				tracing::debug!(
					target: "udp",
					"Sending fragment {}/{}: {} bytes",
					frag_id + 1, frag_total, datagram_size,
				);
			}

			self.connection
				.send_datagram(combined_payload)
				.map_err(|e| eyre::eyre!("Failed to send fragment: {}", e))?;

			offset = end;
		}

		Ok(())
	}

	/// Process an incoming packet fragment
	#[allow(clippy::too_many_arguments)]
	pub async fn process_fragment(
		&self,
		assoc_id: u16,
		pkt_id: u16,
		frag_total: u8,
		frag_id: u8,
		payload: Bytes,
		source: Option<TargetAddr>,
		target: TargetAddr,
	) -> Option<UdpPacket> {
		// Add fragment to reassembly buffer and check if packet is complete
		self.fragment_buffer
			.add_fragment(
				FragmentInfo {
					assoc_id,
					pkt_id,
					frag_total,
					frag_id,
					source,
					target,
				},
				payload,
			)
			.await
	}

	/// Receive a complete packet from remote server
	/// This will forward the packet to the local receive channel
	pub async fn receive_packet(&self, packet: UdpPacket) -> eyre::Result<()> {
		self.receive_tx
			.send(packet)
			.await
			.map_err(|e| eyre::eyre!("Failed to send packet to receive channel: {:?}", e))
	}

	pub async fn collect_garbage(&self) {
		self.fragment_buffer.cleanup_expired().await;
	}

	/// Number of incomplete fragment groups this session is tracking.
	///
	/// Test-only: how many groups a peer can pin is a resource-exhaustion
	/// property, and the tests assert it directly rather than through logs.
	#[cfg(test)]
	pub(crate) async fn incomplete_group_count(&self) -> usize {
		self.fragment_buffer.incomplete_group_count().await
	}

	/// As [`incomplete_group_count`](Self::incomplete_group_count), but without
	/// running eviction first — lets a test observe what the periodic GC alone
	/// achieved. Test-only.
	#[cfg(test)]
	pub(crate) fn incomplete_group_count_without_cleanup(&self) -> usize {
		self.fragment_buffer.incomplete_group_count_without_cleanup()
	}

	pub async fn close(&mut self) -> Result<(), crate::Error> {
		self.connection.drop_udp(self.assoc_id).await
	}
}

/// Relay one locally-sourced packet to the peer, reporting whether the
/// association can keep running.
///
/// A send failure means this association's streams no longer reach the peer:
/// the connection it was created on may have been replaced by a reconnect (the
/// bridge holds the snapshot taken when the association was created), the peer
/// may have closed it, or the stream may have been reset. A failure is also how
/// a *stale* snapshot surfaces at all — the `closed()` watcher has not
/// necessarily resolved yet when the supervisor swaps in the replacement, so a
/// send can fail first.
///
/// Continuing after such a failure blackholes every later packet: the bridge
/// keeps pulling from the local socket, the peer receives nothing, and the
/// caller never learns it should rebuild the association on the live
/// connection. Returning [`ControlFlow::Break`] ends the bridge instead, which
/// drops both channel halves, removes the association from `udp_session`, and
/// lets the caller re-create it on the current connection.
///
/// Both backends use this so quinn and quiche cannot drift apart here.
pub(crate) async fn forward_remote_packet<C: QuicConnection>(
	stream: &UdpStream<C>,
	packet: UdpPacket,
	assoc_id: u16,
) -> ControlFlow<()> {
	let payload_len = packet.payload.len();
	match stream.send_packet(packet).await {
		Ok(()) => {
			tracing::info!(target: "tuic_out", "Sent UDP packet to remote ({payload_len} bytes, assoc {assoc_id:#06x})");
			ControlFlow::Continue(())
		}
		Err(e) => {
			tracing::warn!(target: "tuic_out", "Failed to send UDP packet to remote (assoc {assoc_id:#06x}): {e}");
			ControlFlow::Break(())
		}
	}
}

#[cfg(test)]
mod tests {
	use std::{
		net::Ipv4Addr,
		pin::Pin,
		sync::{Arc, Mutex},
		task::{Context as TaskContext, Poll},
	};

	use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
	use wind_quic::{QuicConnection, QuicError, QuicRecvStream, QuicSendStream};

	use super::*;

	/// Test helper to calculate address size according to SPEC.md Section 6.2
	/// (Address Type Registry) and Section 6.3 (Address Type Specifications)
	fn calculate_addr_size(target: &TargetAddr) -> usize {
		match target {
			TargetAddr::IPv4(..) => 1 + 4 + 2,  // Type (1) + IPv4 (4) + Port (2) = 7 bytes
			TargetAddr::IPv6(..) => 1 + 16 + 2, // Type (1) + IPv6 (16) + Port (2) = 19 bytes
			TargetAddr::Domain(domain, _) => 1 + 1 + domain.len() + 2, /* Type (1) + Len (1) +
			                                      * Domain + Port (2) */
		}
	}

	/// SPEC.md Section 8.6: Fragmentation Size Calculations
	#[test]
	fn test_fragment_count_calculation() {
		const MAX_DATAGRAM_SIZE: usize = 1200;
		let addr = TargetAddr::IPv4(Ipv4Addr::new(192, 168, 1, 1), 8080);

		// First fragment has full address
		let first_frag_overhead = 2 + 8 + calculate_addr_size(&addr);
		// Subsequent fragments use Address::None (1 byte)
		let subsequent_frag_overhead = 2 + 8 + 1;

		let first_frag_max = MAX_DATAGRAM_SIZE - first_frag_overhead;
		let subsequent_frag_max = MAX_DATAGRAM_SIZE - subsequent_frag_overhead;

		let calc_frags = |payload_size: usize| -> usize {
			if payload_size <= first_frag_max {
				1
			} else {
				let remaining = payload_size - first_frag_max;
				1 + remaining.div_ceil(subsequent_frag_max)
			}
		};

		let test_cases = vec![
			(1000, 1),                                     // Small payload, 1 fragment
			(first_frag_max, 1),                           // Exactly max size for first fragment, 1 fragment
			(first_frag_max + 1, 2),                       // Just over, 2 fragments
			(first_frag_max + subsequent_frag_max, 2),     // Exactly 2 fragments
			(first_frag_max + subsequent_frag_max + 1, 3), // Just over 2x, 3 fragments
			(10000, calc_frags(10000)),                    // Large payload
		];

		for (payload_size, expected_fragments) in test_cases {
			let fragment_count = calc_frags(payload_size);
			assert_eq!(
				fragment_count, expected_fragments,
				"Payload {} bytes should require {} fragments",
				payload_size, expected_fragments
			);
		}
	}

	/// SPEC.md Section 8.7: Implementation Constraints - Fragment count must
	/// not exceed 255
	#[test]
	fn test_max_fragment_limit() {
		const MAX_DATAGRAM_SIZE: usize = 1200;

		let addr = TargetAddr::IPv4(Ipv4Addr::new(192, 168, 1, 1), 8080);

		// First fragment has full address
		let first_frag_overhead = 2 + 8 + calculate_addr_size(&addr);
		// Subsequent fragments use Address::None (1 byte)
		let subsequent_frag_overhead = 2 + 8 + 1;

		let first_frag_max = MAX_DATAGRAM_SIZE - first_frag_overhead;
		let subsequent_frag_max = MAX_DATAGRAM_SIZE - subsequent_frag_overhead;

		// Maximum allowable payload with 255 fragments
		// First fragment + 254 subsequent fragments
		let max_payload = first_frag_max + (subsequent_frag_max * (MAX_FRAGMENTS as usize - 1));

		let remaining = max_payload - first_frag_max;
		let fragment_count = 1 + remaining.div_ceil(subsequent_frag_max);

		assert_eq!(fragment_count, 255, "Should be able to send 255 fragments");
		assert!(fragment_count <= MAX_FRAGMENTS as usize, "Fragment count must not exceed 255");

		// One byte over should exceed limit
		let oversized_payload = max_payload + 1;
		let oversized_remaining = oversized_payload - first_frag_max;
		let oversized_count = 1 + oversized_remaining.div_ceil(subsequent_frag_max);
		assert!(
			oversized_count > MAX_FRAGMENTS as usize,
			"Oversized payload should exceed fragment limit"
		);
	}

	/// Verify saturating_sub prevents underflow as mentioned in SPEC.md Section
	/// 8.7
	#[test]
	fn test_saturating_sub_prevents_underflow() {
		let small_mtu: usize = 10;
		let large_overhead: usize = 100;

		// Using saturating_sub should give 0 instead of underflowing
		let result = small_mtu.saturating_sub(large_overhead);
		assert_eq!(result, 0, "saturating_sub should prevent underflow");

		// Normal subtraction would panic in debug mode or wrap in release
		// This test verifies the implementation advice from SPEC.md Section 8.7
	}

	// -----------------------------------------------------------------------
	// Send-path transport selection (F18): a peer that does not advertise
	// DATAGRAM support must receive `Packet` commands on unidirectional
	// streams, because `send_datagram` is rejected by the transport and the
	// frame is discarded — silently losing every UDP reply.
	// -----------------------------------------------------------------------

	/// A [`QuicConnection`] double that records the two UDP send paths so a
	/// test can assert which one a `Packet` took.
	#[derive(Clone)]
	struct RecordingConn {
		max_datagram: Option<usize>,
		/// When set, `send_datagram` fails the way a replaced or closed
		/// connection does — a caller must stop feeding the association.
		fail_datagrams: bool,
		datagrams: Arc<Mutex<Vec<Bytes>>>,
		uni_streams: Arc<Mutex<Vec<Vec<u8>>>>,
	}

	impl RecordingConn {
		fn new(max_datagram: Option<usize>) -> Self {
			Self {
				max_datagram,
				fail_datagrams: false,
				datagrams: Arc::new(Mutex::new(Vec::new())),
				uni_streams: Arc::new(Mutex::new(Vec::new())),
			}
		}

		/// A connection whose datagram path always fails, as it does once the
		/// QUIC connection behind an association is gone.
		fn with_failing_datagrams(max_datagram: Option<usize>) -> Self {
			Self {
				fail_datagrams: true,
				..Self::new(max_datagram)
			}
		}

		fn datagrams(&self) -> Vec<Bytes> {
			self.datagrams.lock().unwrap().clone()
		}

		fn uni_streams(&self) -> Vec<Vec<u8>> {
			self.uni_streams.lock().unwrap().clone()
		}
	}

	/// Records everything written to one locally-opened unidirectional stream.
	/// `send_udp` writes the header+command+address and (in stream relay mode)
	/// the payload as separate `write_all` calls before `finish`, so the buffer
	/// is accumulated locally and published on `finish`.
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

	impl QuicConnection for RecordingConn {
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
			if self.fail_datagrams {
				return Err(QuicError::Other("connection closed".into()));
			}
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
	}

	const TEST_ASSOC_ID: u16 = 0x1234;

	/// Build a `UdpStream` over `conn` with `mode` and a live receive channel.
	fn recording_stream(conn: &RecordingConn, mode: UdpRelayMode) -> UdpStream<RecordingConn> {
		let (tx, _rx) = crossfire::mpmc::bounded_async::<UdpPacket>(8);
		UdpStream::new(conn.clone(), TEST_ASSOC_ID, tx).with_relay_mode(mode)
	}

	/// F17 regression on the wiring layer: `with_gc_lifetime` must actually
	/// reach the reassembly buffer's per-group lifetime, otherwise
	/// `collect_garbage` is a no-op and incomplete groups are pinned for the
	/// association's lifetime.
	#[tokio::test]
	async fn collect_garbage_evicts_groups_past_the_configured_lifetime() {
		let conn = RecordingConn::new(Some(1200));
		let (tx, _rx) = crossfire::mpmc::bounded_async::<UdpPacket>(8);
		let lifetime = Duration::from_millis(100);
		let stream = UdpStream::new(conn, TEST_ASSOC_ID, tx).with_gc_lifetime(lifetime);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);

		let incomplete = stream
			.process_fragment(TEST_ASSOC_ID, 7, 3, 0, Bytes::from_static(b"partial"), None, target)
			.await;
		assert!(incomplete.is_none(), "one fragment of three must not complete");
		assert_eq!(stream.incomplete_group_count().await, 1, "the incomplete group is tracked");

		tokio::time::sleep(lifetime * 2).await;
		stream.collect_garbage().await;

		assert_eq!(
			stream.incomplete_group_count().await,
			0,
			"collect_garbage must evict a group past its configured lifetime"
		);
	}

	/// Decode a wire `Packet` frame as produced by `send_udp` /
	/// `send_fragmented_packet` into its command, target, and trailing payload.
	fn decode_packet_frame(frame: &[u8]) -> (Command, TargetAddr, Bytes) {
		let mut buf = frame;
		let header = crate::proto::decode_header(&mut buf, "test").unwrap();
		assert_eq!(header.command, CmdType::Packet);
		let cmd = crate::proto::decode_command(CmdType::Packet, &mut buf, "test").unwrap();
		let addr = crate::proto::decode_address(&mut buf, "test").unwrap();
		let target = crate::proto::address_to_target(addr).unwrap();
		(cmd, target, Bytes::copy_from_slice(buf))
	}

	fn probe_packet(target: &TargetAddr, payload: &'static [u8]) -> UdpPacket {
		UdpPacket {
			source: None,
			target: target.clone(),
			payload: Bytes::from_static(payload),
		}
	}

	/// Control: a peer that advertises DATAGRAM support keeps the datagram path
	/// (one frame per packet, address + payload inline).
	#[tokio::test]
	async fn datagram_relay_used_when_peer_advertises() {
		let conn = RecordingConn::new(Some(1200));
		let stream = recording_stream(&conn, UdpRelayMode::Native);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);
		let payload = b"datagram-probe";

		stream
			.send_packet(probe_packet(&target, payload))
			.await
			.expect("datagram send");

		let datagrams = conn.datagrams();
		assert_eq!(
			datagrams.len(),
			1,
			"peer advertises DATAGRAM support: one datagram per packet"
		);
		assert!(conn.uni_streams().is_empty(), "datagram path must not open uni streams");

		let (cmd, decoded_target, decoded_payload) = decode_packet_frame(&datagrams[0]);
		assert_eq!(decoded_target, target);
		assert_eq!(
			cmd,
			Command::Packet {
				assoc_id: TEST_ASSOC_ID,
				pkt_id: 0,
				frag_total: 1,
				frag_id: 0,
				size: payload.len() as u16,
			}
		);
		assert_eq!(decoded_payload, &payload[..]);
	}

	/// F18 regression: a peer that does not advertise DATAGRAM support (e.g. a
	/// quiche client configured `udp_relay_mode = "quic"`) must receive every
	/// reply on a unidirectional stream. Before the fix the packet took the
	/// datagram branch, quiche rejected it (`InvalidState`), and the reply was
	/// dropped — a silent, total loss of server-to-client UDP.
	#[tokio::test]
	async fn peer_without_datagram_support_falls_back_to_uni_stream() {
		let conn = RecordingConn::new(None);
		let stream = recording_stream(&conn, UdpRelayMode::Native);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);

		// A single-datagram-sized payload and one that would have to be
		// fragmented: stream relay mode never fragments (streams are
		// flow-controlled), so each packet is exactly one uni stream.
		let small = b"small";
		let large = vec![0xABu8; 4 * 1024];
		stream
			.send_packet(probe_packet(&target, small))
			.await
			.expect("stream send (small)");
		stream
			.send_packet(UdpPacket {
				source: None,
				target: target.clone(),
				payload: Bytes::from(large.clone()),
			})
			.await
			.expect("stream send (large)");

		assert!(
			conn.datagrams().is_empty(),
			"a peer without DATAGRAM support must never be sent a datagram"
		);
		let streams = conn.uni_streams();
		assert_eq!(streams.len(), 2, "one uni stream per reply, no fragmentation");

		let (cmd, decoded_target, decoded_payload) = decode_packet_frame(&streams[0]);
		assert_eq!(decoded_target, target);
		assert_eq!(
			cmd,
			Command::Packet {
				assoc_id: TEST_ASSOC_ID,
				pkt_id: 0,
				frag_total: 1,
				frag_id: 0,
				size: small.len() as u16,
			}
		);
		assert_eq!(decoded_payload, &small[..]);

		let (cmd, decoded_target, decoded_payload) = decode_packet_frame(&streams[1]);
		assert_eq!(decoded_target, target);
		assert_eq!(
			cmd,
			Command::Packet {
				assoc_id: TEST_ASSOC_ID,
				pkt_id: 1,
				frag_total: 1,
				frag_id: 0,
				size: large.len() as u16,
			}
		);
		assert_eq!(decoded_payload, &large[..]);
	}

	/// An explicit `Quic` relay mode keeps using uni streams even when the peer
	/// does advertise DATAGRAM support.
	#[tokio::test]
	async fn explicit_quic_relay_mode_uses_uni_streams() {
		let conn = RecordingConn::new(Some(1200));
		let stream = recording_stream(&conn, UdpRelayMode::Quic);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);
		let payload = b"quic-relay-probe";

		stream.send_packet(probe_packet(&target, payload)).await.expect("stream send");

		assert!(conn.datagrams().is_empty(), "Quic relay mode must not send datagrams");
		let streams = conn.uni_streams();
		assert_eq!(streams.len(), 1);
		let (_, decoded_target, decoded_payload) = decode_packet_frame(&streams[0]);
		assert_eq!(decoded_target, target);
		assert_eq!(decoded_payload, &payload[..]);
	}

	// -----------------------------------------------------------------------
	// Oversize guard (W36/W42): the `size` field of a `Packet` command is a
	// `u16`. A larger payload must be refused with the numeric overflow named,
	// rather than truncated or sent as a frame whose `size` disagrees with the
	// bytes that follow it.
	// -----------------------------------------------------------------------

	/// Regression (W36): the guard reports a `NumericOverflow` naming the
	/// offending field and value, and nothing reaches the wire.
	#[tokio::test]
	async fn oversized_payload_is_rejected_as_a_numeric_overflow() {
		let conn = RecordingConn::new(Some(1200));
		let stream = recording_stream(&conn, UdpRelayMode::Native);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);
		let payload_len = u16::MAX as usize + 1;

		let err = stream
			.send_packet(UdpPacket {
				source: None,
				target,
				payload: Bytes::from(vec![0xABu8; payload_len]),
			})
			.await
			.expect_err("a payload that does not fit `size` must be refused");

		match err.downcast_ref::<crate::proto::ProtoError>() {
			Some(crate::proto::ProtoError::NumericOverflow { field, num, .. }) => {
				assert_eq!(field, "UDP payload size");
				assert_eq!(num, &payload_len.to_string());
			}
			other => panic!("expected NumericOverflow, got {other:?}"),
		}
		assert!(conn.datagrams().is_empty(), "a refused packet must not be sent as a datagram");
		assert!(
			conn.uni_streams().is_empty(),
			"a refused packet must not be sent on a uni stream"
		);
	}

	// -----------------------------------------------------------------------
	// Association bridge send path (W11): a packet the peer can no longer
	// receive must stop the bridge, not be logged and skipped. The caller only
	// learns the association is dead when the bridge ends and drops the
	// channels, so continuing would blackhole every later packet on a
	// connection that a reconnect has already replaced.
	// -----------------------------------------------------------------------

	/// Control: a reachable peer keeps the bridge running.
	#[tokio::test]
	async fn forwarded_packet_with_a_live_connection_continues_the_bridge() {
		let conn = RecordingConn::new(Some(1200));
		let stream = recording_stream(&conn, UdpRelayMode::Native);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);

		let outcome = forward_remote_packet(&stream, probe_packet(&target, b"relayed"), TEST_ASSOC_ID).await;

		assert!(
			outcome.is_continue(),
			"a packet the transport accepted must leave the association alive"
		);
		assert_eq!(conn.datagrams().len(), 1, "the packet reached the wire");
	}

	/// A send failure on the association's snapshot connection (reconnect, peer
	/// close, reset stream) must end the bridge so the caller can rebuild the
	/// association on the live connection.
	#[tokio::test]
	async fn failed_remote_send_breaks_the_bridge_instead_of_blackholing() {
		let conn = RecordingConn::with_failing_datagrams(Some(1200));
		let stream = recording_stream(&conn, UdpRelayMode::Native);
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 5353);

		let outcome = forward_remote_packet(&stream, probe_packet(&target, b"dropped"), TEST_ASSOC_ID).await;

		assert!(
			outcome.is_break(),
			"a send failure must end the association, not be warned about once per packet"
		);
		assert!(
			conn.datagrams().is_empty(),
			"the failing connection records nothing on the wire"
		);
	}
}
