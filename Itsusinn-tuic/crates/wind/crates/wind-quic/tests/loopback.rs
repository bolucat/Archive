//! Backend-generic loopback smoke test.
//!
//! The same `run_case` exercise runs against both backends: open a bidi stream
//! and echo, send a uni stream, round-trip a datagram, confirm the keying
//! material matches on both ends, and close.

#![cfg(any(feature = "quinn", feature = "quiche"))]

use std::{net::SocketAddr, time::Duration};

use bytes::Bytes;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wind_quic::{ClientTlsConfig, QuicConnection, QuicSendStream, ServerTlsConfig, TransportConfig};

/// Generate a self-signed cert for `localhost` and write it to a temp dir,
/// returning `(dir, cert_path, key_path)`. The dir is returned so it outlives
/// the test (dropping it deletes the files).
fn write_self_signed() -> (tempfile::TempDir, String, String) {
	let generated = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
	let cert_pem = generated.cert.pem();
	let key_pem = generated.signing_key.serialize_pem();

	let dir = tempfile::tempdir().unwrap();
	let cert_path = dir.path().join("cert.pem");
	let key_path = dir.path().join("key.pem");
	std::fs::write(&cert_path, cert_pem).unwrap();
	std::fs::write(&key_path, key_pem).unwrap();
	(
		dir,
		cert_path.to_string_lossy().into_owned(),
		key_path.to_string_lossy().into_owned(),
	)
}

fn configs(cert: &str, key: &str) -> (ServerTlsConfig, ClientTlsConfig, TransportConfig) {
	let server_tls = ServerTlsConfig::from_pem_paths(cert, key);
	let mut client_tls = ClientTlsConfig::new("localhost");
	client_tls.verify_certificate = false;
	let transport = TransportConfig::default();
	(server_tls, client_tls, transport)
}

/// The shared exercise. `server` is a freshly-accepted server connection;
/// `client` is the matching client connection.
async fn run_case<C: QuicConnection>(server: C, client: C) {
	const LABEL: &[u8] = b"wind-quic-test-label";
	const CONTEXT: &[u8] = b"wind-quic-test-context";

	let server_task = tokio::spawn(async move {
		let (mut s_send, mut s_recv) = server.accept_bi().await.expect("accept_bi");
		let mut buf = [0u8; 4];
		s_recv.read_exact(&mut buf).await.expect("server read ping");
		assert_eq!(&buf, b"ping");
		s_send.write_all(b"pong").await.expect("server write pong");
		s_send.finish().expect("server finish");

		let mut u_recv = server.accept_uni().await.expect("accept_uni");
		let mut ubuf = Vec::new();
		u_recv.read_to_end(&mut ubuf).await.expect("server read uni");
		assert_eq!(ubuf.as_slice(), b"hello-uni");

		if server.max_datagram_size().is_some() {
			let dg = server.read_datagram().await.expect("server read datagram");
			server.send_datagram(dg).expect("server echo datagram");
		}

		let mut km = [0u8; 32];
		server
			.export_keying_material(&mut km, LABEL, CONTEXT)
			.await
			.expect("server export keying material");

		// Give the datagram echo time to flush before the worker can be torn
		// down by the client closing.
		tokio::time::sleep(Duration::from_millis(50)).await;
		km
	});

	let client_km = {
		let (mut c_send, mut c_recv) = client.open_bi().await.expect("open_bi");
		c_send.write_all(b"ping").await.expect("client write ping");
		c_send.finish().expect("client finish");
		let mut buf = [0u8; 4];
		c_recv.read_exact(&mut buf).await.expect("client read pong");
		assert_eq!(&buf, b"pong");

		let mut u_send = client.open_uni().await.expect("open_uni");
		u_send.write_all(b"hello-uni").await.expect("client write uni");
		u_send.finish().expect("client finish uni");

		if client.max_datagram_size().is_some() {
			client
				.send_datagram(Bytes::from_static(b"datagram-payload"))
				.expect("client send datagram");
			let echoed = client.read_datagram().await.expect("client read datagram");
			assert_eq!(&echoed[..], b"datagram-payload");
		}

		let mut km = [0u8; 32];
		client
			.export_keying_material(&mut km, LABEL, CONTEXT)
			.await
			.expect("client export keying material");
		km
	};

	let server_km = server_task.await.expect("server task");
	assert_eq!(server_km, client_km, "RFC 5705 keying material must match on both ends");

	client.close(0, b"done");
	tokio::time::timeout(Duration::from_secs(2), client.closed())
		.await
		.expect("client.closed() should resolve after close");
}

/// One-directional bulk transfer with a deliberately slow reader.
///
/// Regression guard for the quiche driver's inbound/outbound buffering rewrite.
/// The payload is many times both the per-stream channel capacity and the
/// outbound soft cap, so the sender must back-pressure through a bounded
/// `out_queue` (rather than growing it without bound), while the receiver's
/// periodic pauses drive the inbound `pending_in` buffering and re-flush path.
/// A gross break in either — lost data, an accounting bug in the queue length,
/// or a stalled re-arm — shows up here as a mismatch or a timeout.
async fn run_bulk<C: QuicConnection>(server: C, client: C) {
	const LEN: usize = 4 * 1024 * 1024;

	let server_task = tokio::spawn(async move {
		let (_s_send, mut s_recv) = server.accept_bi().await.expect("accept_bi");
		let mut buf = vec![0u8; LEN];
		let mut got = 0usize;
		let mut reads = 0u32;
		while got < LEN {
			let n = s_recv.read(&mut buf[got..]).await.expect("server read bulk");
			if n == 0 {
				break; // EOF
			}
			got += n;
			reads += 1;
			// Pause periodically so the inbound channel fills and the driver
			// buffers overflow in `pending_in`, exercising the re-flush wakeup.
			if reads % 16 == 0 {
				tokio::time::sleep(Duration::from_millis(1)).await;
			}
		}
		buf.truncate(got);
		buf
	});

	let (mut c_send, _c_recv) = client.open_bi().await.expect("open_bi");
	let payload: Vec<u8> = (0..LEN).map(|i| (i % 251) as u8).collect();
	c_send.write_all(&payload).await.expect("client write bulk");
	c_send.finish().expect("client finish bulk");

	let got = tokio::time::timeout(Duration::from_secs(30), server_task)
		.await
		.expect("bulk transfer timed out (inbound re-flush stall?)")
		.expect("server task");
	assert_eq!(got.len(), LEN, "all bulk bytes must arrive");
	assert_eq!(got, payload, "bulk payload must round-trip intact");

	client.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client.closed()).await;
}

/// An explicit peer reset (`reset(code)`) must surface on the receiver as an
/// I/O error, not a clean EOF — otherwise a truncated/aborted stream looks
/// complete. Both backends must behave identically.
async fn run_reset_visibility<C: QuicConnection>(server: C, client: C) {
	let server_task = tokio::spawn(async move {
		let (mut send, mut recv) = server.accept_bi().await.expect("accept_bi");
		let mut buf = [0u8; 4];
		recv.read_exact(&mut buf).await.expect("server read ping");
		send.write_all(b"partial").await.expect("server write partial");
		// Explicitly reset the send half.
		send.reset(7);
		// Hold the connection open long enough for the reset to propagate.
		tokio::time::sleep(Duration::from_millis(200)).await;
	});

	let (mut c_send, mut c_recv) = client.open_bi().await.expect("open_bi");
	c_send.write_all(b"ping").await.expect("client write ping");

	// Reading to end must error (reset), not return a clean EOF.
	let mut buf = Vec::new();
	let res = tokio::time::timeout(Duration::from_secs(5), c_recv.read_to_end(&mut buf)).await;
	match res {
		Ok(Ok(_)) => panic!(
			"peer reset surfaced as a clean EOF ({} bytes) instead of a reset error",
			buf.len()
		),
		Ok(Err(_)) => { /* expected: reset surfaced as an I/O error */ }
		Err(_) => panic!("read_to_end timed out; reset was never surfaced"),
	}

	server_task.await.expect("server task");
	client.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client.closed()).await;
}

#[cfg(feature = "quinn")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quinn_loopback() {
	use wind_quic::quinn;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let acceptor = quinn::bind_server(addr, &server_tls, &transport).expect("bind_server");
	let local = acceptor.local_addr().expect("local_addr");

	let server_fut = async move { acceptor.accept().await.expect("incoming").expect("server conn") };
	let client_fut = quinn::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	run_case(server_conn, client_conn).await;
}

#[cfg(feature = "quiche")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_loopback() {
	use wind_quic::quiche;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server");
	let local = acceptor.local_addr();

	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_fut = quiche::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	run_case(server_conn, client_conn).await;
}

#[cfg(feature = "quinn")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quinn_bulk_transfer() {
	use wind_quic::quinn;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let acceptor = quinn::bind_server(addr, &server_tls, &transport).expect("bind_server");
	let local = acceptor.local_addr().expect("local_addr");

	let server_fut = async move { acceptor.accept().await.expect("incoming").expect("server conn") };
	let client_fut = quinn::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	run_bulk(server_conn, client_conn).await;
}

// x86_64 only. Pushing ~4 MiB drives quiche 0.29's congestion controller hard,
// which is unreliable off x86_64: it panics in PRR recovery on 32-bit
// (`congestion/prr.rs` overflow) and paces so slowly on the aarch64 CI runners
// that the transfer times out. Both are upstream quiche limitations unrelated
// to the (architecture-independent) driver buffering this test exercises, which
// x86_64 covers; the `quinn_bulk_transfer` variant still runs everywhere.
#[cfg(all(feature = "quiche", target_arch = "x86_64"))]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_bulk_transfer() {
	use wind_quic::quiche;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server");
	let local = acceptor.local_addr();

	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_fut = quiche::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	run_bulk(server_conn, client_conn).await;
}

#[cfg(feature = "quinn")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quinn_reset_visibility() {
	use wind_quic::quinn;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let acceptor = quinn::bind_server(addr, &server_tls, &transport).expect("bind_server");
	let local = acceptor.local_addr().expect("local_addr");

	let server_fut = async move { acceptor.accept().await.expect("incoming").expect("server conn") };
	let client_fut = quinn::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	run_reset_visibility(server_conn, client_conn).await;
}

#[cfg(feature = "quiche")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_reset_visibility() {
	use wind_quic::quiche;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server");
	let local = acceptor.local_addr();

	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_fut = quiche::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	run_reset_visibility(server_conn, client_conn).await;
}

/// A zero-length keying-material export must still reach the TLS exporter and
/// succeed on the quiche backend.
///
/// The driver passes the caller's buffer to `SSL_export_keying_material` as a
/// `(ptr, len)` pair, so a zero-length export used to hand it a dangling
/// pointer (a zero-capacity `Vec` has no allocation). That only happened not to
/// fault because BoringSSL's HKDF expansion loop never runs for a zero length.
/// This test drives the whole path — including the FFI call — with an empty
/// buffer.
#[cfg(feature = "quiche")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_zero_length_export_succeeds() {
	use wind_quic::quiche;

	const LABEL: &[u8] = b"wind-quic-zero-length-export";
	const CONTEXT: &[u8] = b"wind-quic-zero-length-context";

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server");
	let local = acceptor.local_addr();

	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_fut = quiche::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	let mut s_out: [u8; 0] = [];
	let mut c_out: [u8; 0] = [];
	server_conn
		.export_keying_material(&mut s_out, LABEL, CONTEXT)
		.await
		.expect("server export of zero bytes");
	client_conn
		.export_keying_material(&mut c_out, LABEL, CONTEXT)
		.await
		.expect("client export of zero bytes");

	client_conn.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client_conn.closed()).await;
}

/// Regression: per-user traffic accounting samples `byte_stats()` one final
/// time when the connection closes. That read must still return the final
/// `(sent, recv)` *after* the connection has closed and its driver worker has
/// exited. On the quiche backend the read previously round-tripped through the
/// (now-gone) driver and returned `None`, silently dropping the connection's
/// last traffic window (and *all* traffic for connections shorter than the
/// sampler interval). The counts are now cached in shared state and finalized
/// in `on_conn_close`, so a post-close read works on both backends.
async fn byte_stats_survives_close<C: QuicConnection>(server: C, client: C) {
	const PAYLOAD: &[u8] = &[0xABu8; 16 * 1024];

	let server_task = tokio::spawn(async move {
		let (mut s_send, mut s_recv) = server.accept_bi().await.expect("accept_bi");
		let mut buf = vec![0u8; PAYLOAD.len()];
		s_recv.read_exact(&mut buf).await.expect("server read payload");
		s_send.write_all(&buf).await.expect("server echo payload");
		s_send.finish().expect("server finish");

		// Sample exactly as the traffic sampler does: only after the peer
		// closes.
		tokio::time::timeout(Duration::from_secs(2), server.closed())
			.await
			.expect("server.closed() should resolve");
		server.byte_stats().await
	});

	let (mut c_send, mut c_recv) = client.open_bi().await.expect("open_bi");
	c_send.write_all(PAYLOAD).await.expect("client write payload");
	c_send.finish().expect("client finish");
	let mut echo = vec![0u8; PAYLOAD.len()];
	c_recv.read_exact(&mut echo).await.expect("client read echo");
	assert_eq!(echo, PAYLOAD, "echo round-trip");

	client.close(0, b"done");
	tokio::time::timeout(Duration::from_secs(2), client.closed())
		.await
		.expect("client.closed() should resolve");

	let stats = server_task.await.expect("server task");
	let (sent, recv) = stats.expect("byte_stats must still return Some(..) after the connection closed");
	// The pre-fix quiche bug surfaced as `None` here; the substantive check is
	// that the final window is accounted, so both directions must be non-zero
	// and cover at least the payload the server received and echoed back.
	assert!(
		recv as usize >= PAYLOAD.len(),
		"recv wire bytes should cover the received payload: recv={recv} payload={}",
		PAYLOAD.len()
	);
	assert!(
		sent as usize >= PAYLOAD.len(),
		"sent wire bytes should cover the echoed payload: sent={sent} payload={}",
		PAYLOAD.len()
	);
}

#[cfg(feature = "quinn")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quinn_byte_stats_survives_close() {
	use wind_quic::quinn;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let acceptor = quinn::bind_server(addr, &server_tls, &transport).expect("bind_server");
	let local = acceptor.local_addr().expect("local_addr");

	let server_fut = async move { acceptor.accept().await.expect("incoming").expect("server conn") };
	let client_fut = quinn::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	byte_stats_survives_close(server_conn, client_conn).await;
}

#[cfg(feature = "quiche")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_byte_stats_survives_close() {
	use wind_quic::quiche;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server");
	let local = acceptor.local_addr();

	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_fut = quiche::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
	let client_conn = client_conn.expect("client connect");

	byte_stats_survives_close(server_conn, client_conn).await;
}

/// The quiche client must bind its local UDP socket on the peer's address
/// family. It used to always bind `0.0.0.0:0`, so dialing an IPv6 peer failed
/// with an address-family mismatch before the handshake could start.
#[cfg(feature = "quiche")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_connects_to_ipv6_peer() {
	use wind_quic::quiche;

	// Some environments have IPv6 disabled; that is not this test's subject.
	if tokio::net::UdpSocket::bind("[::1]:0").await.is_err() {
		eprintln!("skipping quiche_connects_to_ipv6_peer: no IPv6 loopback on this host");
		return;
	}

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "[::1]:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server on [::1]");
	let local = acceptor.local_addr();
	assert!(local.is_ipv6(), "server must be bound on IPv6: {local}");

	// Wait for the client first, bounded: a mismatched bind address family
	// fails the dial outright, and the acceptor would then wait forever for a
	// connection that can never arrive.
	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_conn = tokio::time::timeout(Duration::from_secs(10), quiche::connect(local, &client_tls, &transport))
		.await
		.expect("client connect to an IPv6 peer timed out")
		.expect("client connect to an IPv6 peer");
	let server_conn = tokio::time::timeout(Duration::from_secs(10), server_fut)
		.await
		.expect("server never saw the IPv6 handshake");

	let (mut c_send, mut c_recv) = client_conn.open_bi().await.expect("open_bi");
	c_send.write_all(b"ping").await.expect("client write ping");
	c_send.finish().expect("client finish");
	let (mut s_send, mut s_recv) = server_conn.accept_bi().await.expect("accept_bi");
	let mut buf = [0u8; 4];
	s_recv.read_exact(&mut buf).await.expect("server read ping");
	assert_eq!(&buf, b"ping");
	s_send.write_all(b"pong").await.expect("server write pong");
	s_send.finish().expect("server finish");
	let mut echo = [0u8; 4];
	c_recv.read_exact(&mut echo).await.expect("client read pong");
	assert_eq!(&echo, b"pong");

	client_conn.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client_conn.closed()).await;
}

/// Echo `payload` over a fresh bidi stream, asserting the byte-for-byte
/// round trip on both ends. Used by the address-family tests below.
async fn echo_once<C: QuicConnection>(server: &C, client: &C) {
	const PAYLOAD: &[u8] = b"family-probe";

	let server_side = async {
		let (mut s_send, mut s_recv) = server.accept_bi().await.expect("accept_bi");
		let mut buf = vec![0u8; PAYLOAD.len()];
		s_recv.read_exact(&mut buf).await.expect("server read probe");
		assert_eq!(buf.as_slice(), PAYLOAD);
		s_send.write_all(&buf).await.expect("server write echo");
		s_send.finish().expect("server finish");
	};
	let client_side = async {
		let (mut c_send, mut c_recv) = client.open_bi().await.expect("open_bi");
		c_send.write_all(PAYLOAD).await.expect("client write probe");
		c_send.finish().expect("client finish");
		let mut echo = vec![0u8; PAYLOAD.len()];
		c_recv.read_exact(&mut echo).await.expect("client read echo");
		assert_eq!(echo.as_slice(), PAYLOAD, "echo round-trip");
	};
	let (server_res, client_res) = tokio::join!(server_side, client_side);
	assert_eq!(server_res, ());
	assert_eq!(client_res, ());
}

/// Whether this host has an IPv6 loopback interface.
///
/// A host without one cannot run the address-family tests; that is not their
/// subject, so they report a skip rather than failing.
async fn has_ipv6_loopback() -> bool {
	tokio::net::UdpSocket::bind("[::1]:0").await.is_ok()
}

/// The quinn client must bind its local UDP socket on the peer's address
/// family. It used to always bind `0.0.0.0:0`, so dialing an IPv6 peer failed
/// with an address-family mismatch before the handshake could start.
#[cfg(feature = "quinn")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quinn_connects_to_ipv6_peer() {
	use wind_quic::quinn;

	if !has_ipv6_loopback().await {
		eprintln!("skipping quinn_connects_to_ipv6_peer: no IPv6 loopback on this host");
		return;
	}

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let addr: SocketAddr = "[::1]:0".parse().unwrap();
	let acceptor = quinn::bind_server(addr, &server_tls, &transport).expect("bind_server on [::1]");
	let local = acceptor.local_addr().expect("local_addr");
	assert!(local.is_ipv6(), "server must be bound on IPv6: {local}");

	// Drive both sides concurrently and bound the whole exchange: a mismatched
	// bind address family fails the dial outright, and the acceptor would then
	// wait forever for a connection that can never arrive.
	let server_fut = async move { acceptor.accept().await.expect("incoming").expect("server conn") };
	let client_fut = quinn::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::time::timeout(Duration::from_secs(10), async {
		let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
		(server_conn, client_conn)
	})
	.await
	.expect("IPv6 handshake timed out");
	let client_conn = client_conn.expect("client connect to an IPv6 peer");

	echo_once(&server_conn, &client_conn).await;

	client_conn.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client_conn.closed()).await;
}

/// [`wind_quic::quinn::QuinnClient`] builds one long-lived endpoint up front,
/// so it cannot infer the peer's family the way [`wind_quic::quinn::connect`]
/// does. It must therefore expose an explicit local bind address; the default
/// stays IPv4 so existing callers keep their behavior.
#[cfg(feature = "quinn")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quinn_client_binds_the_configured_address_family() {
	use wind_quic::quinn;

	if !has_ipv6_loopback().await {
		eprintln!("skipping quinn_client_binds_the_configured_address_family: no IPv6 loopback on this host");
		return;
	}

	// A family that does not match the peer is rejected at connect time rather
	// than sent to the wrong socket, and the default remains IPv4.
	let ipv4_socket: SocketAddr = "0.0.0.0:0".parse().unwrap();
	assert_eq!(quinn::client_bind_addr("127.0.0.1:1".parse().unwrap()), ipv4_socket);
	assert_eq!(
		quinn::client_bind_addr("[::1]:1".parse().unwrap()),
		"[::]:0".parse::<SocketAddr>().unwrap()
	);

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);

	let ipv4_client = quinn::QuinnClient::new(&client_tls, &transport)
		.await
		.expect("default client");
	let ipv4_local = ipv4_client.local_addr().expect("local_addr");
	assert!(
		!ipv4_local.is_ipv6(),
		"QuinnClient::new must keep binding an IPv4 socket: {ipv4_local}"
	);
	assert!(
		ipv4_client.connecting("[::1]:1".parse().unwrap()).is_err(),
		"a cross-family peer must be rejected instead of dialed from the wrong socket"
	);
	drop(ipv4_client);

	// Same-family (IPv6) dial through the explicit bind address.
	let addr: SocketAddr = "[::1]:0".parse().unwrap();
	let acceptor = quinn::bind_server(addr, &server_tls, &transport).expect("bind_server on [::1]");
	let local = acceptor.local_addr().expect("local_addr");

	let server_fut = async move { acceptor.accept().await.expect("incoming").expect("server conn") };
	let client_fut = async {
		let client = quinn::QuinnClient::new_bound(&client_tls, &transport, "[::]:0".parse().unwrap())
			.await
			.expect("new_bound client");
		client.connect(local).await.expect("client connect to an IPv6 peer")
	};
	let (server_conn, client_conn) = tokio::time::timeout(Duration::from_secs(10), async {
		let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
		(server_conn, client_conn)
	})
	.await
	.expect("IPv6 handshake through QuinnClient timed out");

	echo_once(&server_conn, &client_conn).await;

	client_conn.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client_conn.closed()).await;
}

/// Flood `client` with datagrams in bursts while a reader drains them slower
/// than they arrive.
///
/// Models a handle that lags behind the peer: the flood must not be absorbed
/// into an unbounded driver queue. Returns how many datagrams the flood sent
/// (`sent`), how many arrived (`received`), how many the driver dropped in
/// total (`dropped`), and the drop count observed at the moment the flood
/// stopped (`dropped_at_stop`). A healthy run has `dropped_at_stop > 0`
/// *while* the flood is still delivering datagrams; an unbounded queue drops
/// nothing no matter how far behind the reader is, so it can never produce
/// that.
#[cfg(feature = "quiche")]
async fn read_datagrams_during_flood(
	server: &wind_quic::quiche::QuicheConnection,
	client: &wind_quic::quiche::QuicheConnection,
) -> (usize, usize, u64, u64) {
	/// The burst has to be large enough to outrun the reader for the length of
	/// a batch — otherwise the queue never fills and a bounded queue is
	/// indistinguishable from an unbounded one — while staying under the
	/// kernel's UDP receive buffer so the drops this test counts are the
	/// driver's, not the kernel's. A 4096-datagram burst delivered every sent
	/// datagram below the driver's cap on the development host.
	const BATCH: usize = 512;
	const MAX_BATCHES: usize = 64;

	let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
	let reader = async {
		let mut received = 0usize;
		loop {
			tokio::select! {
				res = client.read_datagram() => match res {
					Ok(_) => received += 1,
					Err(_) => break,
				},
				_ = &mut stop_rx => {
					// The flood is over; drain whatever is already queued so
					// the caller can compare arrivals against sends.
					while tokio::time::timeout(Duration::from_millis(100), client.read_datagram())
						.await
						.is_ok()
					{
						received += 1;
					}
					break
				}
			}
			// Deliberately drain slower than the bursts deliver, so the
			// driver's queue is what has to absorb the difference.
			if received.is_multiple_of(4) {
				tokio::time::sleep(Duration::from_millis(1)).await;
			}
		}
		received
	};

	let flood = async {
		let mut sent = 0usize;
		let mut dropped_at_stop = 0u64;
		for _ in 0..MAX_BATCHES {
			for _ in 0..BATCH {
				server
					.send_datagram(Bytes::from_static(b"flood"))
					.expect("send datagram during flood");
			}
			sent += BATCH;
			// Let the peer's worker move the batch into its receive queue;
			// whether that queue is bounded is the subject of the test.
			tokio::time::sleep(Duration::from_millis(2)).await;
			dropped_at_stop = client.dropped_datagrams();
			if dropped_at_stop > 0 {
				break;
			}
		}
		let _ = stop_tx.send(());
		(sent, dropped_at_stop)
	};

	let (received, (sent, dropped_at_stop)) = tokio::join!(reader, flood);
	(received, sent, client.dropped_datagrams(), dropped_at_stop)
}

/// The quiche driver's inbound datagram queue is bounded.
///
/// `process_reads` used to drain every queued datagram into an *unbounded*
/// channel, so a handle that stopped (or lagged in) reading `read_datagram`
/// let peer-driven memory grow without bound — while the outbound side was
/// already capped at 2048. The flood below therefore has to start dropping
/// datagrams while it is still delivering them; if the queue were unbounded
/// nothing would ever be dropped, no matter how far behind the reader is.
#[cfg(feature = "quiche")]
#[test_log::test(tokio::test(flavor = "multi_thread", worker_threads = 2))]
async fn quiche_inbound_datagram_queue_is_bounded() {
	use wind_quic::quiche;

	let (_dir, cert, key) = write_self_signed();
	let (server_tls, client_tls, transport) = configs(&cert, &key);
	assert!(
		transport.enable_datagram,
		"the transport must advertise DATAGRAM support for this test to mean anything"
	);

	let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
	let mut acceptor = quiche::bind_server(addr, &server_tls, &transport, None)
		.await
		.expect("bind_server");
	let local = acceptor.local_addr();

	let server_fut = async move { acceptor.accept().await.expect("server conn") };
	let client_fut = quiche::connect(local, &client_tls, &transport);
	let (server_conn, client_conn) = tokio::time::timeout(Duration::from_secs(10), async {
		let (server_conn, client_conn) = tokio::join!(server_fut, client_fut);
		(server_conn, client_conn)
	})
	.await
	.expect("handshake timed out");
	let client_conn = client_conn.expect("client connect");
	assert!(
		client_conn.max_datagram_size().is_some(),
		"the client must have negotiated DATAGRAM support"
	);

	let (received, sent, dropped, dropped_at_stop) = tokio::time::timeout(
		Duration::from_secs(60),
		read_datagrams_during_flood(&server_conn, &client_conn),
	)
	.await
	.expect("datagram flood timed out");

	assert!(
		dropped_at_stop > 0,
		"the driver dropped nothing while the reader fell behind (sent={sent} received={received} dropped={dropped}): the \
		 inbound datagram queue is unbounded again"
	);
	assert!(
		dropped > 0,
		"a bounded inbound queue must report its drops (sent={sent} received={received})"
	);
	assert!(
		received > 0,
		"the flood has to deliver datagrams for the drop count to mean anything (sent={sent} dropped={dropped})"
	);
	assert!(
		received <= sent,
		"a receiver can never see more datagrams than were sent (sent={sent} received={received})"
	);
	assert_eq!(
		client_conn.dropped_datagrams(),
		dropped,
		"the drop counter must keep counting monotonically"
	);

	client_conn.close(0, b"done");
	let _ = tokio::time::timeout(Duration::from_secs(2), client_conn.closed()).await;
}
