//! End-to-end tests for the direct UDP relay's dual-stack behaviour.
//!
//! These drive the public `OutboundAction::handle_udp` entrypoint exactly as
//! the TUIC/SOCKS inbounds do — feeding `UdpPacket`s in through the association
//! channel and reading the replies back out — against real loopback UDP echo
//! servers.
//!
//! They are the regression guard for the fix that made the relay socket
//! dual-stack. Previously it was hard-bound to `0.0.0.0:0` (IPv4 only), so an
//! IPv6 target failed `send_to` with `EAFNOSUPPORT` ("Address family not
//! supported by protocol") and the reply never arrived — i.e. before the fix
//! `ipv6_target_roundtrips` would time out.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use bytes::Bytes;
use tokio::{net::UdpSocket, sync::mpsc, time::timeout};
use wind_base::direct::{DirectOutbound, DirectOutboundOpts};
use wind_core::{
	OutboundAction, StackPrefer, SystemResolver,
	types::TargetAddr,
	udp::{UdpPacket, UdpStream},
};

/// Generous upper bound — a loopback round trip is sub-millisecond; this only
/// exists so a dropped packet fails loudly instead of hanging the suite.
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// A running relay association: push packets toward targets via `to_relay`,
/// read replies from targets via `from_relay`.
struct RelayHarness {
	to_relay: mpsc::Sender<UdpPacket>,
	from_relay: mpsc::Receiver<UdpPacket>,
	// Held only to keep the association task alive for the test's duration; it
	// ends when `to_relay` drops (closing the relay's `rx`) or the test's
	// runtime shuts down.
	_task: tokio::task::JoinHandle<()>,
}

/// Spawn a `DirectOutbound` UDP relay and return the client-side channel ends.
///
/// Two crossed channels model the association, mirroring how the dispatcher
/// wires an inbound to an outbound:
///   client → relay  (packets to send to targets)
///   relay  → client (packets received back from targets)
fn spawn_relay() -> RelayHarness {
	let resolver = Arc::new(SystemResolver::new(StackPrefer::V4first));
	let outbound = DirectOutbound::new(
		DirectOutboundOpts {
			bind_ipv4: None,
			bind_ipv6: None,
			bind_device: None,
			stream_timeout: Duration::ZERO,
			tcp_keepalive: Some(wind_core::tcp::TcpKeepalive::default()),
		},
		resolver,
	);

	let (to_relay, relay_rx) = mpsc::channel::<UdpPacket>(16);
	let (relay_tx, from_relay) = mpsc::channel::<UdpPacket>(16);
	let stream = UdpStream {
		tx: relay_tx,
		rx: relay_rx,
	};

	let task = tokio::spawn(async move {
		let _ = outbound.handle_udp(stream).await;
	});

	RelayHarness {
		to_relay,
		from_relay,
		_task: task,
	}
}

/// Bind a UDP echo server to `bind`, echoing each datagram back to its sender
/// until the socket is dropped. Returns the actual bound address.
async fn spawn_echo_server(bind: &str) -> std::io::Result<SocketAddr> {
	let sock = UdpSocket::bind(bind).await?;
	let addr = sock.local_addr()?;
	tokio::spawn(async move {
		let mut buf = vec![0u8; 2048];
		while let Ok((n, from)) = sock.recv_from(&mut buf).await {
			if sock.send_to(&buf[..n], from).await.is_err() {
				break;
			}
		}
	});
	Ok(addr)
}

/// Send `payload` to `target` through the relay and await the single reply.
async fn roundtrip(h: &mut RelayHarness, target: TargetAddr, payload: &'static [u8]) -> UdpPacket {
	h.to_relay
		.send(UdpPacket {
			source: None,
			target,
			payload: Bytes::from_static(payload),
		})
		.await
		.expect("relay accepts the outbound packet");

	timeout(REPLY_TIMEOUT, h.from_relay.recv())
		.await
		.expect("relay produced a reply before the timeout")
		.expect("relay reply channel stayed open")
}

#[tokio::test]
async fn ipv6_target_roundtrips() {
	// On a host without IPv6 loopback the dual-stack relay falls back to an
	// IPv4-only socket and genuinely cannot reach `::1`; skip rather than fail.
	let Ok(echo) = spawn_echo_server("[::1]:0").await else {
		eprintln!("skipping ipv6_target_roundtrips: no IPv6 loopback available");
		return;
	};

	let mut h = spawn_relay();
	let reply = roundtrip(&mut h, TargetAddr::from(echo), b"hello-v6").await;

	assert_eq!(reply.payload, Bytes::from_static(b"hello-v6"), "payload echoed back");
	// The responder is the IPv6 echo server; its address must surface as IPv6.
	assert_eq!(
		reply.target,
		TargetAddr::from(echo),
		"reply attributed to the IPv6 responder, got {:?}",
		reply.target
	);
	assert!(matches!(reply.target, TargetAddr::IPv6(..)));
}

#[tokio::test]
async fn ipv4_target_roundtrips_with_unmapped_source() {
	let echo = spawn_echo_server("127.0.0.1:0").await.expect("bind IPv4 echo server");

	let mut h = spawn_relay();
	let reply = roundtrip(&mut h, TargetAddr::from(echo), b"hello-v4").await;

	assert_eq!(reply.payload, Bytes::from_static(b"hello-v4"), "payload echoed back");
	// Crux of the dual-stack fix: the relay reaches 127.0.0.1 via its
	// IPv4-mapped form, but the reply source must be unmapped back to plain
	// IPv4 — otherwise the client sees `::ffff:127.0.0.1` and cannot match the
	// reply to the IPv4 target it sent to.
	assert_eq!(
		reply.target,
		TargetAddr::from(echo),
		"reply source unmapped to IPv4, got {:?}",
		reply.target
	);
	assert!(matches!(reply.target, TargetAddr::IPv4(..)));
}

#[tokio::test]
async fn single_association_relays_both_families() {
	let v4_echo = spawn_echo_server("127.0.0.1:0").await.expect("bind IPv4 echo server");
	let Ok(v6_echo) = spawn_echo_server("[::1]:0").await else {
		eprintln!("skipping single_association_relays_both_families: no IPv6 loopback available");
		return;
	};

	let mut h = spawn_relay();

	// Both targets travel over the SAME association — i.e. the same single
	// dual-stack relay socket must reach IPv4 and IPv6 hosts interchangeably.
	h.to_relay
		.send(UdpPacket {
			source: None,
			target: TargetAddr::from(v4_echo),
			payload: Bytes::from_static(b"to-v4"),
		})
		.await
		.unwrap();
	h.to_relay
		.send(UdpPacket {
			source: None,
			target: TargetAddr::from(v6_echo),
			payload: Bytes::from_static(b"to-v6"),
		})
		.await
		.unwrap();

	// Replies may arrive in either order; match on payload.
	let mut got_v4 = false;
	let mut got_v6 = false;
	for _ in 0..2 {
		let reply = timeout(REPLY_TIMEOUT, h.from_relay.recv())
			.await
			.expect("reply before the timeout")
			.expect("reply channel stayed open");
		match reply.payload.as_ref() {
			b"to-v4" => {
				assert_eq!(reply.target, TargetAddr::from(v4_echo));
				assert!(matches!(reply.target, TargetAddr::IPv4(..)));
				got_v4 = true;
			}
			b"to-v6" => {
				assert_eq!(reply.target, TargetAddr::from(v6_echo));
				assert!(matches!(reply.target, TargetAddr::IPv6(..)));
				got_v6 = true;
			}
			other => panic!("unexpected echo payload: {other:?}"),
		}
	}
	assert!(got_v4 && got_v6, "both address families round-tripped on one relay socket");
}
