//! Integration tests for the wind-tuic QUIC proxy.
//!
//! Provides public test helpers and verifies TUIC connection, authentication,
//! and TCP proxy behaviour.

#[cfg(test)]
use std::{collections::HashMap, time::Duration};
use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::UdpSocket;
use tracing::{Instrument as _, warn};
#[cfg(test)]
use wind_core::AppContext;
use wind_core::{
	InboundCallback,
	tcp::AbstractTcpStream,
	types::TargetAddr,
	udp::{UdpPacket, UdpStream},
};

/// A simple [`InboundCallback`] that relays TCP connections directly to their
/// targets and relays UDP packets to real targets via a bound UDP socket.
///
/// Suitable for use in integration tests that exercise TUIC server behaviour.
#[derive(Clone)]
pub struct DirectCallback;

impl InboundCallback for DirectCallback {
	async fn handle_tcpstream(
		&self,
		target_addr: TargetAddr,
		mut stream: impl AbstractTcpStream + 'static,
	) -> eyre::Result<()> {
		let addr = target_addr.to_string();
		let mut target = tokio::net::TcpStream::connect(&addr).await?;
		tokio::io::copy_bidirectional(&mut stream, &mut target).await?;
		Ok(())
	}

	async fn handle_udpstream(&self, stream: UdpStream) -> eyre::Result<()> {
		let UdpStream { tx, mut rx } = stream;
		let relay_socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);

		let send_socket = relay_socket.clone();
		let recv_handle = tokio::spawn(
			async move {
				while let Some(packet) = rx.recv().await {
					let target_addr = match packet.target {
						TargetAddr::IPv4(ip, port) => SocketAddr::new(std::net::IpAddr::V4(ip), port),
						TargetAddr::IPv6(ip, port) => SocketAddr::new(std::net::IpAddr::V6(ip), port),
						TargetAddr::Domain(..) => {
							warn!(
								target: "wind_test::direct_cb",
								target_addr = "domain (unresolved)",
								payload_len = packet.payload.len(),
								"DirectCallback drops UDP packet with unresolved domain target",
							);
							continue;
						}
					};
					// Don't `let _ = ...` the result. On macOS, payloads larger
					// than `net.inet.udp.maxdgram` (default 9216) fail
					// `send_to` with EMSGSIZE — silently dropping that turns
					// the test into a 15-second timeout 1000 lines downstream.
					// At least warn so the next person reading CI logs has a
					// fighting chance.
					if let Err(e) = send_socket.send_to(&packet.payload, target_addr).await {
						warn!(
							target: "wind_test::direct_cb",
							%target_addr,
							payload_len = packet.payload.len(),
							error_kind = ?e.kind(),
							error = %e,
							"DirectCallback send_to failed; UDP packet dropped",
						);
					}
				}
			}
			.in_current_span(),
		);

		tokio::spawn(
			async move {
				let mut buf = vec![0u8; 65536];
				loop {
					match relay_socket.recv_from(&mut buf).await {
						Ok((n, peer)) => {
							let source = match peer {
								SocketAddr::V4(a) => TargetAddr::IPv4(*a.ip(), a.port()),
								SocketAddr::V6(a) => TargetAddr::IPv6(*a.ip(), a.port()),
							};
							let packet = UdpPacket {
								source: Some(source.clone()),
								target: source,
								payload: Bytes::copy_from_slice(&buf[..n]),
							};
							if tx.send(packet).await.is_err() {
								// Upstream channel closed — the only legitimate
								// shutdown signal. Quiet break.
								break;
							}
						}
						Err(e) => {
							// Recv errors are rare (mostly socket closure /
							// cancellation). Make the cause visible instead of
							// swallowing it.
							warn!(
								target: "wind_test::direct_cb",
								error_kind = ?e.kind(),
								error = %e,
								"DirectCallback relay socket recv_from failed; closing reply task",
							);
							break;
						}
					}
				}
			}
			.in_current_span(),
		);

		recv_handle.await?;
		Ok(())
	}
}

/// Generate a self-signed TLS certificate suitable for TUIC testing.
pub fn generate_tuic_test_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
	let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
	let cert_der = CertificateDer::from(cert.cert);
	let key_der = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
	(vec![cert_der], PrivateKeyDer::Pkcs8(key_der))
}

#[cfg(test)]
mod tests {
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	use uuid::Uuid;
	use wind_core::{AbstractInbound, AbstractOutbound};
	use wind_tuic::quinn::{
		inbound::{TuicInbound, TuicInboundOpts},
		outbound::{ReconnectConfig, TuicOutbound, TuicOutboundOpts},
	};

	use super::*;

	const TEST_PASSWORD: &[u8] = b"wind_tuic_test_secret";

	struct TuicTestSetup {
		server_addr: SocketAddr,
		uuid: Uuid,
		ctx: Arc<AppContext>,
	}

	impl Drop for TuicTestSetup {
		fn drop(&mut self) {
			self.ctx.token.cancel();
		}
	}

	async fn setup_tuic_server() -> eyre::Result<TuicTestSetup> {
		let (ctx, server_addr, uuid, _listen) = spawn_tuic_server().await?;
		// The listen task is left detached: `TuicTestSetup::drop` cancels the
		// context token, which makes the accept loop break on its own.
		Ok(TuicTestSetup { server_addr, uuid, ctx })
	}

	/// Lower-level server bring-up that also hands back the `listen`
	/// accept-loop join handle (and the context, so the caller can cancel it).
	///
	/// Used by the graceful-shutdown tests, which need to *await* the accept
	/// loop to prove it exits on cancellation — something
	/// [`setup_tuic_server`] can't expose because [`TuicTestSetup`] owns a
	/// `Drop` guard and can't surrender a field by move.
	async fn spawn_tuic_server() -> eyre::Result<(Arc<AppContext>, SocketAddr, Uuid, tokio::task::JoinHandle<eyre::Result<()>>)>
	{
		let (cert, key) = generate_tuic_test_cert();
		let uuid = Uuid::new_v4();
		let mut users = HashMap::new();
		users.insert(uuid, String::from_utf8_lossy(TEST_PASSWORD).to_string());

		// Obtain a free UDP port without holding the socket
		let temp = std::net::UdpSocket::bind("127.0.0.1:0")?;
		let server_addr = temp.local_addr()?;
		drop(temp);

		let ctx = Arc::new(AppContext::default());
		let opts = TuicInboundOpts {
			listen_addr: server_addr,
			certificate: cert,
			private_key: key,
			alpn: vec!["h3".to_string()],
			users,
			auth_timeout: Duration::from_secs(5),
			max_idle_time: Duration::from_secs(30),
			zero_rtt: false,
			..Default::default()
		};

		let server = TuicInbound::new(ctx.clone(), opts);
		let callback = Arc::new(DirectCallback);
		let listen = tokio::spawn(async move { server.listen(callback.as_ref()).await }.in_current_span());

		// Allow the server time to bind and begin accepting
		tokio::time::sleep(Duration::from_millis(300)).await;

		Ok((ctx, server_addr, uuid, listen))
	}

	/// Connect a TUIC client to `addr`/`uuid` and start its heartbeat poll.
	/// Used by the graceful-shutdown tests, which need a client whose lifetime
	/// is independent of the [`TuicTestSetup`] `Drop` guard.
	async fn connect_client(addr: SocketAddr, uuid: Uuid) -> eyre::Result<Arc<TuicOutbound>> {
		connect_client_with(addr, uuid, ReconnectConfig::default()).await
	}

	/// As [`connect_client`], but with an explicit reconnect policy — lets
	/// tests disable reconnect or tune its backoff.
	async fn connect_client_with(addr: SocketAddr, uuid: Uuid, reconnect: ReconnectConfig) -> eyre::Result<Arc<TuicOutbound>> {
		let ctx = Arc::new(AppContext::default());
		let opts = TuicOutboundOpts {
			peer_addr: addr,
			sni: "localhost".to_string(),
			auth: (uuid, Arc::from(TEST_PASSWORD)),
			zero_rtt_handshake: false,
			heartbeat: Duration::from_secs(5),
			gc_interval: Duration::from_secs(5),
			gc_lifetime: Duration::from_secs(30),
			skip_cert_verify: true,
			alpn: vec!["h3".to_string()],
			reconnect,
		};
		let client = Arc::new(TuicOutbound::new(ctx, opts).await?);
		let poll_client = client.clone();
		tokio::spawn(
			async move {
				let _ = poll_client.start_poll().await;
			}
			.in_current_span(),
		);
		tokio::time::sleep(Duration::from_millis(150)).await;
		Ok(client)
	}

	async fn connect_tuic_client(setup: &TuicTestSetup) -> eyre::Result<Arc<TuicOutbound>> {
		let ctx = Arc::new(AppContext::default());
		let opts = TuicOutboundOpts {
			peer_addr: setup.server_addr,
			sni: "localhost".to_string(),
			auth: (setup.uuid, Arc::from(TEST_PASSWORD)),
			zero_rtt_handshake: false,
			heartbeat: Duration::from_secs(5),
			gc_interval: Duration::from_secs(5),
			gc_lifetime: Duration::from_secs(30),
			skip_cert_verify: true,
			alpn: vec!["h3".to_string()],
			reconnect: ReconnectConfig::default(),
		};
		let client: std::sync::Arc<TuicOutbound> = std::sync::Arc::new(TuicOutbound::new(ctx.clone(), opts).await?);
		let poll_client = client.clone();
		tokio::spawn(
			async move {
				let _ = poll_client.start_poll().await;
			}
			.in_current_span(),
		);
		tokio::time::sleep(Duration::from_millis(100)).await;
		Ok(client)
	}

	/// A UDP association must be torn down when its local stream closes:
	/// `handle_udp` has to return (freeing its task, assoc id, and cache entry
	/// and sending a `Dissociate`) rather than spinning forever. Before the fix
	/// it looped on the global token only, so `handle_udp` never returned for a
	/// finished session — this test would hang at the join timeout.
	#[tokio::test]
	async fn udp_association_is_released_when_local_stream_closes() {
		let setup = setup_tuic_server().await.expect("start TUIC server");
		let client = connect_tuic_client(&setup).await.expect("connect TUIC client");

		for _ in 0..3 {
			let (tx_to_client, rx_at_client) = tokio::sync::mpsc::channel::<UdpPacket>(32);
			let (tx_to_test, _rx_at_test) = tokio::sync::mpsc::channel::<UdpPacket>(32);
			let stream = UdpStream {
				tx: tx_to_test,
				rx: rx_at_client,
			};
			let c = client.clone();
			let handle = tokio::spawn(async move { c.handle_udp(stream, None::<TuicOutbound>).await });

			// Let the association get created, then close the local side.
			tokio::time::sleep(Duration::from_millis(100)).await;
			drop(tx_to_client);

			// `handle_udp` must now return promptly; before the fix it spun forever.
			tokio::time::timeout(Duration::from_secs(5), handle)
				.await
				.expect("handle_udp did not return after the local stream closed")
				.expect("handle_udp task panicked")
				.expect("handle_udp errored");
		}

		client.udp_session.run_pending_tasks().await;
		assert_eq!(
			client.udp_session.entry_count(),
			0,
			"UDP session cache entries leaked after all local streams closed"
		);
	}

	/// A TUIC client must be able to establish a QUIC connection to the server.
	#[tokio::test]
	async fn test_tuic_connection() {
		let setup = setup_tuic_server().await.expect("Failed to start TUIC server");
		let result: eyre::Result<std::sync::Arc<TuicOutbound>> = connect_tuic_client(&setup).await;
		assert!(result.is_ok(), "Client should connect successfully: {:?}", result.err());
	}

	/// Connecting with a valid UUID and correct password must succeed.
	#[tokio::test]
	async fn test_tuic_auth_valid_credentials() {
		let setup = setup_tuic_server().await.expect("Failed to start TUIC server");
		let result: eyre::Result<std::sync::Arc<TuicOutbound>> = connect_tuic_client(&setup).await;
		assert!(result.is_ok(), "Valid credentials must be accepted: {:?}", result.err());
	}

	/// The QUIC transport layer must accept a connection even when the
	/// application-level password is wrong.  The server validates the Auth
	/// unidirectional stream asynchronously and will close the connection after
	/// the auth timeout.
	#[tokio::test]
	async fn test_tuic_auth_wrong_password() {
		let setup = setup_tuic_server().await.expect("Failed to start TUIC server");
		let ctx = Arc::new(AppContext::default());
		let opts = TuicOutboundOpts {
			peer_addr: setup.server_addr,
			sni: "localhost".to_string(),
			auth: (setup.uuid, Arc::from(b"wrong_password_123".as_slice())),
			zero_rtt_handshake: false,
			heartbeat: Duration::from_secs(5),
			gc_interval: Duration::from_secs(5),
			gc_lifetime: Duration::from_secs(30),
			skip_cert_verify: true,
			alpn: vec!["h3".to_string()],
			reconnect: ReconnectConfig::default(),
		};
		let result: eyre::Result<TuicOutbound> = TuicOutbound::new(ctx, opts).await;
		assert!(
			result.is_ok(),
			"QUIC transport connection should succeed regardless of password (auth is async): {:?}",
			result.err()
		);
	}

	/// Connecting with an unknown UUID should still establish the QUIC
	/// connection; the server will reject the Auth stream and close the
	/// connection after the auth timeout.
	#[tokio::test]
	async fn test_tuic_auth_unknown_user() {
		let setup = setup_tuic_server().await.expect("Failed to start TUIC server");
		let ctx = Arc::new(AppContext::default());
		let opts = TuicOutboundOpts {
			peer_addr: setup.server_addr,
			sni: "localhost".to_string(),
			auth: (Uuid::new_v4(), Arc::from(TEST_PASSWORD)),
			zero_rtt_handshake: false,
			heartbeat: Duration::from_secs(5),
			gc_interval: Duration::from_secs(5),
			gc_lifetime: Duration::from_secs(30),
			skip_cert_verify: true,
			alpn: vec!["h3".to_string()],
			reconnect: ReconnectConfig::default(),
		};
		let result: eyre::Result<TuicOutbound> = TuicOutbound::new(ctx, opts).await;
		assert!(
			result.is_ok(),
			"QUIC transport should succeed with unknown UUID; auth failure is handled async: {:?}",
			result.err()
		);
	}

	/// End-to-end TCP relay through TUIC: a message written on the local stream
	/// must reach the echo target and the echo must arrive back at the caller.
	#[tokio::test]
	async fn test_tuic_tcp_proxy() {
		use tokio::net::TcpListener;

		let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let echo_addr = echo.local_addr().unwrap();
		tokio::spawn(async move {
			while let Ok((mut stream, _)) = echo.accept().await {
				tokio::spawn(async move {
					let mut buf = vec![0u8; 4096];
					loop {
						match stream.read(&mut buf).await {
							Ok(0) | Err(_) => break,
							Ok(n) => {
								if stream.write_all(&buf[..n]).await.is_err() {
									break;
								}
							}
						}
					}
				});
			}
		});

		let setup = setup_tuic_server().await.expect("Failed to start TUIC server");
		let client: std::sync::Arc<TuicOutbound> = connect_tuic_client(&setup).await.expect("Failed to connect TUIC client");

		// `local` is the test end; `remote` is passed to handle_tcp as the local
		// stream.
		let (mut local, remote) = tokio::io::duplex(4096);
		let target = TargetAddr::IPv4(std::net::Ipv4Addr::LOCALHOST, echo_addr.port());

		tokio::spawn(async move {
			let _ = client.handle_tcp(target, remote, None::<TuicOutbound>).await;
		});

		let msg = b"hello wind-tuic";
		local.write_all(msg).await.expect("Write to relay stream failed");

		let mut buf = vec![0u8; msg.len()];
		let read_result = tokio::time::timeout(Duration::from_secs(5), local.read_exact(&mut buf)).await;

		assert!(read_result.is_ok(), "Read timed out waiting for echo");
		assert!(read_result.unwrap().is_ok(), "read_exact failed");
		assert_eq!(&buf, msg, "Echoed payload must match sent data");
	}

	/// A large message (causing multiple QUIC sends) must be relayed correctly.
	#[tokio::test]
	async fn test_tuic_tcp_proxy_large_payload() {
		use tokio::net::TcpListener;

		let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let echo_addr = echo.local_addr().unwrap();
		tokio::spawn(async move {
			while let Ok((stream, _)) = echo.accept().await {
				tokio::spawn(async move {
					let (mut r, mut w) = stream.into_split();
					tokio::io::copy(&mut r, &mut w).await.ok();
				});
			}
		});

		let setup = setup_tuic_server().await.expect("Failed to start TUIC server");
		let client: std::sync::Arc<TuicOutbound> = connect_tuic_client(&setup).await.expect("Failed to connect TUIC client");

		let (mut local, remote) = tokio::io::duplex(65536);
		let target = TargetAddr::IPv4(std::net::Ipv4Addr::LOCALHOST, echo_addr.port());
		tokio::spawn(async move {
			let _ = client.handle_tcp(target, remote, None::<TuicOutbound>).await;
		});

		let payload: Vec<u8> = (0u8..=255).cycle().take(32 * 1024).collect();
		local.write_all(&payload).await.expect("Write failed");

		let mut received = vec![0u8; payload.len()];
		let read_result = tokio::time::timeout(Duration::from_secs(10), local.read_exact(&mut received)).await;

		assert!(read_result.is_ok(), "Large payload read timed out");
		assert!(read_result.unwrap().is_ok(), "read_exact failed for large payload");
		assert_eq!(received, payload, "Large payload echo must match");
	}

	/// PR2 regression test for log-level noise on the UDP fragmentation path.
	///
	/// Before PR2 `crates/wind-tuic/src/proto/udp_stream.rs` emitted
	/// `"Fragmentation params: ..."` once per fragmented send and
	/// `"Sending fragment N/M: K bytes"` once per fragment, both at
	/// `tracing::Level::INFO`. On a busy UDP path this was extremely noisy
	/// (allocation + formatting + I/O per packet) and obscured the genuine
	/// state-change events operators care about.
	///
	/// PR2 demoted both call sites to `tracing::Level::DEBUG`. The test uses
	/// `#[tracing_test::traced_test]` to install a global subscriber that
	/// captures every event into a process-wide buffer, scoped per-test via
	/// the test's own span (the macro enters a span named after the function;
	/// `logs_assert` filters captured lines by ` {span_name}:`). Spawned
	/// tasks across `wind-tuic`, `wind-socks`, `wind-naive`, and this file's
	/// own helpers are wrapped with `Instrument::in_current_span` so the
	/// captured buffer sees events from deeply-nested `tokio::spawn` chains.
	/// A 32 KiB UDP payload forces ~28 fragments at the 1200-byte default
	/// datagram size; the assertion then confirms no `INFO` line contains
	/// either forbidden format string and that at least one `DEBUG` line
	/// does (proving fragmentation ran and the capture pipeline is alive).
	///
	/// Note on the `no-env-filter` feature: the `#[traced_test]` macro
	/// silently ignores its attribute arguments and hardcodes the EnvFilter
	/// directive to `<test_crate>=trace` (here `wind_test=trace`), which
	/// would reject every event whose target is an identifier-style string
	/// like `"udp"` or `"tuic_out"`. Enabling `tracing-test/no-env-filter`
	/// in `Cargo.toml` switches the directive to plain `trace`, matching
	/// every event regardless of target.
	#[tracing_test::traced_test]
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_fragmented_udp_emits_no_info_log_noise() {
		// (1) Spin up a UDP echo server on loopback.
		let echo = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
		let echo_addr = echo.local_addr().unwrap();
		tokio::spawn(
			async move {
				let mut buf = vec![0u8; 65536];
				loop {
					match echo.recv_from(&mut buf).await {
						Ok((n, from)) => {
							let _ = echo.send_to(&buf[..n], from).await;
						}
						Err(_) => break,
					}
				}
			}
			.in_current_span(),
		);

		// (2) Bring up the TUIC server + client.
		let setup = setup_tuic_server().await.expect("setup tuic server");
		let client = connect_tuic_client(&setup).await.expect("connect tuic client");

		// (3) Build a wind-core UdpStream pair: the test owns one side, the
		// TUIC outbound owns the other.
		let (tx_to_client, rx_at_client) = tokio::sync::mpsc::channel::<UdpPacket>(32);
		let (tx_to_test, mut rx_at_test) = tokio::sync::mpsc::channel::<UdpPacket>(32);

		let stream_for_client = UdpStream {
			tx: tx_to_test,
			rx: rx_at_client,
		};
		let client_for_udp = client.clone();
		tokio::spawn(
			async move {
				let _ = client_for_udp.handle_udp(stream_for_client, None::<TuicOutbound>).await;
			}
			.in_current_span(),
		);

		// (4) Push a UDP packet that MUST fragment.
		//
		// macOS's default `net.inet.udp.maxdgram = 9216` limits one UDP
		// `send_to` to 9 KiB. The DirectCallback's UDP relay socket forwards
		// the reassembled payload as a single datagram to the echo server,
		// so a 32 KiB payload silently fails with EMSGSIZE on darwin and the
		// echo never roundtrips back — observed in CI. Cap at 9 KiB on
		// macOS; on every other platform keep the 32 KiB stress size so the
		// test still exercises a deep fragmentation count (~24 frags vs ~7).
		// At 9 KiB with the default 1414-byte QUIC datagram size we still
		// produce ~7 fragments, which is enough to verify the demoted-log
		// invariant.
		let payload_size = if cfg!(target_os = "macos") { 9 * 1024 } else { 32 * 1024 };
		let payload: Bytes = (0u8..=255).cycle().take(payload_size).collect::<Vec<u8>>().into();
		let target = TargetAddr::IPv4(std::net::Ipv4Addr::LOCALHOST, echo_addr.port());
		tx_to_client
			.send(UdpPacket {
				source: None,
				target: target.clone(),
				payload: payload.clone(),
			})
			.await
			.expect("send UDP packet into TUIC outbound");

		// (5) Drain the reassembled echo so we know fragmentation actually ran
		// on both sides before we examine logs.
		let echoed = tokio::time::timeout(Duration::from_secs(15), rx_at_test.recv())
			.await
			.expect("echo timed out — fragmentation roundtrip did not complete in time")
			.expect("upstream channel closed before echo arrived");
		assert_eq!(echoed.payload.len(), payload.len(), "echoed payload length mismatch");
		assert_eq!(echoed.payload, payload, "echoed payload bytes mismatch");

		// (6) Give spawned sender tasks a moment to flush any pending logs.
		tokio::time::sleep(Duration::from_millis(50)).await;

		// (7) Inspect the captured log lines. `tracing-test`'s `logs_assert`
		// filters captured lines by ` {span_name}:` (the test function name);
		// spawned tasks instrumented with `.in_current_span()` keep the
		// test's span as the current span, so their pre-formatted lines DO
		// include `test_..._noise:` and pass the filter.
		logs_assert(|lines: &[&str]| {
			for forbidden in ["Fragmentation params", "Sending fragment "] {
				if let Some(bad) = lines.iter().find(|l| l.contains(" INFO ") && l.contains(forbidden)) {
					return Err(format!(
						"PR2 demoted log '{forbidden}' is back at INFO level — found: {bad:?}"
					));
				}
			}
			if !lines.iter().any(|l| l.contains(" INFO ")) {
				return Err("sanity: no INFO events captured during the roundtrip".into());
			}
			let debug_hits = lines
				.iter()
				.filter(|l| l.contains(" DEBUG "))
				.filter(|l| l.contains("Sending fragment ") || l.contains("Fragmentation params"))
				.count();
			if debug_hits == 0 {
				return Err(
					"sanity: no DEBUG fragment events captured — fragmentation didn't run or the capture is broken".into(),
				);
			}
			Ok(())
		});
	}

	/// Graceful shutdown — idle server. Cancelling the context token must make
	/// the inbound `listen` accept-loop break, close the QUIC endpoint, and
	/// return `Ok(())` within a bounded time rather than hanging.
	#[tokio::test]
	async fn test_graceful_shutdown_idle_listen_loop_exits() {
		let (ctx, _addr, _uuid, listen) = spawn_tuic_server().await.expect("Failed to start TUIC server");

		ctx.token.cancel();

		let joined = tokio::time::timeout(Duration::from_secs(5), listen)
			.await
			.expect("listen loop did not exit within 5s of cancellation")
			.expect("listen task panicked");
		assert!(joined.is_ok(), "listen returned an error on shutdown: {:?}", joined.err());
	}

	/// Graceful shutdown — active connection. With a live client connected, the
	/// server's per-connection handler is spawned into `ctx.tasks`. After
	/// cancellation, the whole cancellation chain (listen loop →
	/// `serve_connection` → acceptor tasks) must wind down so the tracked
	/// tasks drain and the accept loop returns — all within a bounded time.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_graceful_shutdown_drains_active_connection() {
		let (ctx, addr, uuid, listen) = spawn_tuic_server().await.expect("Failed to start TUIC server");

		// Establish a connection so the server spawns a handler into ctx.tasks.
		let _client = connect_client(addr, uuid).await.expect("Failed to connect TUIC client");
		// Give the server's accept loop a moment to register the connection
		// handler in the tracker.
		tokio::time::sleep(Duration::from_millis(200)).await;

		// Trigger graceful shutdown.
		ctx.token.cancel();

		// The connection handler(s) tracked in ctx.tasks must finish promptly —
		// before the cancellation-chain fix, the acceptor tasks would keep
		// `serve_connection` alive and this would hang.
		ctx.tasks.close();
		tokio::time::timeout(Duration::from_secs(5), ctx.tasks.wait())
			.await
			.expect("tracked connection tasks did not drain within 5s of cancellation");

		// And the accept loop itself exits cleanly.
		let joined = tokio::time::timeout(Duration::from_secs(5), listen)
			.await
			.expect("listen loop did not exit within 5s of cancellation")
			.expect("listen task panicked");
		assert!(joined.is_ok(), "listen returned an error on shutdown: {:?}", joined.err());
	}

	/// Spawn a TUIC relay server bound to a fixed address with a known user;
	/// returns its context (to cancel) and the listen-loop join handle. Lets
	/// the reconnect test restart a server on the same address.
	async fn spawn_server_on(addr: SocketAddr, uuid: Uuid) -> (Arc<AppContext>, tokio::task::JoinHandle<eyre::Result<()>>) {
		let (cert, key) = generate_tuic_test_cert();
		let mut users = HashMap::new();
		users.insert(uuid, String::from_utf8_lossy(TEST_PASSWORD).to_string());

		let ctx = Arc::new(AppContext::default());
		let opts = TuicInboundOpts {
			listen_addr: addr,
			certificate: cert,
			private_key: key,
			alpn: vec!["h3".to_string()],
			users,
			auth_timeout: Duration::from_secs(5),
			max_idle_time: Duration::from_secs(30),
			zero_rtt: false,
			..Default::default()
		};
		let server = TuicInbound::new(ctx.clone(), opts);
		let callback = Arc::new(DirectCallback);
		let handle = tokio::spawn(async move { server.listen(callback.as_ref()).await }.in_current_span());
		tokio::time::sleep(Duration::from_millis(300)).await;
		(ctx, handle)
	}

	/// Attempt one proxied TCP echo through the client. Returns the echoed
	/// bytes, or an error if the relay/connection is currently unavailable
	/// (e.g. mid reconnect).
	async fn proxy_echo_once(client: &Arc<TuicOutbound>, echo_port: u16, msg: &[u8]) -> eyre::Result<Vec<u8>> {
		let (mut local, remote) = tokio::io::duplex(4096);
		let target = TargetAddr::IPv4(std::net::Ipv4Addr::LOCALHOST, echo_port);
		let c = client.clone();
		tokio::spawn(async move {
			let _ = c.handle_tcp(target, remote, None::<TuicOutbound>).await;
		});
		local.write_all(msg).await?;
		let mut buf = vec![0u8; msg.len()];
		tokio::time::timeout(Duration::from_secs(2), local.read_exact(&mut buf)).await??;
		Ok(buf)
	}

	/// The client must transparently reconnect after the relay server restarts
	/// on the same address: a new proxied request succeeds over the fresh
	/// connection even though the original connection (and its streams) were
	/// torn down.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_client_reconnects_after_server_restart() {
		use tokio::net::TcpListener;

		// Persistent echo target.
		let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let echo_port = echo.local_addr().unwrap().port();
		tokio::spawn(async move {
			while let Ok((mut s, _)) = echo.accept().await {
				tokio::spawn(async move {
					let (mut r, mut w) = s.split();
					let _ = tokio::io::copy(&mut r, &mut w).await;
				});
			}
		});

		// Reserve a fixed UDP port for the relay so the second server can rebind it.
		let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
		let server_addr = probe.local_addr().unwrap();
		drop(probe);

		let uuid = Uuid::new_v4();

		// Server #1 + client; confirm the proxy works.
		let (ctx1, handle1) = spawn_server_on(server_addr, uuid).await;
		let client = connect_client(server_addr, uuid).await.expect("connect client");
		let got = proxy_echo_once(&client, echo_port, b"before")
			.await
			.expect("initial proxied echo must succeed");
		assert_eq!(got, b"before");

		// Drop server #1 and wait for the listen loop to release the UDP port.
		ctx1.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), handle1).await;
		tokio::time::sleep(Duration::from_millis(300)).await;

		// Server #2 on the SAME address with the same credentials.
		let (ctx2, _handle2) = spawn_server_on(server_addr, uuid).await;

		// The supervisor should reconnect within a few backoff cycles; retry the
		// proxied echo until it succeeds (or give up after ~15s).
		let mut reconnected = false;
		for _ in 0..60 {
			if let Ok(got) = proxy_echo_once(&client, echo_port, b"after").await
				&& got == b"after"
			{
				reconnected = true;
				break;
			}
			tokio::time::sleep(Duration::from_millis(250)).await;
		}
		assert!(reconnected, "client did not reconnect and proxy after server restart");

		ctx2.token.cancel();
	}

	/// Shutting the client down while its supervisor is stuck in the reconnect
	/// backoff loop (server still down) must abandon reconnect and drain the
	/// tracked tasks promptly, not hang.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_client_shuts_down_cleanly_while_reconnecting() {
		let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
		let server_addr = probe.local_addr().unwrap();
		drop(probe);
		let uuid = Uuid::new_v4();

		let (ctx1, handle1) = spawn_server_on(server_addr, uuid).await;
		let client = connect_client(server_addr, uuid).await.expect("connect client");

		// Kill the server so the supervisor enters its reconnect backoff loop.
		ctx1.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), handle1).await;
		// Let the supervisor notice the drop and start retrying.
		tokio::time::sleep(Duration::from_millis(500)).await;

		// Now shut the client down. The supervisor (sharing `client.ctx`) must
		// stop reconnecting and its tracked tasks must drain.
		client.ctx.token.cancel();
		client.ctx.tasks.close();
		tokio::time::timeout(Duration::from_secs(5), client.ctx.tasks.wait())
			.await
			.expect("client tasks did not drain within 5s while reconnecting");
	}

	/// With reconnect disabled, a dropped connection is NOT re-established:
	/// after the server restarts on the same address, proxied requests keep
	/// failing.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_client_does_not_reconnect_when_disabled() {
		use tokio::net::TcpListener;

		let echo = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let echo_port = echo.local_addr().unwrap().port();
		tokio::spawn(async move {
			while let Ok((mut s, _)) = echo.accept().await {
				tokio::spawn(async move {
					let (mut r, mut w) = s.split();
					let _ = tokio::io::copy(&mut r, &mut w).await;
				});
			}
		});

		let probe = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
		let server_addr = probe.local_addr().unwrap();
		drop(probe);
		let uuid = Uuid::new_v4();

		let (ctx1, handle1) = spawn_server_on(server_addr, uuid).await;
		let client = connect_client_with(
			server_addr,
			uuid,
			ReconnectConfig {
				enabled: false,
				..Default::default()
			},
		)
		.await
		.expect("connect client");

		// Works while the original connection is alive.
		let got = proxy_echo_once(&client, echo_port, b"before")
			.await
			.expect("initial proxied echo must succeed");
		assert_eq!(got, b"before");

		// Drop server #1, restart on the same address.
		ctx1.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), handle1).await;
		tokio::time::sleep(Duration::from_millis(300)).await;
		let (ctx2, _handle2) = spawn_server_on(server_addr, uuid).await;

		// Reconnect is disabled, so no proxied echo should ever succeed even
		// though a fresh server is now listening.
		let mut recovered = false;
		for _ in 0..16 {
			if proxy_echo_once(&client, echo_port, b"after").await.is_ok() {
				recovered = true;
				break;
			}
			tokio::time::sleep(Duration::from_millis(250)).await;
		}
		assert!(!recovered, "client reconnected despite reconnect being disabled");

		ctx2.token.cancel();
	}

	/// End-to-end hooks test (quinn backend): a custom-configured TUIC server
	/// with a [`StatsCollector`] and a per-user connection-limit
	/// [`ConnectionHooks`]. After a TCP relay, the collector must show non-zero
	/// per-user upload/download (proving the QUIC-stats sampler ran) and at
	/// least one request; a second connection for the same user must be
	/// rejected.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn test_tuic_hooks_stats_and_conn_limit() {
		use std::sync::atomic::{AtomicUsize, Ordering};

		use wind_core::{ConnInfo, ConnectDecision, ConnectionHooks, InboundHooks, StatsCollector, UserId};

		// Per-user concurrent-connection limiter.
		struct Limiter {
			limit: usize,
			active: AtomicUsize,
			rejected: AtomicUsize,
		}
		#[async_trait::async_trait]
		impl ConnectionHooks for Limiter {
			async fn on_authenticated(&self, _i: &ConnInfo, _u: &UserId) -> ConnectDecision {
				if self.active.fetch_add(1, Ordering::SeqCst) + 1 > self.limit {
					self.active.fetch_sub(1, Ordering::SeqCst);
					self.rejected.fetch_add(1, Ordering::SeqCst);
					ConnectDecision::Reject("connection limit".into())
				} else {
					ConnectDecision::Accept
				}
			}

			async fn on_disconnect(&self, _i: &ConnInfo, u: Option<&UserId>) {
				if u.is_some() {
					self.active.fetch_sub(1, Ordering::SeqCst);
				}
			}
		}

		// TCP echo target.
		let echo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
		let echo_addr = echo.local_addr().unwrap();
		tokio::spawn(async move {
			while let Ok((stream, _)) = echo.accept().await {
				tokio::spawn(async move {
					let (mut r, mut w) = stream.into_split();
					tokio::io::copy(&mut r, &mut w).await.ok();
				});
			}
		});

		let stats = Arc::new(StatsCollector::new());
		let limiter = Arc::new(Limiter {
			limit: 1,
			active: AtomicUsize::new(0),
			rejected: AtomicUsize::new(0),
		});

		// Bring up a TUIC server with hooks wired in.
		let (cert, key) = generate_tuic_test_cert();
		let uuid = Uuid::new_v4();
		let mut users = HashMap::new();
		users.insert(uuid, String::from_utf8_lossy(TEST_PASSWORD).to_string());
		let temp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
		let server_addr = temp.local_addr().unwrap();
		drop(temp);

		let ctx = Arc::new(AppContext::default());
		let hooks = InboundHooks {
			stats: Some(stats.clone()),
			connection: Some(limiter.clone()),
			sample_interval: Duration::from_millis(200),
			..Default::default()
		};
		let opts = TuicInboundOpts {
			listen_addr: server_addr,
			certificate: cert,
			private_key: key,
			alpn: vec!["h3".to_string()],
			users,
			auth_timeout: Duration::from_secs(5),
			max_idle_time: Duration::from_secs(30),
			zero_rtt: false,
			hooks,
			..Default::default()
		};
		let server = TuicInbound::new(ctx.clone(), opts);
		let cb = Arc::new(DirectCallback);
		let listen = tokio::spawn(async move { server.listen(cb.as_ref()).await }.in_current_span());
		tokio::time::sleep(Duration::from_millis(300)).await;

		let connect = |reconnect: ReconnectConfig| async move {
			let cctx = Arc::new(AppContext::default());
			let opts = TuicOutboundOpts {
				peer_addr: server_addr,
				sni: "localhost".to_string(),
				auth: (uuid, Arc::from(TEST_PASSWORD)),
				zero_rtt_handshake: false,
				heartbeat: Duration::from_secs(5),
				gc_interval: Duration::from_secs(5),
				gc_lifetime: Duration::from_secs(30),
				skip_cert_verify: true,
				alpn: vec!["h3".to_string()],
				reconnect,
			};
			let c = Arc::new(TuicOutbound::new(cctx, opts).await.unwrap());
			let pc = c.clone();
			tokio::spawn(
				async move {
					let _ = pc.start_poll().await;
				}
				.in_current_span(),
			);
			tokio::time::sleep(Duration::from_millis(250)).await;
			c
		};

		// Client 1: authenticates and relays a payload through the echo target.
		let client = connect(ReconnectConfig::default()).await;
		let (mut local, remote) = tokio::io::duplex(65536);
		let target = TargetAddr::IPv4(std::net::Ipv4Addr::LOCALHOST, echo_addr.port());
		{
			let client = client.clone();
			tokio::spawn(
				async move {
					let _ = client.handle_tcp(target, remote, None::<TuicOutbound>).await;
				}
				.in_current_span(),
			);
		}
		let payload: Vec<u8> = (0u8..=255).cycle().take(32 * 1024).collect();
		local.write_all(&payload).await.unwrap();
		let mut recv = vec![0u8; payload.len()];
		tokio::time::timeout(Duration::from_secs(10), local.read_exact(&mut recv))
			.await
			.expect("relay timed out")
			.expect("relay read failed");
		assert_eq!(recv, payload, "echoed payload must match");

		// Let at least one sampler tick fold the QUIC byte counters into the collector.
		tokio::time::sleep(Duration::from_millis(500)).await;

		let user = UserId::from(uuid);
		let s = stats.snapshot_user(&user).expect("stats must be recorded for the user");
		assert!(s.request_count >= 1, "expected >=1 request, got {}", s.request_count);
		assert!(s.upload > 0, "expected upload > 0");
		assert!(s.download > 0, "expected download > 0");

		// Client 2 (same user) must be rejected by the per-user limit. Disable
		// reconnect so it doesn't spin re-auth attempts.
		let c2 = connect(ReconnectConfig {
			enabled: false,
			..ReconnectConfig::default()
		})
		.await;
		let (_l2, r2) = tokio::io::duplex(1024);
		let t2 = TargetAddr::IPv4(std::net::Ipv4Addr::LOCALHOST, echo_addr.port());
		let _ = c2.handle_tcp(t2, r2, None::<TuicOutbound>).await;
		tokio::time::sleep(Duration::from_millis(400)).await;
		assert!(
			limiter.rejected.load(Ordering::SeqCst) >= 1,
			"a second connection for the same user must be rejected by the limit hook"
		);

		ctx.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), listen).await;
	}
}
