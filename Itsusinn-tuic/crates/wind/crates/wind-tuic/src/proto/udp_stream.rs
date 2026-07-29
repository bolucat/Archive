use std::sync::atomic::{AtomicU16, Ordering};

use bytes::{BufMut, Bytes, BytesMut};
use crossfire::{MAsyncTx, mpmc};
use tokio_util::codec::Encoder;
use wind_core::{types::TargetAddr, udp::UdpPacket};

type UdpPacketTx = MAsyncTx<mpmc::Array<UdpPacket>>;

use tuic_core::udp::{FragmentInfo, FragmentReassemblyBuffer, MAX_FRAGMENTS};
use wind_quic::QuicConnection;

use crate::proto::{Address, AddressCodec, ClientProtoExt as _, CmdCodec, CmdType, Command, Header, HeaderCodec};

/// A TUIC UDP association over any [`QuicConnection`].
///
/// The fragment **reassembly** state machine lives in
/// [`tuic_core::udp::FragmentReassemblyBuffer`]; this type owns the
/// connection-coupled send path (datagram sizing, fragmentation, dispatch) and
/// bridges reassembled packets to a receive channel.
pub struct UdpStream<C: QuicConnection> {
	connection: C,
	assoc_id: u16,
	receive_tx: UdpPacketTx,
	next_pkt_id: AtomicU16,
	// Fragment reassembly state machine (backend-agnostic, from tuic-core).
	fragment_buffer: FragmentReassemblyBuffer,
}

impl<C: QuicConnection> UdpStream<C> {
	pub fn new(connection: C, assoc_id: u16, receive_tx: UdpPacketTx) -> Self {
		Self {
			connection,
			assoc_id,
			receive_tx,
			next_pkt_id: AtomicU16::new(0),
			fragment_buffer: FragmentReassemblyBuffer::new(),
		}
	}

	pub async fn send_packet(&self, packet: UdpPacket) -> eyre::Result<()> {
		let payload_len = packet.payload.len();

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

	pub async fn close(&mut self) -> Result<(), crate::Error> {
		self.connection.drop_udp(self.assoc_id).await
	}
}

#[cfg(test)]
mod tests {
	use std::net::Ipv4Addr;

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
}
