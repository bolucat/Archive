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

		// Sample exactly as the traffic sampler does: only after the peer closes.
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
	// that the final window is accounted, so both directions must be non-zero and
	// cover at least the payload the server received and echoed back.
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
