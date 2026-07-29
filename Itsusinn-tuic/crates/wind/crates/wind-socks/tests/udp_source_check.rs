//! PR1 regression tests for the SOCKS5 UDP relay:
//!
//!  * `serve_udp_with_client` MUST drop datagrams whose source IP does not
//!    match the expected client (RFC 1928 §6).
//!  * The reply header MUST identify the REMOTE host that sent the data, not
//!    the client itself (RFC 1928 §7).

use std::{
	net::{IpAddr, Ipv4Addr, SocketAddr},
	time::Duration,
};

use bytes::Bytes;
use tokio::{net::UdpSocket, sync::mpsc, time::timeout};
use wind_core::{
	types::TargetAddr,
	udp::{UdpPacket, UdpStream},
};
use wind_socks::udp::serve_udp_with_client;

/// Build a minimal SOCKS5 UDP request frame: `RSV(2) FRAG(1) ATYP(1) DST.ADDR
/// DST.PORT payload`.
fn make_socks_udp_pkt(target: SocketAddr, payload: &[u8]) -> Vec<u8> {
	let mut buf = vec![0x00, 0x00, 0x00]; // RSV, RSV, FRAG
	match target {
		SocketAddr::V4(v4) => {
			buf.push(0x01);
			buf.extend_from_slice(&v4.ip().octets());
			buf.extend_from_slice(&v4.port().to_be_bytes());
		}
		SocketAddr::V6(v6) => {
			buf.push(0x04);
			buf.extend_from_slice(&v6.ip().octets());
			buf.extend_from_slice(&v6.port().to_be_bytes());
		}
	}
	buf.extend_from_slice(payload);
	buf
}

/// Spawn `serve_udp_with_client` and return:
///   - the relay's bound address (where senders point),
///   - the receiver side of relay→upstream packets (what the relay accepted),
///   - the sender side of upstream→relay packets (replies the test injects).
async fn spawn_relay(expected_client_ip: Option<IpAddr>) -> (SocketAddr, mpsc::Receiver<UdpPacket>, mpsc::Sender<UdpPacket>) {
	// Bind on a random port. We need a std socket because serve_udp_with_client
	// takes ownership of one and wraps it itself.
	let relay_std = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind relay");
	relay_std.set_nonblocking(true).unwrap();
	let bind_addr = relay_std.local_addr().unwrap();

	let (tx_to_upstream, rx_at_upstream) = mpsc::channel::<UdpPacket>(16);
	let (tx_at_upstream, rx_at_relay) = mpsc::channel::<UdpPacket>(16);

	let stream = UdpStream {
		tx: tx_to_upstream,
		rx: rx_at_relay,
	};

	tokio::spawn(async move {
		let _ = serve_udp_with_client(relay_std, stream, expected_client_ip).await;
	});

	(bind_addr, rx_at_upstream, tx_at_upstream)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_packet_from_unexpected_source_ip() {
	// Relay is told the legitimate client lives on TEST-NET-1 (192.0.2.1). Any
	// packet we send from 127.0.0.1 must be silently dropped.
	let (relay_addr, mut rx_at_upstream, _tx_at_upstream) = spawn_relay(Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)))).await;

	let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
	let pkt = make_socks_udp_pkt(SocketAddr::from(([1, 1, 1, 1], 53)), b"hello-dns");
	sender.send_to(&pkt, relay_addr).await.expect("send_to");

	// The relay should drop the datagram; nothing reaches the upstream channel.
	let res = timeout(Duration::from_millis(300), rx_at_upstream.recv()).await;
	assert!(
		res.is_err(),
		"datagram from unauthorized source IP must NOT be forwarded, got {res:?}"
	);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepts_packet_from_expected_source_ip() {
	// Relay expects the client on 127.0.0.1 — the same IP our sender will bind to.
	let (relay_addr, mut rx_at_upstream, _tx_at_upstream) = spawn_relay(Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).await;

	let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
	let target = SocketAddr::from(([1, 1, 1, 1], 53));
	let pkt = make_socks_udp_pkt(target, b"hello-dns");
	sender.send_to(&pkt, relay_addr).await.expect("send_to");

	let received = timeout(Duration::from_millis(500), rx_at_upstream.recv())
		.await
		.expect("relay must forward matching-source packet")
		.expect("upstream channel closed");

	assert_eq!(received.payload.as_ref(), b"hello-dns");
	match received.target {
		TargetAddr::IPv4(ip, port) => {
			assert_eq!(ip, Ipv4Addr::new(1, 1, 1, 1));
			assert_eq!(port, 53);
		}
		other => panic!("expected IPv4 target, got {other:?}"),
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_packet_latches_in_client_when_no_expected_ip() {
	// No expected_client_ip → the first packet's source IP becomes the latch.
	// A second packet from a different IP would be dropped, but we cannot bind
	// to a second loopback alias portably; instead we just verify the latch
	// behaviour through a single end-to-end exchange.
	let (relay_addr, mut rx_at_upstream, _tx_at_upstream) = spawn_relay(None).await;

	let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
	let pkt = make_socks_udp_pkt(SocketAddr::from(([8, 8, 8, 8], 53)), b"latch-me");
	sender.send_to(&pkt, relay_addr).await.expect("send_to");

	let received = timeout(Duration::from_millis(500), rx_at_upstream.recv())
		.await
		.expect("first packet from any source is latched in")
		.expect("upstream channel closed");

	assert_eq!(received.payload.as_ref(), b"latch-me");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reply_header_origin_is_remote_not_client() {
	// Verify RFC 1928 §7: the ATYP/DST.ADDR/DST.PORT in the UDP reply must
	// identify the remote host that sent the data, NOT the client. The bug we
	// fixed used `current_client` (the SOCKS5 client's own address) as the
	// reply origin, which made multi-destination demuxing impossible.

	let (relay_addr, mut rx_at_upstream, tx_at_upstream) = spawn_relay(Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).await;

	let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
	let client_addr = sender.local_addr().unwrap();

	// Step 1: client → relay (just to latch source addr in the relay tx_task).
	let pkt = make_socks_udp_pkt(SocketAddr::from(([1, 1, 1, 1], 53)), b"q");
	sender.send_to(&pkt, relay_addr).await.expect("send_to");
	let _ = timeout(Duration::from_millis(500), rx_at_upstream.recv())
		.await
		.expect("relay must accept matching client")
		.expect("upstream channel closed");

	// Step 2: inject a "reply" from upstream. Source is some unrelated remote.
	let remote_origin = SocketAddr::from(([9, 9, 9, 9], 4242));
	tx_at_upstream
		.send(UdpPacket {
			source: Some(TargetAddr::IPv4(Ipv4Addr::new(9, 9, 9, 9), 4242)),
			target: TargetAddr::IPv4(Ipv4Addr::new(1, 1, 1, 1), 53),
			payload: Bytes::from_static(b"reply"),
		})
		.await
		.unwrap();

	// Step 3: read it back on the client.
	let mut buf = [0u8; 256];
	let (n, from) = timeout(Duration::from_millis(500), sender.recv_from(&mut buf))
		.await
		.expect("reply must arrive")
		.expect("recv_from");
	assert_eq!(from, relay_addr, "reply must come from the relay's UDP port");

	// Parse the SOCKS5 UDP header in the reply: RSV(2) FRAG(1) ATYP(1) ADDR PORT
	// payload.
	let frame = &buf[..n];
	assert_eq!(&frame[0..2], &[0, 0], "RSV must be 00 00");
	assert_eq!(frame[2], 0, "FRAG must be 0");
	let atyp = frame[3];
	let origin_ip;
	let origin_port;
	let payload_start;
	match atyp {
		0x01 => {
			origin_ip = IpAddr::V4(Ipv4Addr::new(frame[4], frame[5], frame[6], frame[7]));
			origin_port = u16::from_be_bytes([frame[8], frame[9]]);
			payload_start = 10;
		}
		other => panic!("unexpected ATYP {other} in reply header"),
	}

	// The whole point: origin must be the REMOTE 9.9.9.9:4242, not the client's
	// own address `client_addr`.
	let remote_ip: IpAddr = remote_origin.ip();
	assert_eq!(
		origin_ip, remote_ip,
		"reply origin IP must be the remote host, not the client (was {client_addr})"
	);
	assert_eq!(origin_port, remote_origin.port(), "reply origin port must match remote");
	assert_eq!(&frame[payload_start..], b"reply");
}

/// PR3-G regression: a datagram that passes the source-IP check but fails to
/// parse as a SOCKS5 UDP request must NOT update the relay's latched client
/// address. Previously `source_addr.store(addr)` happened BEFORE parsing, so
/// a malformed packet from an authorized source IP could displace the
/// legitimate sender's port, sending all subsequent replies to a stale or
/// hostile peer.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_packet_does_not_displace_latched_source() {
	let (relay_addr, mut rx_at_upstream, tx_at_upstream) = spawn_relay(Some(IpAddr::V4(Ipv4Addr::LOCALHOST))).await;

	// 1) Send a well-formed packet from a legitimate sender. This both passes the
	//    IP check and (with the fix) sets `source_addr` to its `(127.0.0.1,
	//    legit_port)`.
	let legit = UdpSocket::bind("127.0.0.1:0").await.expect("bind legit");
	let legit_addr = legit.local_addr().unwrap();
	let frame = make_socks_udp_pkt(SocketAddr::from(([1, 1, 1, 1], 53)), b"first");
	legit.send_to(&frame, relay_addr).await.expect("send legit");
	let _ = timeout(Duration::from_millis(500), rx_at_upstream.recv())
		.await
		.expect("legit packet must be forwarded")
		.expect("upstream channel closed");

	// 2) Send a MALFORMED packet (RSV != 0x00 0x00) from a DIFFERENT 127.0.0.1
	//    port. Pre-PR3-G this stored the malformed sender's address into
	//    `source_addr` *before* parsing failed.
	let mallory = UdpSocket::bind("127.0.0.1:0").await.expect("bind mallory");
	let mallory_addr = mallory.local_addr().unwrap();
	assert_ne!(legit_addr, mallory_addr, "test setup: distinct local ports");
	let mut bad = vec![0xffu8, 0xff, 0x00, 0x01];
	bad.extend_from_slice(&[1, 1, 1, 1, 0, 53]);
	bad.extend_from_slice(b"junk");
	mallory.send_to(&bad, relay_addr).await.expect("send malformed");
	// Give the relay a tick to process (or drop) the bad packet.
	tokio::time::sleep(Duration::from_millis(50)).await;

	// 3) Inject a reply from upstream. It should still be delivered to
	//    `legit_addr`, NOT `mallory_addr`.
	tx_at_upstream
		.send(UdpPacket {
			source: Some(TargetAddr::IPv4(Ipv4Addr::new(1, 1, 1, 1), 53)),
			target: TargetAddr::IPv4(Ipv4Addr::new(1, 1, 1, 1), 53),
			payload: Bytes::from_static(b"reply"),
		})
		.await
		.unwrap();

	// 4) `legit` should receive; `mallory` should not.
	let mut buf = [0u8; 256];
	let legit_recv = timeout(Duration::from_millis(500), legit.recv_from(&mut buf)).await;
	assert!(
		legit_recv.is_ok(),
		"reply should reach the legitimate client even though a malformed packet arrived after the first valid one"
	);

	let mut buf2 = [0u8; 256];
	let mallory_recv = timeout(Duration::from_millis(150), mallory.recv_from(&mut buf2)).await;
	assert!(
		mallory_recv.is_err(),
		"reply must NOT be diverted to the malformed-packet sender: {mallory_recv:?}"
	);
}
