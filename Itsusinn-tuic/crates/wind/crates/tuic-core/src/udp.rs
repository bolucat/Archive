//! UDP fragment reassembly state machine for the TUIC native (datagram) UDP
//! relay mode.
//!
//! This is the receive-side counterpart to the per-backend send path. It is
//! fully backend-agnostic: it consumes already-decoded fragment metadata plus a
//! payload [`Bytes`] and yields a reassembled [`UdpPacket`] once every fragment
//! of a `(assoc_id, pkt_id)` group has arrived. The buffer is hardened against
//! attacker-controlled fragment fields (see
//! [`FragmentReassemblyBuffer::add_fragment`]).

use std::{
	sync::{
		Arc, OnceLock,
		atomic::{AtomicU64, Ordering},
	},
	time::{Duration, Instant},
};

use arc_swap::{ArcSwap, ArcSwapOption};
use bytes::{BufMut, Bytes, BytesMut};
use moka::future::Cache;
use wind_core::{types::TargetAddr, udp::UdpPacket};

/// Maximum number of fragments allowed for a single packet.
pub const MAX_FRAGMENTS: u8 = 255;
/// Timeout (ms) after which incomplete fragment groups are evicted.
const FRAGMENT_TIMEOUT_MS: u64 = 30000;

static INIT_TIME: OnceLock<Instant> = OnceLock::new();

fn init_time() -> &'static Instant {
	INIT_TIME.get_or_init(Instant::now)
}

/// Fragment information for reassembly.
pub struct FragmentInfo {
	pub assoc_id: u16,
	pub pkt_id: u16,
	pub frag_total: u8,
	pub frag_id: u8,
	pub source: Option<TargetAddr>,
	pub target: TargetAddr,
}

/// Structure to track fragments of a packet for reassembly.
struct FragmentMetadata {
	frag_total: u8,
	fragments: Cache<u8, Bytes>,
	last_updated: AtomicU64,
	source: ArcSwapOption<TargetAddr>,
	target: ArcSwap<TargetAddr>,
}

/// Buffer for reassembling fragmented packets.
pub struct FragmentReassemblyBuffer {
	fragments: Cache<(u16, u16), Arc<FragmentMetadata>>, // (assoc_id, pkt_id) -> fragment metadata
}

impl Default for FragmentReassemblyBuffer {
	fn default() -> Self {
		Self::new()
	}
}

impl FragmentReassemblyBuffer {
	/// Create a new fragment reassembly buffer.
	pub fn new() -> Self {
		Self {
			fragments: Cache::new(1000),
		}
	}

	/// Add a fragment to the buffer.
	///
	/// `frag_total` and `frag_id` arrive straight from the wire and are
	/// fully attacker-controlled. We validate them up front:
	///
	/// * `frag_total == 0` — meaningless ("packet split into zero pieces"). The
	///   old code would insert a zero-capacity sub-cache and trip the
	///   "entry_count == frag_total" check immediately with an empty payload
	///   set, which `reassemble_packet` then turned into a zero-byte packet.
	///   Reject up front.
	/// * `frag_id >= frag_total` — out of range; would poison the
	///   per-(assoc_id, pkt_id) sub-cache by storing under a key the reassembly
	///   walk never reads, permanently blocking the genuine packet from
	///   completing. Reject.
	/// * `frag_total` disagreement with the first fragment we've seen for this
	///   (assoc_id, pkt_id) — also rejected, otherwise an attacker could
	///   over-declare `frag_total = 255` on a forged packet and pin a 255-entry
	///   sub-cache against a victim's stream.
	pub async fn add_fragment(&self, info: FragmentInfo, payload: Bytes) -> Option<UdpPacket> {
		let FragmentInfo {
			assoc_id,
			pkt_id,
			frag_total,
			frag_id,
			source,
			target,
		} = info;

		if frag_total == 0 || frag_id >= frag_total {
			tracing::warn!(
				target: "udp",
				assoc_id,
				pkt_id,
				frag_total,
				frag_id,
				"Dropping fragment with invalid frag fields (frag_total == 0 or frag_id >= frag_total)",
			);
			return None;
		}

		let key = (assoc_id, pkt_id);

		// Placeholder address used for non-first fragments.
		let is_placeholder_addr = matches!(target, TargetAddr::IPv4(ip, 0) if ip.is_unspecified());
		// Wrap in Arc once so both the `or_insert_with` future and the
		// post-lookup `store` path can share a reference without cloning the
		// underlying `TargetAddr` (which for `Domain` includes a `String`).
		let target_arc = Arc::new(target);
		let source_arc = source.map(Arc::new);

		let is_complete = {
			let meta = self
				.fragments
				.entry(key)
				.or_insert_with({
					let target_arc = target_arc.clone();
					let source_arc = source_arc.clone();
					async move {
						Arc::new(FragmentMetadata {
							frag_total,
							fragments: Cache::new(frag_total.into()),
							last_updated: AtomicU64::new(init_time().elapsed().as_secs()),
							source: ArcSwapOption::new(source_arc),
							target: ArcSwap::new(target_arc),
						})
					}
				})
				.await;

			// Reject fragments whose `frag_total` disagrees with the packet
			// that's already being assembled. Without this check, a forged
			// packet with a different frag_total wedges (or grows) the
			// per-(assoc_id, pkt_id) sub-cache.
			if meta.value().frag_total != frag_total {
				tracing::warn!(
					target: "udp",
					assoc_id,
					pkt_id,
					expected_frag_total = meta.value().frag_total,
					got_frag_total = frag_total,
					frag_id,
					"Dropping fragment with mismatched frag_total for an existing reassembly",
				);
				return None;
			}

			// If this is the first fragment (frag_id == 0) and it has a real address,
			// update the target address in case we received other fragments first with
			// placeholder addresses
			if frag_id == 0 && !is_placeholder_addr {
				meta.value().target.store(target_arc);
			}

			meta.value()
				.last_updated
				.store(init_time().elapsed().as_secs(), Ordering::Relaxed);

			meta.value().fragments.insert(frag_id, payload).await;

			meta.value().fragments.run_pending_tasks().await;

			meta.value().fragments.entry_count() == meta.value().frag_total as u64
		};

		if is_complete {
			return self.reassemble_packet(key).await;
		}

		None
	}

	/// Clean up expired fragments.
	///
	/// `invalidate_entries_if` only registers a predicate; the actual eviction
	/// happens during housekeeping. We drive it via `run_pending_tasks` so the
	/// cleanup is observable before this call returns.
	pub async fn cleanup_expired(&self) {
		if let Err(e) = self.fragments.invalidate_entries_if(move |_, meta| {
			init_time().elapsed() - Duration::from_secs(meta.last_updated.load(Ordering::Relaxed))
				>= Duration::from_millis(FRAGMENT_TIMEOUT_MS)
		}) {
			tracing::warn!(target: "udp", "Failed to register fragment cleanup predicate: {:?}", e);
			return;
		}
		self.fragments.run_pending_tasks().await;
	}

	/// Reassemble a complete packet from fragments.
	async fn reassemble_packet(&self, key: (u16, u16)) -> Option<UdpPacket> {
		if let Some(meta) = self.fragments.remove(&key).await {
			let mut total_size = 0;
			for i in 0..meta.frag_total {
				let fragment = meta.fragments.get(&i).await?;
				total_size += fragment.len();
			}
			let mut buffer = BytesMut::with_capacity(total_size);

			// Combine fragments in order.
			for i in 0..meta.frag_total {
				let fragment = meta.fragments.get(&i).await?;
				buffer.put_slice(&fragment);
			}

			let payload = buffer.freeze();
			match Arc::try_unwrap(meta) {
				Ok(m) => {
					let source = m
						.source
						.into_inner()
						.map(|arc| Arc::try_unwrap(arc).unwrap_or_else(|a| (*a).clone()));
					let target = Arc::try_unwrap(m.target.into_inner()).unwrap_or_else(|a| (*a).clone());

					Some(UdpPacket { source, target, payload })
				}
				Err(arc) => {
					let source = arc.source.load().as_ref().map(|a| (**a).clone());
					let target = (**arc.target.load()).clone();

					Some(UdpPacket { source, target, payload })
				}
			}
		} else {
			None
		}
	}
}

#[cfg(test)]
mod tests {
	use std::net::Ipv4Addr;

	use super::*;

	#[test_log::test(tokio::test)]
	async fn test_fragment_reassembly_single_fragment() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);
		let payload = Bytes::from("test payload");

		let result = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 100,
					frag_total: 1,
					frag_id: 0,
					source: None,
					target: target.clone(),
				},
				payload.clone(),
			)
			.await;

		assert!(result.is_some(), "Single fragment should complete immediately");
		let packet = result.unwrap();
		assert_eq!(packet.payload, payload);
	}

	#[test_log::test(tokio::test)]
	async fn test_fragment_reassembly_multiple_fragments() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		let frag1 = Bytes::from("Hello ");
		let frag2 = Bytes::from("World");

		let result1 = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 200,
					frag_total: 2,
					frag_id: 0,
					source: None,
					target: target.clone(),
				},
				frag1.clone(),
			)
			.await;
		assert!(result1.is_none(), "First fragment should not complete packet");

		let result2 = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 200,
					frag_total: 2,
					frag_id: 1,
					source: None,
					target: target.clone(),
				},
				frag2.clone(),
			)
			.await;
		assert!(result2.is_some(), "Second fragment should complete packet");

		let packet = result2.unwrap();
		assert_eq!(packet.payload, Bytes::from("Hello World"));
	}

	#[test_log::test(tokio::test)]
	async fn test_fragment_reassembly_out_of_order() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		let frag0 = Bytes::from("A");
		let frag1 = Bytes::from("B");
		let frag2 = Bytes::from("C");

		assert!(
			buffer
				.add_fragment(
					FragmentInfo {
						assoc_id: 1,
						pkt_id: 300,
						frag_total: 3,
						frag_id: 2,
						source: None,
						target: target.clone(),
					},
					frag2.clone(),
				)
				.await
				.is_none()
		);
		assert!(
			buffer
				.add_fragment(
					FragmentInfo {
						assoc_id: 1,
						pkt_id: 300,
						frag_total: 3,
						frag_id: 0,
						source: None,
						target: target.clone(),
					},
					frag0.clone(),
				)
				.await
				.is_none()
		);

		let result = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 300,
					frag_total: 3,
					frag_id: 1,
					source: None,
					target: target.clone(),
				},
				frag1.clone(),
			)
			.await;
		assert!(result.is_some(), "All fragments received, should complete");

		let packet = result.unwrap();
		assert_eq!(packet.payload, Bytes::from("ABC"));
	}

	#[test_log::test(tokio::test)]
	async fn test_multiple_simultaneous_fragmentations() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 100,
					frag_total: 2,
					frag_id: 0,
					source: None,
					target: target.clone(),
				},
				Bytes::from("A1"),
			)
			.await;
		buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 101,
					frag_total: 2,
					frag_id: 0,
					source: None,
					target: target.clone(),
				},
				Bytes::from("B1"),
			)
			.await;

		let result1 = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 100,
					frag_total: 2,
					frag_id: 1,
					source: None,
					target: target.clone(),
				},
				Bytes::from("A2"),
			)
			.await;
		assert!(result1.is_some());
		assert_eq!(result1.unwrap().payload, Bytes::from("A1A2"));

		let result2 = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 101,
					frag_total: 2,
					frag_id: 1,
					source: None,
					target: target.clone(),
				},
				Bytes::from("B2"),
			)
			.await;
		assert!(result2.is_some());
		assert_eq!(result2.unwrap().payload, Bytes::from("B1B2"));
	}

	#[test_log::test(tokio::test)]
	async fn test_fragment_cleanup() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 400,
					frag_total: 2,
					frag_id: 0,
					source: None,
					target: target.clone(),
				},
				Bytes::from("test"),
			)
			.await;

		buffer.fragments.run_pending_tasks().await;
		assert_eq!(buffer.fragments.entry_count(), 1, "Should have one incomplete packet");

		buffer.fragments.remove(&(1, 400)).await;
		buffer.fragments.run_pending_tasks().await;

		assert_eq!(buffer.fragments.entry_count(), 0, "Fragments should be cleaned up");
	}

	/// `frag_total == 0` and `frag_id >= frag_total` are both forbidden by
	/// the spec, but are attacker-controlled on the wire. The buffer must
	/// drop such fragments instead of producing a zero-byte "reassembled"
	/// packet or poisoning the per-pkt sub-cache.
	#[test_log::test(tokio::test)]
	async fn test_add_fragment_rejects_zero_total() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		let res = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 1,
					frag_total: 0,
					frag_id: 0,
					source: None,
					target,
				},
				Bytes::from_static(b"x"),
			)
			.await;
		assert!(res.is_none(), "frag_total=0 must be dropped");
		buffer.fragments.run_pending_tasks().await;
		assert_eq!(buffer.fragments.entry_count(), 0, "frag_total=0 must not insert an entry");
	}

	#[test_log::test(tokio::test)]
	async fn test_add_fragment_rejects_out_of_range_frag_id() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		let res = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 1,
					frag_total: 3,
					frag_id: 7, // > frag_total
					source: None,
					target,
				},
				Bytes::from_static(b"x"),
			)
			.await;
		assert!(res.is_none(), "frag_id >= frag_total must be dropped");
		buffer.fragments.run_pending_tasks().await;
		assert_eq!(
			buffer.fragments.entry_count(),
			0,
			"out-of-range frag_id must not insert an entry"
		);
	}

	/// Once a reassembly slot is open with `frag_total = N`, fragments
	/// claiming a different `frag_total` for the same (assoc_id, pkt_id)
	/// must be rejected, otherwise an attacker can over-declare to grow
	/// the per-packet sub-cache or block completion entirely.
	#[test_log::test(tokio::test)]
	async fn test_add_fragment_rejects_mismatched_frag_total() {
		let buffer = FragmentReassemblyBuffer::new();
		let target = TargetAddr::IPv4(Ipv4Addr::new(127, 0, 0, 1), 8080);

		// First legitimate fragment opens the slot at frag_total=2.
		let _ = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 1,
					frag_total: 2,
					frag_id: 0,
					source: None,
					target: target.clone(),
				},
				Bytes::from_static(b"AA"),
			)
			.await;

		// Forged fragment claiming frag_total=255 — must be dropped.
		let res = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 1,
					frag_total: 255,
					frag_id: 200,
					source: None,
					target: target.clone(),
				},
				Bytes::from_static(b"X"),
			)
			.await;
		assert!(res.is_none(), "mismatched frag_total must be dropped");

		// The legitimate completion path still works after the forged drop.
		let completed = buffer
			.add_fragment(
				FragmentInfo {
					assoc_id: 1,
					pkt_id: 1,
					frag_total: 2,
					frag_id: 1,
					source: None,
					target,
				},
				Bytes::from_static(b"BB"),
			)
			.await;
		let packet = completed.expect("legitimate completion must still succeed");
		assert_eq!(&packet.payload[..], b"AABB");
	}
}
