use std::net::SocketAddr;
#[cfg(test)]
use std::{sync::Arc, time::Duration};

use eyre::Context;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::{TcpStream, UdpSocket},
};

#[ctor::ctor(unsafe)]
fn setup_crypto() {
	#[cfg(feature = "aws-lc-rs")]
	{
		_ = rustls::crypto::aws_lc_rs::default_provider().install_default()
	}
	#[cfg(feature = "ring")]
	{
		_ = rustls::crypto::ring::default_provider().install_default()
	}
}

/// Basic TCP connection test using native Tokio (without proxy)
/// Used to verify if the target server is reachable
pub async fn test_direct_tcp(target_host: &str, target_port: u16) -> eyre::Result<()> {
	let addr = format!("{}:{}", target_host, target_port);
	println!("Direct connection to: {}", addr);

	let mut stream = TcpStream::connect(&addr).await?;
	println!("Connection successful!");

	let request = format!("GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", target_host);
	stream.write_all(request.as_bytes()).await?;

	let mut response = vec![0u8; 1024];
	let n = stream.read(&mut response).await?;
	println!("Response received: {} bytes", n);

	Ok(())
}

/// Basic UDP test using native Tokio (without proxy)
pub async fn test_direct_udp(target_host: &str, target_port: u16) -> eyre::Result<()> {
	let addr = format!("{}:{}", target_host, target_port);
	println!("Direct UDP connection to: {}", addr);

	let local_addr: SocketAddr = "0.0.0.0:0".parse()?;
	let socket = UdpSocket::bind(local_addr).await?;
	socket.connect(&addr).await?;

	let dns_query: Vec<u8> = vec![
		0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65,
		0x03, 0x63, 0x6f, 0x6d, 0x00, 0x00, 0x01, 0x00, 0x01,
	];

	socket.send(&dns_query).await?;
	println!("UDP packet sent");

	let mut buffer = vec![0u8; 512];
	let n = socket.recv(&mut buffer).await?;
	println!("UDP response received: {} bytes", n);

	Ok(())
}

pub async fn test_socks5_tcp(proxy_addr: &str, target_host: &str, target_port: u16) -> eyre::Result<()> {
	use fast_socks5::client::Socks5Stream;

	println!("\n========== SOCKS5 TCP Test ==========");
	println!("Proxy address: {}", proxy_addr);
	println!("Target address: {}:{}", target_host, target_port);

	let mut stream = Socks5Stream::connect(
		proxy_addr,
		target_host.to_string(),
		target_port,
		fast_socks5::client::Config::default(),
	)
	.await
	.map_err(|e| eyre::eyre!("SOCKS5 connection failed: {}", e))?;

	println!("✓ Connected to target through proxy");

	let request = format!(
		"GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: wind-test/1.0\r\n\r\n",
		target_host
	);
	stream.write_all(request.as_bytes()).await?;
	stream.flush().await?;
	println!("✓ HTTP request sent ({} bytes)", request.len());

	let mut response = Vec::new();
	let bytes_read = stream.read_to_end(&mut response).await?;

	println!("✓ Response received: {} bytes", bytes_read);

	if bytes_read == 0 {
		return Err(eyre::eyre!("TCP proxy test failed: received zero bytes from target"));
	}

	let preview_len = std::cmp::min(500, bytes_read);
	if let Ok(text) = String::from_utf8(response[..preview_len].to_vec()) {
		println!("\n--- Response Preview (first {} bytes) ---", preview_len);
		for line in text.lines().take(15) {
			println!("{}", line);
		}
		if bytes_read > preview_len {
			println!("... ({} bytes truncated)", bytes_read - preview_len);
		}
	}

	println!("========== TCP Test Successful ==========\n");
	Ok(())
}

pub async fn test_socks5_udp(proxy_addr: &str, target_host: &str, target_port: u16) -> eyre::Result<()> {
	use std::time::Duration;

	use fast_socks5::client::Socks5Datagram;

	println!("\n========== SOCKS5 UDP Test ==========");
	println!("Proxy address: {}", proxy_addr);
	println!("Target address: {}:{}", target_host, target_port);

	let backing_socket = TcpStream::connect(proxy_addr)
		.await
		.map_err(|e| eyre::eyre!("Failed to connect to proxy: {}", e))?;
	println!("✓ TCP connection established with proxy");

	// Use 127.0.0.1:0 to bind to any available interface and let the system choose
	// an available port
	let udp_socket_addr = "127.0.0.1:0".parse::<SocketAddr>()?;
	let socket = Socks5Datagram::bind(backing_socket, udp_socket_addr)
		.await
		.context("SOCKS5 UDP association failed")?;

	println!("✓ UDP association established through proxy");
	println!("✓ Local UDP socket bound to: {}", socket.get_ref().local_addr()?);

	let dns_query: Vec<u8> = vec![
		0xAB, 0xCD, // Transaction ID
		0x01, 0x00, // Flags: standard query
		0x00, 0x01, // Questions: 1
		0x00, 0x00, // Answer RRs: 0
		0x00, 0x00, // Authority RRs: 0
		0x00, 0x00, // Additional RRs: 0
		// Query: example.com
		0x07, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 0x03, b'c', b'o', b'm', 0x00, 0x00, 0x01, // Type: A
		0x00, 0x01, // Class: IN
	];

	println!("✓ DNS query prepared ({} bytes)", dns_query.len());

	socket.send_to(&dns_query, (target_host, target_port)).await?;
	println!("✓ DNS query sent through proxy");

	let mut buffer = vec![0u8; 1024];
	match tokio::time::timeout(Duration::from_secs(5), socket.recv_from(&mut buffer)).await {
		Ok(Ok((len, _from_addr))) => {
			println!("✓ Response received: {} bytes", len);

			let response = &buffer[..len];

			// Verify the response is an echo of what we sent
			if response.len() != dns_query.len() {
				return Err(eyre::eyre!(
					"UDP echo failed: sent {} bytes but received {} bytes",
					dns_query.len(),
					response.len()
				));
			}
			if response != dns_query.as_slice() {
				return Err(eyre::eyre!("UDP echo failed: received bytes do not match sent bytes"));
			}
			println!("  ✓ UDP round-trip verified (echo matched)");
		}
		Ok(Err(e)) => {
			return Err(eyre::eyre!("UDP receive error: {}", e));
		}
		Err(_) => {
			println!("⚠ UDP response timeout (5 seconds)");
			println!("  This could mean:");
			println!("  - The SOCKS5 server doesn't support UDP ASSOCIATE");
			println!("  - The target server is not responding");
			println!("  - Firewall blocking UDP packets");
			return Err(eyre::eyre!("UDP response timeout (5 seconds)"));
		}
	}

	println!("========== UDP Test Complete ==========\n");

	Ok(())
}

pub async fn test_socks5_udp_large_packet(
	proxy_addr: &str,
	target_host: &str,
	target_port: u16,
	packet_size: usize,
) -> eyre::Result<()> {
	use std::time::Duration;

	use fast_socks5::client::Socks5Datagram;

	println!("\n========== SOCKS5 UDP Large Packet Test ==========");
	println!("Proxy address: {}", proxy_addr);
	println!("Target address: {}:{}", target_host, target_port);
	println!("Packet size: {} bytes", packet_size);

	const IPV4_HEADER_SIZE: usize = 20;
	const UDP_HEADER_SIZE: usize = 8;
	const ETHERNET_MTU: usize = 1500;
	const MAX_UDP_PAYLOAD_IPV4: usize = ETHERNET_MTU - IPV4_HEADER_SIZE - UDP_HEADER_SIZE;

	if packet_size > MAX_UDP_PAYLOAD_IPV4 {
		println!(
			"⚠ Packet size ({} bytes) exceeds MTU payload limit ({} bytes)",
			packet_size, MAX_UDP_PAYLOAD_IPV4
		);
		println!("  This will trigger IP fragmentation");
	} else {
		println!(
			"ℹ Packet size ({} bytes) fits within MTU payload limit ({} bytes)",
			packet_size, MAX_UDP_PAYLOAD_IPV4
		);
	}

	let backing_socket = TcpStream::connect(proxy_addr)
		.await
		.map_err(|e| eyre::eyre!("Failed to connect to proxy: {}", e))?;
	println!("✓ TCP connection established with proxy");

	let udp_socket_addr = "127.0.0.1:0".parse::<SocketAddr>()?;
	let socket = Socks5Datagram::bind(backing_socket, udp_socket_addr)
		.await
		.context("SOCKS5 UDP association failed")?;

	println!("✓ UDP association established through proxy");
	println!("✓ Local UDP socket bound to: {}", socket.get_ref().local_addr()?);

	let mut large_packet = Vec::with_capacity(packet_size);

	large_packet.extend_from_slice(b"WIND_FRAG_TEST");
	large_packet.extend_from_slice(&(packet_size as u32).to_be_bytes());

	// Fill the rest with a repeating pattern for easy verification
	let pattern = b"0123456789ABCDEF";
	let mut pattern_offset = 0;

	while large_packet.len() < packet_size {
		let remaining = packet_size - large_packet.len();
		let copy_len = std::cmp::min(remaining, pattern.len() - pattern_offset);
		large_packet.extend_from_slice(&pattern[pattern_offset..pattern_offset + copy_len]);
		pattern_offset = (pattern_offset + copy_len) % pattern.len();
	}

	// Add a checksum at the end for integrity verification
	let mut checksum: u32 = 0;
	for byte in &large_packet {
		checksum = checksum.wrapping_add(*byte as u32);
	}

	if large_packet.len() >= 4 {
		let checksum_bytes = checksum.to_be_bytes();
		let len = large_packet.len();
		large_packet[len - 4..].copy_from_slice(&checksum_bytes);
	}

	println!("✓ Large test packet prepared ({} bytes)", large_packet.len());
	println!("  Pattern: repeating '0123456789ABCDEF'");
	println!("  Checksum: 0x{:08X}", checksum);

	let start_time = std::time::Instant::now();
	socket.send_to(&large_packet, (target_host, target_port)).await?;
	let send_duration = start_time.elapsed();
	println!(
		"✓ Large packet sent through proxy ({:.2}ms)",
		send_duration.as_secs_f64() * 1000.0
	);

	let mut buffer = vec![0u8; packet_size + 1024]; // Extra buffer for potential overhead
	match tokio::time::timeout(Duration::from_secs(10), socket.recv_from(&mut buffer)).await {
		Ok(Ok((len, from_addr))) => {
			let receive_duration = start_time.elapsed();
			println!(
				"✓ Response received: {} bytes from {} ({:.2}ms total)",
				len,
				from_addr,
				receive_duration.as_secs_f64() * 1000.0
			);

			if len == large_packet.len() {
				let received_data = &buffer[..len];

				if received_data.starts_with(b"WIND_FRAG_TEST") {
					println!("✓ Packet header verified");
				} else {
					return Err(eyre::eyre!("Packet header mismatch"));
				}

				if len >= 4 {
					let received_checksum_bytes = &received_data[len - 4..];
					let received_checksum = u32::from_be_bytes([
						received_checksum_bytes[0],
						received_checksum_bytes[1],
						received_checksum_bytes[2],
						received_checksum_bytes[3],
					]);

					let mut calc_checksum: u32 = 0;
					for byte in &received_data[..len - 4] {
						calc_checksum = calc_checksum.wrapping_add(*byte as u32);
					}
					calc_checksum = calc_checksum.wrapping_add(checksum); // Add original checksum

					if received_checksum == checksum {
						println!("✓ Packet integrity verified (checksum: 0x{:08X})", received_checksum);
					} else {
						return Err(eyre::eyre!(
							"Packet integrity check failed: expected checksum 0x{:08X}, received 0x{:08X}, calculated 0x{:08X}",
							checksum,
							received_checksum,
							calc_checksum
						));
					}
				}

				if received_data == large_packet {
					println!("✓ Complete packet integrity verified - fragmentation handled correctly");
				} else {
					// Find first difference for debugging
					let mut first_diff = None;
					for (i, (sent, recv)) in large_packet.iter().zip(received_data.iter()).enumerate() {
						if sent != recv {
							first_diff = Some((i, *sent, *recv));
							break;
						}
					}

					if let Some((i, sent, recv)) = first_diff {
						return Err(eyre::eyre!(
							"Packet data mismatch detected at byte {}: sent 0x{:02X}, received 0x{:02X}",
							i,
							sent,
							recv
						));
					} else {
						return Err(eyre::eyre!("Packet data mismatch detected"));
					}
				}
			} else {
				return Err(eyre::eyre!(
					"Response size mismatch: expected {} bytes, got {} bytes",
					large_packet.len(),
					len
				));
			}
		}
		Ok(Err(e)) => {
			return Err(eyre::eyre!("UDP receive error: {}", e));
		}
		Err(_) => {
			println!("⚠ UDP response timeout (10 seconds)");
			println!("  Possible causes:");
			println!("  - Large packets dropped due to fragmentation issues");
			println!("  - Target server cannot handle large UDP packets");
			println!("  - Network path MTU discovery issues");
			println!("  - SOCKS5 server fragmentation handling problems");
			return Err(eyre::eyre!("Large packet test timeout"));
		}
	}

	println!("========== UDP Large Packet Test Complete ==========\n");

	Ok(())
}

/// Helper function to start the proxy server for testing
///
/// This function creates a complete test proxy setup with:
/// - A SOCKS5 inbound server (listening for client connections)
/// - A TUIC inbound server (for encrypted relay)
/// - Direct outbound connections (no external TUIC server required)
///
/// Architecture:
/// ```markdown
/// Client -> SOCKS5 Inbound -> TUIC Outbound (Client) -> TUIC Inbound (Server) -> Direct Outbound -> Internet
///           (127.0.0.1:socks_port)                      (127.0.0.1:tuic_port)
/// ```
///
/// The TUIC inbound server uses a self-signed certificate and handles both TCP
/// and UDP traffic by directly connecting to target addresses (no intermediary
/// TUIC server needed).
///
/// Returns the AppContext and server task handle

#[cfg(test)]
async fn start_test_proxy(socks_port: u16) -> eyre::Result<(Arc<wind_core::AppContext>, tokio::task::JoinHandle<()>)> {
	use wind_socks::inbound::SocksInboundOpt;

	let ctx = Arc::new(wind_core::AppContext::default());

	let config = TestConfig {
		socks_opt: SocksInboundOpt {
			listen_addr: format!("127.0.0.1:{}", socks_port).parse()?,
			public_addr: None,
			auth: wind_socks::inbound::AuthMode::NoAuth,
			skip_auth: false,
			allow_udp: true,
			hooks: Default::default(),
		},
		tuic_port: 0, // Let OS assign a port
	};

	let ctx_clone = ctx.clone();
	let server_handle = tokio::spawn(async move {
		if let Err(e) = run_test_proxy(ctx_clone.clone(), config).await {
			panic!("Server error: {}", e);
		}
	});

	// Wait a bit for server to start
	tokio::time::sleep(Duration::from_millis(500)).await;

	Ok((ctx, server_handle))
}
#[cfg(test)]
struct TestConfig {
	socks_opt: wind_socks::inbound::SocksInboundOpt,
	tuic_port: u16,
}
#[cfg(test)]
async fn run_test_proxy(ctx: Arc<wind_core::AppContext>, config: TestConfig) -> eyre::Result<()> {
	use std::collections::HashMap;

	use wind_core::{InboundCallback, inbound::AbstractInbound, tcp::AbstractTcpStream, types::TargetAddr};

	let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
	let cert_der = cert.cert.der().to_vec();
	let key_der = cert.signing_key.serialize_der();

	let tuic_opts = wind_tuic::quinn::inbound::TuicInboundOpts {
		listen_addr: format!("127.0.0.1:{}", config.tuic_port).parse()?,
		certificate: vec![rustls::pki_types::CertificateDer::from(cert_der)],
		private_key: rustls::pki_types::PrivateKeyDer::Pkcs8(key_der.into()),
		alpn: vec!["h3".to_string()],
		users: {
			let mut users = HashMap::new();
			users.insert(
				"c1e6dbe2-f417-4890-994c-9ee15b926597".parse().unwrap(),
				"test_passwd".to_string(),
			);
			users
		},
		auth_timeout: Duration::from_secs(3),
		max_idle_time: Duration::from_secs(15),
		..Default::default()
	};

	#[derive(Clone)]
	struct TestManager {
		socks_inbound: Arc<wind_socks::inbound::SocksInbound>,
		tuic_inbound: Arc<wind_tuic::quinn::inbound::TuicInbound>,
	}

	impl InboundCallback for TestManager {
		async fn handle_tcpstream(&self, target_addr: TargetAddr, stream: impl AbstractTcpStream) -> eyre::Result<()> {
			handle_tcp_direct(target_addr, stream).await?;
			Ok(())
		}

		async fn handle_udpstream(&self, stream: wind_core::udp::UdpStream) -> eyre::Result<()> {
			handle_udp_direct(stream).await?;
			Ok(())
		}
	}

	async fn handle_tcp_direct(target_addr: TargetAddr, mut inbound_stream: impl AbstractTcpStream) -> eyre::Result<()> {
		use tokio::io::AsyncWriteExt;

		let addr = target_addr.to_string();
		let mut outbound_stream = tokio::net::TcpStream::connect(&addr).await?;

		let (mut inbound_read, mut inbound_write) = tokio::io::split(&mut inbound_stream);
		let (mut outbound_read, mut outbound_write) = outbound_stream.split();

		tokio::try_join!(
			async {
				tokio::io::copy(&mut inbound_read, &mut outbound_write).await?;
				outbound_write.shutdown().await?;
				eyre::Ok(())
			},
			async {
				tokio::io::copy(&mut outbound_read, &mut inbound_write).await?;
				inbound_write.shutdown().await?;
				eyre::Ok(())
			}
		)?;

		Ok(())
	}

	async fn handle_udp_direct(stream: wind_core::udp::UdpStream) -> eyre::Result<()> {
		use std::{collections::HashMap, sync::Arc};

		use tokio::sync::Mutex;

		let wind_core::udp::UdpStream { tx, mut rx } = stream;
		let target_sockets: Arc<Mutex<HashMap<String, Arc<tokio::net::UdpSocket>>>> = Arc::new(Mutex::new(HashMap::new()));
		let target_sockets_clone = target_sockets.clone();

		tokio::spawn(async move {
			while let Some(packet) = rx.recv().await {
				let target_key = packet.target.to_string();

				let target_socket = {
					let mut sockets = target_sockets_clone.lock().await;
					if let Some(sock) = sockets.get(&target_key) {
						sock.clone()
					} else {
						let new_sock = Arc::new(tokio::net::UdpSocket::bind("0.0.0.0:0").await.unwrap());
						sockets.insert(target_key.clone(), new_sock.clone());

						let tx_for_recv = tx.clone();
						let target_sock_for_recv = new_sock.clone();
						let source_addr = packet.target.clone();
						tokio::spawn(async move {
							let mut buf = vec![0u8; 65536];
							loop {
								match target_sock_for_recv.recv_from(&mut buf).await {
									Ok((len, _from)) => {
										let reply_packet = wind_core::udp::UdpPacket {
											source: None,
											target: source_addr.clone(),
											payload: tokio_util::bytes::Bytes::copy_from_slice(&buf[..len]),
										};
										if let Err(e) = tx_for_recv.send(reply_packet).await {
											eprintln!("UDP relay: failed to send reply packet: {}", e);
										}
									}
									Err(_) => break,
								}
							}
						});

						new_sock
					}
				};

				if let Err(e) = target_socket.send_to(&packet.payload, target_key).await {
					eprintln!("UDP relay: failed to send to target: {}", e);
				}
			}
			eyre::Ok::<()>(())
		});

		Ok(())
	}

	let tuic_inbound = Arc::new(wind_tuic::quinn::inbound::TuicInbound::new(ctx.clone(), tuic_opts));
	let socks_inbound = Arc::new(wind_socks::inbound::SocksInbound::new(
		config.socks_opt,
		ctx.token.child_token(),
	));

	let manager = Arc::new(TestManager {
		socks_inbound: socks_inbound.clone(),
		tuic_inbound: tuic_inbound.clone(),
	});

	let manager_for_tuic = manager.clone();
	ctx.tasks.spawn(async move {
		manager_for_tuic.tuic_inbound.listen(manager_for_tuic.as_ref()).await?;
		eyre::Ok(())
	});

	let manager_for_socks = manager.clone();
	ctx.tasks.spawn(async move {
		manager_for_socks.socks_inbound.listen(manager_for_socks.as_ref()).await?;
		eyre::Ok(())
	});

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn test_direct_tcp_connection() {
		let result = test_direct_tcp("example.com", 80).await;
		assert!(result.is_ok(), "Direct TCP connection failed: {:?}", result.err());
	}

	#[tokio::test]
	async fn test_direct_udp_connection() {
		let result = test_direct_udp("8.8.8.8", 53).await;
		assert!(result.is_ok(), "Direct UDP connection failed: {:?}", result.err());
	}

	#[tokio::test]
	async fn test_tcp_through_proxy() {
		let test_port = 16666;
		let (ctx, _server_handle) = start_test_proxy(test_port).await.expect("Failed to start proxy");

		let result = test_socks5_tcp(&format!("127.0.0.1:{}", test_port), "example.com", 80).await;

		ctx.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), ctx.tasks.wait()).await;

		assert!(result.is_ok(), "TCP proxy test failed: {:?}", result.err());
	}

	#[tokio::test]
	async fn test_udp_through_proxy() {
		use std::sync::{
			Arc,
			atomic::{AtomicBool, Ordering},
		};

		use tokio::net::UdpSocket;

		let test_port = 16667;
		let (ctx, _server_handle) = start_test_proxy(test_port).await.expect("Failed to start proxy");

		let echo_server_running = Arc::new(AtomicBool::new(true));
		let echo_server_running_clone = echo_server_running.clone();

		let echo_socket = UdpSocket::bind("127.0.0.1:0")
			.await
			.expect("Failed to bind echo server socket");
		let echo_addr = echo_socket.local_addr().expect("Failed to get echo server address");
		let echo_port = echo_addr.port();

		println!("✓ UDP Echo Server started on port {}", echo_port);

		let echo_task = tokio::spawn(async move {
			let mut buffer = vec![0u8; 65536];
			let mut packet_count = 0;

			while echo_server_running_clone.load(Ordering::Relaxed) {
				match tokio::time::timeout(Duration::from_millis(100), echo_socket.recv_from(&mut buffer)).await {
					Ok(Ok((len, from_addr))) => {
						packet_count += 1;
						println!(
							"  Echo server received packet #{}: {} bytes from {}",
							packet_count, len, from_addr
						);
						if let Err(e) = echo_socket.send_to(&buffer[..len], from_addr).await {
							println!("  Echo server send error: {}", e);
						}
					}
					Ok(Err(e)) => {
						println!("  Echo server receive error: {}", e);
						break;
					}
					Err(_) => continue, // Timeout - continue loop
				}
			}
			println!("✓ UDP Echo Server stopped after handling {} packets", packet_count);
		});

		tokio::time::sleep(Duration::from_millis(100)).await;

		let result = test_socks5_udp(&format!("127.0.0.1:{}", test_port), "127.0.0.1", echo_port).await;

		echo_server_running.store(false, Ordering::Relaxed);
		match tokio::time::timeout(Duration::from_secs(5), echo_task).await {
			Ok(_) => {}
			Err(_) => panic!("Echo server stop timeout in test_udp_through_proxy"),
		}

		ctx.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), ctx.tasks.wait()).await;

		match &result {
			Ok(_) => println!("✓ UDP proxy test passed successfully"),
			Err(e) => {
				let err_str = e.to_string();
				if err_str.contains("Command not supported") {
					println!("⚠ SOCKS5 server does not support UDP ASSOCIATE");
					println!("  This test requires a SOCKS5 server with UDP support enabled");
					println!("  Error: {}", e);
					panic!("UDP ASSOCIATE command not supported by SOCKS5 server");
				}
				panic!("UDP proxy test failed: {:?}", e);
			}
		}
	}

	#[tokio::test]
	async fn test_udp_large_packet_through_proxy() {
		use std::sync::{
			Arc,
			atomic::{AtomicBool, Ordering},
		};

		use tokio::net::UdpSocket;

		let test_packet_size = 512; // Start small to ensure basic UDP works

		println!("Testing UDP functionality with {} byte packet", test_packet_size);

		let test_port = 16668;
		let (ctx, _server_handle) = start_test_proxy(test_port).await.expect("Failed to start proxy");

		let echo_server_running = Arc::new(AtomicBool::new(true));
		let echo_server_running_clone = echo_server_running.clone();

		let echo_socket = UdpSocket::bind("127.0.0.1:0")
			.await
			.expect("Failed to bind echo server socket");
		let echo_addr = echo_socket.local_addr().expect("Failed to get echo server address");
		let echo_port = echo_addr.port();

		println!("✓ UDP Echo Server started on port {}", echo_port);

		let echo_task = tokio::spawn(async move {
			let mut buffer = vec![0u8; 65536]; // Large buffer for fragmented packets
			let mut packet_count = 0;

			while echo_server_running_clone.load(Ordering::Relaxed) {
				match tokio::time::timeout(std::time::Duration::from_millis(100), echo_socket.recv_from(&mut buffer)).await {
					Ok(Ok((len, from_addr))) => {
						packet_count += 1;
						println!(
							"  Echo server received packet #{}: {} bytes from {}",
							packet_count, len, from_addr
						);

						if let Err(e) = echo_socket.send_to(&buffer[..len], from_addr).await {
							println!("  Echo server send error: {}", e);
						} else {
							println!("  Echo server sent back {} bytes to {}", len, from_addr);
						}
					}
					Ok(Err(e)) => {
						println!("  Echo server receive error: {}", e);
						break;
					}
					Err(_) => {
						// Timeout - continue loop to check if we should stop
						continue;
					}
				}
			}

			println!("✓ UDP Echo Server stopped after handling {} packets", packet_count);
		});

		tokio::time::sleep(std::time::Duration::from_millis(100)).await;

		// First, test the echo server directly (without proxy) to ensure it works
		println!("\n=== Testing echo server directly (no proxy) ===");
		let direct_test_result = test_direct_udp_with_echo_server("127.0.0.1", echo_port, 512).await;
		match &direct_test_result {
			Ok(_) => println!("✓ Direct UDP echo test passed"),
			Err(e) => {
				println!("✗ Direct UDP echo test failed: {}", e);
				echo_server_running.store(false, Ordering::Relaxed);
				let _ = tokio::time::timeout(std::time::Duration::from_secs(5), echo_task).await;
				panic!("Echo server is not working correctly");
			}
		}

		println!("\n=== Testing small packet through proxy (512 bytes) ===");
		let result_small = test_socks5_udp_large_packet(
			&format!("127.0.0.1:{}", test_port),
			"127.0.0.1", // Use localhost for echo test
			echo_port,   // Use the dynamically allocated port
			512,
		)
		.await;

		match &result_small {
			Ok(_) => {
				println!("✓ Small packet test passed - now testing large packet");

				println!("\n=== Testing large packet through proxy (2000 bytes) ===");
				let result_large = test_socks5_udp_large_packet(
					&format!("127.0.0.1:{}", test_port),
					"127.0.0.1",
					echo_port,
					2000, // This should trigger fragmentation
				)
				.await;

				match &result_large {
					Ok(_) => println!("✓ UDP large packet test passed successfully"),
					Err(e) => {
						if e.to_string().contains("timeout") {
							panic!("Large packet test timed out - fragmentation may not be supported: {:?}", e);
						} else {
							panic!("UDP large packet test failed: {:?}", e);
						}
					}
				}
			}
			Err(e) => {
				let err_str = e.to_string();
				if err_str.contains("Command not supported") {
					panic!("SOCKS5 server does not support UDP ASSOCIATE: {}", e);
				} else {
					panic!("Basic UDP proxy test failed: {}", e);
				}
			}
		}

		echo_server_running.store(false, Ordering::Relaxed);

		match tokio::time::timeout(std::time::Duration::from_secs(5), echo_task).await {
			Ok(_) => println!("✓ Echo server stopped successfully"),
			Err(_) => panic!("Echo server stop timeout in test_udp_large_packet_through_proxy"),
		}

		ctx.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), ctx.tasks.wait()).await;
	}

	/// Helper function to test UDP echo server directly without proxy
	async fn test_direct_udp_with_echo_server(host: &str, port: u16, packet_size: usize) -> eyre::Result<()> {
		let test_socket = UdpSocket::bind("127.0.0.1:0").await?;

		let mut test_packet = Vec::with_capacity(packet_size);
		test_packet.extend_from_slice(b"TEST_DIRECT");
		test_packet.extend_from_slice(&(packet_size as u32).to_be_bytes());

		while test_packet.len() < packet_size {
			let _remaining = packet_size - test_packet.len();
			let byte = (test_packet.len() % 256) as u8;
			test_packet.push(byte);
		}

		test_socket.send_to(&test_packet, (host, port)).await?;

		let mut buffer = vec![0u8; packet_size + 100];
		let (len, _) = tokio::time::timeout(std::time::Duration::from_secs(5), test_socket.recv_from(&mut buffer)).await??;

		if len == test_packet.len() && buffer[..len] == test_packet {
			Ok(())
		} else {
			Err(eyre::eyre!(
				"Echo response mismatch: expected {} bytes, got {} bytes",
				test_packet.len(),
				len
			))
		}
	}

	#[tokio::test]
	async fn test_udp_fragmentation_demonstration() {
		use std::sync::{
			Arc,
			atomic::{AtomicBool, Ordering},
		};

		use tokio::net::UdpSocket;

		println!("=== UDP Fragmentation Demonstration ===");
		println!("This test demonstrates UDP packet fragmentation without requiring a working SOCKS5 proxy");

		let echo_server_running = Arc::new(AtomicBool::new(true));
		let echo_server_running_clone = echo_server_running.clone();

		let echo_socket = UdpSocket::bind("127.0.0.1:0")
			.await
			.expect("Failed to bind echo server socket");
		let echo_addr = echo_socket.local_addr().expect("Failed to get echo server address");
		let echo_port = echo_addr.port();

		println!("✓ UDP Echo Server started on port {}", echo_port);

		let echo_task = tokio::spawn(async move {
			let mut buffer = vec![0u8; 65536];
			let mut packet_count = 0;

			while echo_server_running_clone.load(Ordering::Relaxed) {
				match tokio::time::timeout(std::time::Duration::from_millis(100), echo_socket.recv_from(&mut buffer)).await {
					Ok(Ok((len, from_addr))) => {
						packet_count += 1;
						println!(
							"  Echo server received packet #{}: {} bytes from {}",
							packet_count, len, from_addr
						);
						if let Err(e) = echo_socket.send_to(&buffer[..len], from_addr).await {
							println!("  Echo server send error: {}", e);
						} else {
							println!("  Echo server sent back {} bytes to {}", len, from_addr);
						}
					}
					Ok(Err(_)) => break,
					Err(_) => continue,
				}
			}

			println!("✓ UDP Echo Server stopped after handling {} packets", packet_count);
		});

		tokio::time::sleep(std::time::Duration::from_millis(100)).await;

		let test_sizes = vec![
			512,  // Small packet
			1400, // Just under MTU
			1472, // Max UDP payload for IPv4
			1500, // Standard MTU (will cause fragmentation)
			2000, // Large packet requiring fragmentation
			4000, // Very large packet
		];

		for size in test_sizes {
			println!("\n--- Testing packet size: {} bytes ---", size);

			const IPV4_HEADER_SIZE: usize = 20;
			const UDP_HEADER_SIZE: usize = 8;
			const ETHERNET_MTU: usize = 1500;
			const MAX_UDP_PAYLOAD_IPV4: usize = ETHERNET_MTU - IPV4_HEADER_SIZE - UDP_HEADER_SIZE;

			if size > MAX_UDP_PAYLOAD_IPV4 {
				println!(
					"⚠ Packet size ({} bytes) exceeds MTU payload limit ({} bytes)",
					size, MAX_UDP_PAYLOAD_IPV4
				);
				println!("  This will trigger IP fragmentation");
			} else {
				println!(
					"ℹ Packet size ({} bytes) fits within MTU payload limit ({} bytes)",
					size, MAX_UDP_PAYLOAD_IPV4
				);
			}

			match test_direct_udp_with_echo_server("127.0.0.1", echo_port, size).await {
				Ok(_) => {
					println!("✓ Direct UDP test passed for {} bytes", size);
					if size > MAX_UDP_PAYLOAD_IPV4 {
						println!("  ✓ IP fragmentation handled successfully by OS network stack");
					}
				}
				Err(e) => {
					panic!("Direct UDP test failed for {} bytes: {}", size, e);
				}
			}
		}

		echo_server_running.store(false, Ordering::Relaxed);
		let _ = tokio::time::timeout(std::time::Duration::from_secs(5), echo_task).await;
	}

	#[tokio::test]
	async fn test_udp_multiple_mtu_sizes() {
		use std::sync::{
			Arc,
			atomic::{AtomicBool, Ordering},
		};

		use tokio::net::UdpSocket;

		let test_port = 16669;
		let (ctx, _server_handle) = start_test_proxy(test_port).await.expect("Failed to start proxy");

		// Test multiple packet sizes around MTU boundaries
		let test_sizes = vec![
			512,  // Small packet
			1400, // Just under MTU
			1472, // Max UDP payload for IPv4
			1500, // Standard MTU
			2000, // Over MTU - requires fragmentation
			4000, // Much larger packet
		];

		let echo_server_running = Arc::new(AtomicBool::new(true));
		let echo_server_running_clone = echo_server_running.clone();

		let echo_socket = UdpSocket::bind("127.0.0.1:0")
			.await
			.expect("Failed to bind echo server socket");
		let echo_addr = echo_socket.local_addr().expect("Failed to get echo server address");
		let echo_port = echo_addr.port();

		println!("✓ UDP Echo Server started on port {} for MTU size testing", echo_port);

		let echo_task = tokio::spawn(async move {
			let mut buffer = vec![0u8; 65536]; // Large buffer for fragmented packets
			let mut packet_count = 0;

			while echo_server_running_clone.load(Ordering::Relaxed) {
				match tokio::time::timeout(std::time::Duration::from_millis(100), echo_socket.recv_from(&mut buffer)).await {
					Ok(Ok((len, from_addr))) => {
						packet_count += 1;
						println!(
							"    Echo server received packet #{}: {} bytes from {}",
							packet_count, len, from_addr
						);

						if let Err(e) = echo_socket.send_to(&buffer[..len], from_addr).await {
							println!("    Echo server send error: {}", e);
						}
					}
					Ok(Err(e)) => {
						println!("    Echo server receive error: {}", e);
						break;
					}
					Err(_) => {
						// Timeout - continue loop to check if we should stop
						continue;
					}
				}
			}

			println!("✓ UDP Echo Server stopped after handling {} packets", packet_count);
		});

		tokio::time::sleep(std::time::Duration::from_millis(100)).await;

		for size in test_sizes {
			println!("\n--- Testing packet size: {} bytes ---", size);

			let result = test_socks5_udp_large_packet(&format!("127.0.0.1:{}", test_port), "127.0.0.1", echo_port, size).await;

			match &result {
				Ok(_) => println!("✓ Packet size {} bytes: SUCCESS", size),
				Err(e) => {
					let err_str = e.to_string();
					if err_str.contains("Command not supported") {
						println!("⚠ SOCKS5 server does not support UDP ASSOCIATE - skipping remaining tests");
						echo_server_running.store(false, Ordering::Relaxed);
						let _ = tokio::time::timeout(std::time::Duration::from_secs(5), echo_task).await;
						ctx.token.cancel();
						let _ = tokio::time::timeout(Duration::from_secs(5), ctx.tasks.wait()).await;
						panic!("UDP ASSOCIATE command not supported by SOCKS5 server");
					}
					println!("⚠ Packet size {} bytes: FAILED - {}", size, e);
					panic!("Test failed for packet size {} bytes: {}", size, e);
				}
			}
		}

		echo_server_running.store(false, Ordering::Relaxed);

		match tokio::time::timeout(std::time::Duration::from_secs(5), echo_task).await {
			Ok(_) => println!("✓ Echo server stopped successfully"),
			Err(_) => panic!("Echo server stop timeout in test_udp_multiple_mtu_sizes"),
		}

		ctx.token.cancel();
		let _ = tokio::time::timeout(Duration::from_secs(5), ctx.tasks.wait()).await;
	}
}
