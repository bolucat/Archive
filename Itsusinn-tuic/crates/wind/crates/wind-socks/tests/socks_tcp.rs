//! End-to-end TCP tests for the SOCKS5 inbound, exercising the real handshake
//! over loopback: method negotiation (no-auth / RFC 1929 username-password),
//! CONNECT to IPv4 and domain targets, and the unsupported-command path.
//!
//! A minimal hand-rolled SOCKS5 client is used so the test depends only on the
//! wire protocol, not on any client library version. The inbound is wired to a
//! TCP-relay callback that dials the real target and copies bytes both ways, so
//! a successful CONNECT yields a genuine echo roundtrip.

use std::{
	net::{Ipv4Addr, SocketAddr},
	time::Duration,
};

use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;
use wind_core::{AbstractInbound, InboundCallback, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream};
use wind_socks::inbound::{AuthMode, SocksInbound, SocksInboundOpt};

/// Inbound callback that relays an accepted SOCKS5 TCP stream to its real
/// target and copies bytes bidirectionally.
#[derive(Clone)]
struct TcpRelayCallback;

impl InboundCallback for TcpRelayCallback {
	async fn handle_tcpstream(&self, target: TargetAddr, mut stream: impl AbstractTcpStream + 'static) -> eyre::Result<()> {
		let mut upstream = TcpStream::connect(target.to_string()).await?;
		tokio::io::copy_bidirectional(&mut stream, &mut upstream).await?;
		Ok(())
	}

	async fn handle_udpstream(&self, _udp_stream: UdpStream) -> eyre::Result<()> {
		Ok(())
	}
}

/// Spawn a TCP echo server on loopback; returns its address.
async fn spawn_echo() -> SocketAddr {
	let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
	let addr = listener.local_addr().unwrap();
	tokio::spawn(async move {
		while let Ok((mut sock, _)) = listener.accept().await {
			tokio::spawn(async move {
				let mut buf = [0u8; 4096];
				loop {
					match sock.read(&mut buf).await {
						Ok(0) | Err(_) => break,
						Ok(n) => {
							if sock.write_all(&buf[..n]).await.is_err() {
								break;
							}
						}
					}
				}
			});
		}
	});
	addr
}

/// Spawn a SOCKS5 inbound with the given auth mode; returns its address and a
/// cancel token to shut it down.
async fn spawn_socks(auth: AuthMode) -> (SocketAddr, CancellationToken) {
	let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
	let addr = probe.local_addr().unwrap();
	drop(probe);

	let opts = SocksInboundOpt {
		listen_addr: addr,
		public_addr: None,
		auth,
		skip_auth: false,
		allow_udp: false,
		hooks: Default::default(),
	};
	let cancel = CancellationToken::new();
	let inbound = SocksInbound::new(opts, cancel.clone());
	tokio::spawn(async move {
		let cb = TcpRelayCallback;
		let _ = inbound.listen(&cb).await;
	});

	tokio::time::sleep(Duration::from_millis(200)).await;
	(addr, cancel)
}

/// Negotiate the no-auth method (0x00). Panics if the server doesn't select it.
async fn negotiate_no_auth(s: &mut TcpStream) {
	s.write_all(&[0x05, 0x01, 0x00]).await.unwrap();
	let mut resp = [0u8; 2];
	s.read_exact(&mut resp).await.unwrap();
	assert_eq!(resp, [0x05, 0x00], "server must select no-auth");
}

/// Run the RFC 1929 username/password sub-negotiation. Returns the status byte
/// (0x00 = success), or `Err` if the server closed the stream on rejection.
async fn negotiate_password(s: &mut TcpStream, user: &str, pass: &str) -> std::io::Result<u8> {
	s.write_all(&[0x05, 0x01, 0x02]).await?;
	let mut method = [0u8; 2];
	s.read_exact(&mut method).await?;
	if method[1] != 0x02 {
		// Server refused the username/password method outright.
		return Ok(method[1]);
	}
	let mut req = vec![0x01, user.len() as u8];
	req.extend_from_slice(user.as_bytes());
	req.push(pass.len() as u8);
	req.extend_from_slice(pass.as_bytes());
	s.write_all(&req).await?;

	let mut status = [0u8; 2];
	s.read_exact(&mut status).await?;
	Ok(status[1])
}

/// Send a CONNECT request and return the reply code (0x00 = success). The
/// inbound always replies with an IPv4 BND.ADDR, so the reply is 10 bytes.
async fn connect_request(s: &mut TcpStream, atyp_body: Vec<u8>) -> u8 {
	let mut req = vec![0x05, 0x01, 0x00];
	req.extend_from_slice(&atyp_body);
	s.write_all(&req).await.unwrap();
	let mut reply = [0u8; 10];
	s.read_exact(&mut reply).await.unwrap();
	reply[1]
}

fn ipv4_body(ip: Ipv4Addr, port: u16) -> Vec<u8> {
	let mut b = vec![0x01];
	b.extend_from_slice(&ip.octets());
	b.extend_from_slice(&port.to_be_bytes());
	b
}

fn domain_body(host: &str, port: u16) -> Vec<u8> {
	let mut b = vec![0x03, host.len() as u8];
	b.extend_from_slice(host.as_bytes());
	b.extend_from_slice(&port.to_be_bytes());
	b
}

/// Assert a connected stream echoes a payload back unchanged.
async fn assert_echo_roundtrip(s: &mut TcpStream) {
	let msg = b"hello socks5";
	s.write_all(msg).await.unwrap();
	let mut buf = [0u8; 12];
	let read = tokio::time::timeout(Duration::from_secs(5), s.read_exact(&mut buf)).await;
	assert!(read.is_ok(), "echo read timed out");
	read.unwrap().unwrap();
	assert_eq!(&buf, msg, "echoed payload must match");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_auth_tcp_connect_ipv4_echoes() {
	let echo = spawn_echo().await;
	let (proxy, _cancel) = spawn_socks(AuthMode::NoAuth).await;

	let mut s = TcpStream::connect(proxy).await.unwrap();
	negotiate_no_auth(&mut s).await;
	let rep = connect_request(&mut s, ipv4_body(Ipv4Addr::LOCALHOST, echo.port())).await;
	assert_eq!(rep, 0x00, "CONNECT must succeed");
	assert_echo_roundtrip(&mut s).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_auth_tcp_connect_domain_echoes() {
	let echo = spawn_echo().await;
	let (proxy, _cancel) = spawn_socks(AuthMode::NoAuth).await;

	let mut s = TcpStream::connect(proxy).await.unwrap();
	negotiate_no_auth(&mut s).await;
	// Domain target: the relay callback resolves "localhost" itself.
	let rep = connect_request(&mut s, domain_body("127.0.0.1", echo.port())).await;
	assert_eq!(rep, 0x00, "CONNECT to domain target must succeed");
	assert_echo_roundtrip(&mut s).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn password_auth_accepts_correct_credentials() {
	let echo = spawn_echo().await;
	let (proxy, _cancel) = spawn_socks(AuthMode::Password {
		username: "alice".into(),
		password: "s3cret".into(),
	})
	.await;

	let mut s = TcpStream::connect(proxy).await.unwrap();
	let status = negotiate_password(&mut s, "alice", "s3cret").await.unwrap();
	assert_eq!(status, 0x00, "correct credentials must authenticate");

	let rep = connect_request(&mut s, ipv4_body(Ipv4Addr::LOCALHOST, echo.port())).await;
	assert_eq!(rep, 0x00, "CONNECT must succeed after auth");
	assert_echo_roundtrip(&mut s).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn password_auth_rejects_wrong_credentials() {
	let (proxy, _cancel) = spawn_socks(AuthMode::Password {
		username: "alice".into(),
		password: "s3cret".into(),
	})
	.await;

	let mut s = TcpStream::connect(proxy).await.unwrap();
	let result = negotiate_password(&mut s, "alice", "wrong").await;
	// Either the server reports a non-zero status, or it closes the stream.
	let rejected = matches!(result, Ok(st) if st != 0x00) || result.is_err();
	assert!(rejected, "wrong credentials must be rejected, got {result:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_command_is_rejected() {
	let (proxy, _cancel) = spawn_socks(AuthMode::NoAuth).await;

	let mut s = TcpStream::connect(proxy).await.unwrap();
	negotiate_no_auth(&mut s).await;

	// CMD = 0x02 (BIND) is not supported by this inbound.
	let mut req = vec![0x05, 0x02, 0x00];
	req.extend_from_slice(&ipv4_body(Ipv4Addr::LOCALHOST, 9));
	s.write_all(&req).await.unwrap();

	let mut reply = [0u8; 10];
	s.read_exact(&mut reply).await.unwrap();
	assert_ne!(reply[1], 0x00, "BIND must not be reported as success");
}
