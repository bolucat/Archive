use std::time::Duration;

use async_trait::async_trait;
use fast_socks5::client::{Config as Socks5Config, Socks5Stream};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::Instrument;
use wind_core::{
	FlowContext, Outbound,
	tcp::{AbstractTcpStream, TcpKeepalive},
	types::TargetAddr,
	udp::UdpStream,
};

/// Options for a SOCKS5 outbound action handler.
#[derive(Clone, Debug)]
pub struct SocksOutboundOpts {
	/// SOCKS5 proxy address (e.g. "127.0.0.1:1080").
	pub addr: String,
	/// Optional authentication credentials.
	pub username: Option<String>,
	pub password: Option<String>,
	/// Whether UDP sessions may be handed to this outbound at all.
	///
	/// UDP-over-SOCKS5 (UDP ASSOCIATE, RFC 1928 §7) is **not implemented**, so
	/// a UDP session is refused with an explicit error either way — this flag
	/// only selects which refusal the caller sees (`false`/unset: the operator
	/// disabled UDP for this outbound; `true`: UDP was permitted but the
	/// transport is unavailable).  It is kept so the configuration surface
	/// stays compatible.
	pub allow_udp: Option<bool>,
	/// Half-close idle timeout for the TCP relay.  Once one side sends FIN,
	/// the relay is reaped after this much inactivity.  Set to
	/// `Duration::ZERO` to disable.
	pub stream_timeout: Duration,
	/// TCP keepalive configuration for the outbound socket.  When `None`,
	/// `SO_KEEPALIVE` is not set.
	pub tcp_keepalive: Option<TcpKeepalive>,
}

/// SOCKS5 outbound handler implementing the object-safe [`Outbound`] trait.
pub struct SocksOutbound {
	opts: SocksOutboundOpts,
}

impl SocksOutbound {
	pub fn new(opts: SocksOutboundOpts) -> Self {
		Self { opts }
	}
}

#[async_trait]
impl Outbound for SocksOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, mut stream: Box<dyn AbstractTcpStream>) -> eyre::Result<()> {
		let target = ctx.target;
		let span = tracing::debug_span!("socks5_tcp", target = %target, addr = %self.opts.addr);
		async move {
			let mut socks_stream = connect_socks5_tcp(&self.opts.addr, &target, &self.opts).await?;
			let (_, _, err) = wind_core::io::copy_bidirectional(&mut stream, &mut socks_stream, self.opts.stream_timeout).await;
			_ = socks_stream.shutdown().await;
			if let Some(e) = err {
				tracing::debug!(error = %e, "socks5 copy_bidirectional ended");
			}
			Ok(())
		}
		.instrument(span)
		.await
	}

	/// Refuse the UDP session with an explicit error.
	///
	/// This outbound cannot carry UDP: UDP-over-SOCKS5 (UDP ASSOCIATE, RFC 1928
	/// §7) is not implemented, and the datagrams are not sent directly either.
	/// Both branches used to return `Ok(())`, which made the dispatcher and the
	/// inbound report success while every datagram was dropped — a black hole
	/// that was invisible at every log level.  Returning an error instead lets
	/// the caller surface it (the SOCKS5 inbound logs a failed
	/// `handle_udpstream` at `error`) so the operator can see the misroute.
	async fn handle_udp(&self, _ctx: FlowContext, _udp_stream: UdpStream) -> eyre::Result<()> {
		if !self.opts.allow_udp.unwrap_or(false) {
			return Err(eyre::eyre!(
				"socks5 outbound '{}' refuses the UDP session: UDP is disabled for this outbound (allow_udp is false or unset)",
				self.opts.addr
			));
		}
		Err(eyre::eyre!(
			"socks5 outbound '{}' cannot carry the UDP session: UDP-over-SOCKS5 is not implemented; route UDP traffic through \
			 an outbound that supports it",
			self.opts.addr
		))
	}
}

/// Connect to `target_addr` through the SOCKS5 proxy.
async fn connect_socks5_tcp(
	socks_addr: &str,
	target_addr: &TargetAddr,
	opts: &SocksOutboundOpts,
) -> eyre::Result<Socks5Stream<TcpStream>> {
	let config = Socks5Config::default();

	let (target_host, target_port) = match target_addr {
		TargetAddr::IPv4(ip, port) => (ip.to_string(), *port),
		TargetAddr::IPv6(ip, port) => (ip.to_string(), *port),
		TargetAddr::Domain(domain, port) => (domain.clone(), *port),
	};

	let stream = match (&opts.username, &opts.password) {
		(Some(user), Some(pass)) => {
			Socks5Stream::connect_with_password(socks_addr, target_host, target_port, user.clone(), pass.clone(), config)
				.await
				.map_err(|e| eyre::eyre!("SOCKS5 connect failed: {}", e))?
		}
		_ => Socks5Stream::connect(socks_addr, target_host, target_port, config)
			.await
			.map_err(|e| eyre::eyre!("SOCKS5 connect failed: {}", e))?,
	};

	// Disable Nagle on the hop to the SOCKS proxy — the same small-write
	// latency concern as the direct path (see
	// `wind_base::direct::connect_direct_tcp`).
	if let Err(e) = stream.get_socket_ref().set_nodelay(true) {
		tracing::debug!(error = %e, "failed to set TCP_NODELAY on socks5 outbound");
	}

	// Enable TCP keepalive with the same rationale as the direct path.
	if let Some(ref ka) = opts.tcp_keepalive
		&& let Err(e) = apply_socks_keepalive(stream.get_socket_ref(), ka)
	{
		tracing::debug!(error = %e, "failed to set TCP keepalive on socks5 outbound");
	}

	Ok(stream)
}

fn apply_socks_keepalive(s: &tokio::net::TcpStream, ka: &TcpKeepalive) -> std::io::Result<()> {
	#[cfg(unix)]
	{
		use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
		let sock = unsafe { socket2::Socket::from_raw_fd(s.as_raw_fd()) };
		if let Err(e) = sock.set_keepalive(true) {
			let _ = sock.into_raw_fd();
			return Err(e);
		}
		let _ = sock.into_raw_fd();
	}
	#[cfg(any(target_os = "linux", target_os = "android"))]
	{
		use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};
		let sock = unsafe { socket2::Socket::from_raw_fd(s.as_raw_fd()) };
		let socket2_ka = socket2::TcpKeepalive::new()
			.with_time(ka.idle)
			.with_interval(ka.interval)
			.with_retries(ka.retries);
		let res = sock.set_tcp_keepalive(&socket2_ka);
		let _ = sock.into_raw_fd();
		res?
	}
	let _ = (s, ka);
	Ok(())
}

#[cfg(test)]
mod tests {
	use std::{future::Future, net::SocketAddr, sync::Arc};

	use bytes::Bytes;
	use tokio::sync::mpsc;
	use wind_core::{
		Dispatcher, FlowContext, InboundCallback, RouteAction, Router,
		hooks::Protocol,
		rule::{InboundType, NetworkType},
		types::TargetAddr,
		udp::{UdpPacket, UdpStream},
	};

	use super::*;

	/// Routes every flow to the `socks5` handler registered by the tests.
	struct ForwardSocks5;

	impl Router for ForwardSocks5 {
		#[allow(clippy::manual_async_fn)]
		fn route(&self, _ctx: &FlowContext) -> impl Future<Output = eyre::Result<RouteAction>> + Send {
			async { Ok(RouteAction::Forward("socks5".to_string())) }
		}
	}

	fn outbound(allow_udp: Option<bool>) -> SocksOutbound {
		SocksOutbound::new(SocksOutboundOpts {
			addr: "127.0.0.1:1080".to_string(),
			username: None,
			password: None,
			allow_udp,
			stream_timeout: Duration::ZERO,
			tcp_keepalive: None,
		})
	}

	/// A UDP session the handler is not expected to read from.
	fn udp_session_without_data() -> UdpStream {
		let (tx, _peer_receiver) = mpsc::channel::<UdpPacket>(1);
		let (_peer_sender, rx) = mpsc::channel::<UdpPacket>(1);
		UdpStream { tx, rx }
	}

	fn udp_ctx() -> FlowContext {
		FlowContext {
			target: TargetAddr::Domain("example.com".into(), 53),
			network: NetworkType::Udp,
			source: Some("192.0.2.10:40000".parse::<SocketAddr>().unwrap()),
			inbound_tag: Arc::from("socks-in"),
			protocol: Protocol::Socks5,
			user: None,
			inbound_port: Some(1080),
			inbound_type: Some(InboundType::Socks),
		}
	}

	/// The outbound cannot carry UDP at all, so even with `allow_udp == true`
	/// the session must fail loudly instead of resolving to `Ok(())` (which
	/// reported success while dropping every datagram).
	#[tokio::test]
	async fn udp_session_is_refused_with_an_explicit_error_when_udp_is_allowed() {
		let err = outbound(Some(true))
			.handle_udp(udp_ctx(), udp_session_without_data())
			.await
			.expect_err("UDP-over-SOCKS5 is unimplemented, so this must not report success");
		let msg = err.to_string();
		assert!(msg.contains("UDP-over-SOCKS5"), "unexpected error: {msg}");
		assert!(msg.contains("127.0.0.1:1080"), "the error must name the outbound: {msg}");
	}

	/// An operator-disabled UDP path is intentional, but it must still be
	/// reported: `Ok(())` made the refusal indistinguishable from a delivered
	/// datagram.
	#[tokio::test]
	async fn udp_session_is_refused_with_an_explicit_error_when_udp_is_disabled() {
		for allow_udp in [Some(false), None] {
			let err = outbound(allow_udp)
				.handle_udp(udp_ctx(), udp_session_without_data())
				.await
				.expect_err("a disabled UDP path must fail loudly, not drop silently");
			let msg = err.to_string();
			assert!(
				msg.contains("allow_udp"),
				"unexpected error for allow_udp={allow_udp:?}: {msg}"
			);
		}
	}

	/// End-to-end at the dispatcher boundary: the error must reach the inbound
	/// callback (which logs it at `error`) rather than being converted into a
	/// successful session.
	#[tokio::test]
	async fn dispatcher_reports_the_refused_udp_session_instead_of_success() {
		let mut dispatcher = Dispatcher::new(ForwardSocks5);
		dispatcher.add_handler("socks5", Arc::new(outbound(Some(true))));

		let (tx, _peer_receiver) = mpsc::channel::<UdpPacket>(4);
		let (first_tx, rx) = mpsc::channel::<UdpPacket>(1);
		first_tx
			.try_send(UdpPacket {
				source: None,
				target: TargetAddr::Domain("example.com".into(), 53),
				payload: Bytes::from_static(b"query"),
			})
			.expect("pre-load the first datagram so the dispatcher can route");

		let err = dispatcher
			.handle_udpstream(udp_ctx(), UdpStream { tx, rx })
			.await
			.expect_err("the dispatcher must not turn a refused UDP session into success");
		assert!(err.to_string().contains("UDP-over-SOCKS5"), "unexpected error: {err}");
	}
}
