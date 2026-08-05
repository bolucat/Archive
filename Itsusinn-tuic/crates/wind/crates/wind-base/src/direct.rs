use std::{
	net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
	sync::Arc,
	time::Duration,
};

use async_trait::async_trait;
use socket2::{Domain, Socket, Type};
use tokio::{
	io::AsyncWriteExt,
	net::{TcpSocket, TcpStream, UdpSocket},
};
use tracing::Instrument;
use wind_core::{
	FlowContext, Outbound,
	resolve::Resolver,
	tcp::{AbstractTcpStream, TcpKeepalive},
	types::TargetAddr,
	udp::{UdpPacket, UdpStream},
	utils::StackPrefer,
};

use crate::resolve::resolve_target_with_preference;

/// Options for a direct outbound connection.
#[derive(Clone, Debug)]
pub struct DirectOutboundOpts {
	pub bind_ipv4: Option<Ipv4Addr>,
	pub bind_ipv6: Option<Ipv6Addr>,
	pub bind_device: Option<String>,
	/// Half-close idle timeout for the TCP relay.  Once one side sends FIN,
	/// the relay is reaped after this much inactivity so the still-open
	/// socket doesn't linger in CLOSE_WAIT for the lifetime of the parent
	/// connection.  Set to `Duration::ZERO` to disable.
	pub stream_timeout: Duration,
	/// TCP keepalive configuration for the outbound socket.  When `None`,
	/// `SO_KEEPALIVE` is not set (OS default, typically off).
	pub tcp_keepalive: Option<TcpKeepalive>,
	/// IP stack preference for this outbound.  When `Some`, overrides the
	/// resolver's global preference by calling `resolve_all` + picking the
	/// best address.  When `None`, the resolver's built-in preference is
	/// used (backward-compatible).
	pub ip_mode: Option<StackPrefer>,
	/// Linux `SO_MARK` value for policy routing (fwmark).  When `Some`,
	/// every socket created by this outbound is marked so that the kernel's
	/// routing policy (e.g. `ip rule fwmark`) can steer the traffic.
	/// Ignored on non-Linux platforms.
	pub routing_mark: Option<u32>,
	/// Enable TCP Fast Open (TFO).  When `true`, the outbound TCP socket
	/// may send data in the initial SYN packet, reducing one RTT for
	/// repeated connections (requires kernel support:
	/// `sysctl net.ipv4.tcp_fastopen`).
	pub tfo: bool,
	/// Enable MultiPath TCP (MPTCP).  When `true`, the outbound TCP socket
	/// uses `IPPROTO_MPTCP` so the connection can use multiple network paths
	/// simultaneously (requires Linux kernel ≥ 5.6 with MPTCP enabled).
	/// Ignored on non-Linux platforms.
	pub mptcp: bool,
}

/// Direct outbound handler – connects to the target without any proxy.
pub struct DirectOutbound {
	opts: DirectOutboundOpts,
	resolver: Arc<dyn Resolver>,
}

impl DirectOutbound {
	pub fn new(opts: DirectOutboundOpts, resolver: Arc<dyn Resolver>) -> Self {
		Self { opts, resolver }
	}
}

#[async_trait]
impl Outbound for DirectOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, mut stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let target = ctx.target;
		let span = tracing::debug_span!("direct_tcp", target = %target);
		async move {
			let target_sa = resolve_target_with_preference(&target, self.resolver.as_ref(), self.opts.ip_mode).await?;
			let mut target_stream = connect_direct_tcp(target_sa, &self.opts).await?;
			let (_, _, err) =
				wind_core::io::copy_bidirectional(&mut stream, &mut target_stream, self.opts.stream_timeout).await;
			_ = target_stream.shutdown().await;
			if let Some(e) = err {
				tracing::debug!(error = %e, "direct copy_bidirectional ended");
			}
			Ok(())
		}
		.instrument(span)
		.await
	}

	async fn handle_udp(&self, _ctx: FlowContext, udp_stream: UdpStream) -> eyre::Result<()> {
		let span = tracing::debug_span!("direct_udp");
		relay_udp_direct(self.opts.clone(), self.resolver.clone(), udp_stream)
			.instrument(span)
			.await
	}
}

/// Open a direct TCP connection to `addr`, optionally binding a local
/// address/device and setting `SO_MARK` as specified in `opts`.
///
/// When `routing_mark` is `Some`, the socket is created via `socket2`
/// (which supports `SO_MARK`); otherwise the fast-path `tokio::TcpSocket`
/// is used.
pub async fn connect_direct_tcp(addr: SocketAddr, opts: &DirectOutboundOpts) -> eyre::Result<TcpStream> {
	// routing_mark and mptcp need the full socket2 path.
	#[cfg(any(target_os = "linux", target_os = "android"))]
	if opts.routing_mark.is_some() || opts.mptcp {
		return connect_direct_tcp_with_mark(addr, opts).await;
	}

	// TFO also requires socket2 — tokio::TcpSocket doesn't expose TCP_FASTOPEN.
	if opts.tfo {
		return connect_direct_tcp_with_tfo(addr, opts).await;
	}

	let socket = match addr {
		SocketAddr::V4(_) => TcpSocket::new_v4()?,
		SocketAddr::V6(_) => TcpSocket::new_v6()?,
	};

	let bind_addr: Option<SocketAddr> = match addr {
		SocketAddr::V4(_) => opts.bind_ipv4.map(|ip| SocketAddr::V4(SocketAddrV4::new(ip, 0))),
		SocketAddr::V6(_) => opts.bind_ipv6.map(|ip| SocketAddr::V6(SocketAddrV6::new(ip, 0, 0, 0))),
	};

	if let Some(local) = bind_addr {
		socket.bind(local)?;
	}

	#[cfg(any(target_os = "linux", target_os = "android"))]
	if let Some(ref dev) = opts.bind_device {
		socket.bind_device(Some(dev.as_bytes()))?;
	}

	let stream = socket.connect(addr).await?;
	finish_tcp_stream(stream, opts)
}

/// Create a TCP connection through `socket2` so advanced socket options
/// (SO_MARK, TCP_FASTOPEN, MPTCP) can be set before `connect`.
///
/// * `mark`   — SO_MARK value (Linux only, ignored when `None`).
/// * `tfo`    — enable TCP_FASTOPEN when `true`.
/// * `mptcp`  — create with IPPROTO_MPTCP when `true` (Linux ≥ 5.6 only).
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn connect_direct_tcp_socket2(
	addr: SocketAddr,
	opts: &DirectOutboundOpts,
	mark: Option<u32>,
	mptcp: bool,
) -> eyre::Result<TcpStream> {
	use socket2::Protocol;

	let domain = match addr {
		SocketAddr::V4(_) => Domain::IPV4,
		SocketAddr::V6(_) => Domain::IPV6,
	};
	// Protocol::MPTCP is `#[cfg(target_os = "linux")]` in socket2 0.6,
	// but this function is compiled for android as well.  Use the raw
	// libc constant (IPPROTO_MPTCP = 262) which is available on both.
	let protocol = if mptcp {
		Some(Protocol::from(libc::IPPROTO_MPTCP))
	} else {
		None
	};
	let sock = Socket::new(domain, Type::STREAM, protocol)?;

	// SO_MARK
	if let Some(m) = mark.or(opts.routing_mark) {
		sock.set_mark(m)?;
	}

	// Bind
	let bind_addr: Option<SocketAddr> = match addr {
		SocketAddr::V4(_) => opts.bind_ipv4.map(|ip| SocketAddr::V4(SocketAddrV4::new(ip, 0))),
		SocketAddr::V6(_) => opts.bind_ipv6.map(|ip| SocketAddr::V6(SocketAddrV6::new(ip, 0, 0, 0))),
	};
	if let Some(local) = bind_addr {
		sock.bind(&local.into())?;
	}

	if let Some(ref dev) = opts.bind_device {
		sock.bind_device(Some(dev.as_bytes()))?;
	}

	// TCP_FASTOPEN
	if opts.tfo {
		unsafe {
			use std::os::unix::io::AsRawFd;
			let val: libc::c_int = 1;
			if libc::setsockopt(
				sock.as_raw_fd(),
				libc::IPPROTO_TCP,
				libc::TCP_FASTOPEN,
				&val as *const _ as *const libc::c_void,
				std::mem::size_of::<libc::c_int>() as libc::socklen_t,
			) != 0
			{
				tracing::debug!("TCP_FASTOPEN not available on this socket");
			}
		}
	}

	sock.set_nonblocking(true)?;
	// socket2::Socket::connect is blocking; run on the blocking pool.
	let sock = tokio::task::spawn_blocking(move || -> std::io::Result<socket2::Socket> {
		sock.connect(&addr.into())?;
		Ok(sock)
	})
	.await??;
	let std_stream: std::net::TcpStream = sock.into();
	let tokio_stream = TcpStream::from_std(std_stream)?;
	finish_tcp_stream(tokio_stream, opts)
}

/// Create a TCP connection through `socket2` so `SO_MARK` (and other
/// ancillary options) can be set before `connect`.
#[cfg(any(target_os = "linux", target_os = "android"))]
async fn connect_direct_tcp_with_mark(addr: SocketAddr, opts: &DirectOutboundOpts) -> eyre::Result<TcpStream> {
	connect_direct_tcp_socket2(addr, opts, opts.routing_mark, opts.mptcp).await
}

/// Create a TCP connection with `TCP_FASTOPEN` enabled.
///
/// TFO is set before `connect` so the kernel may include data in the SYN
/// packet on the next connection to the same peer (Linux ≥ 3.7, macOS ≥ 10.14).
async fn connect_direct_tcp_with_tfo(addr: SocketAddr, opts: &DirectOutboundOpts) -> eyre::Result<TcpStream> {
	let domain = match addr {
		SocketAddr::V4(_) => Domain::IPV4,
		SocketAddr::V6(_) => Domain::IPV6,
	};
	let sock = Socket::new(domain, Type::STREAM, None)?;

	// Bind
	let bind_addr: Option<SocketAddr> = match addr {
		SocketAddr::V4(_) => opts.bind_ipv4.map(|ip| SocketAddr::V4(SocketAddrV4::new(ip, 0))),
		SocketAddr::V6(_) => opts.bind_ipv6.map(|ip| SocketAddr::V6(SocketAddrV6::new(ip, 0, 0, 0))),
	};
	if let Some(local) = bind_addr {
		sock.bind(&local.into())?;
	}

	#[cfg(any(target_os = "linux", target_os = "android"))]
	if let Some(ref dev) = opts.bind_device {
		sock.bind_device(Some(dev.as_bytes()))?;
	}

	// TCP_FASTOPEN: enable client-side TFO so that the kernel can send data
	// in the SYN on subsequent connections to this peer.  The value 1
	// enables client TFO (see tcp(7)).
	#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
	unsafe {
		use std::os::unix::io::AsRawFd;
		let val: libc::c_int = 1;
		if libc::setsockopt(
			sock.as_raw_fd(),
			libc::IPPROTO_TCP,
			libc::TCP_FASTOPEN,
			&val as *const _ as *const libc::c_void,
			std::mem::size_of::<libc::c_int>() as libc::socklen_t,
		) != 0
		{
			tracing::debug!("TCP_FASTOPEN not available on this socket");
		}
	}

	#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "freebsd")))]
	{
		// On platforms without libc TCP_FASTOPEN (macOS handles it
		// differently via connectx), silently skip TFO.
		let _ = opts.tfo;
	}

	sock.set_nonblocking(true)?;
	// socket2::Socket::connect is blocking; run on the blocking pool.
	let sock = tokio::task::spawn_blocking(move || -> std::io::Result<socket2::Socket> {
		sock.connect(&addr.into())?;
		Ok(sock)
	})
	.await??;
	let std_stream: std::net::TcpStream = sock.into();
	let tokio_stream = TcpStream::from_std(std_stream)?;
	finish_tcp_stream(tokio_stream, opts)
}

/// Apply post-connect socket options common to both connection paths.
fn finish_tcp_stream(stream: TcpStream, opts: &DirectOutboundOpts) -> eyre::Result<TcpStream> {
	// Disable Nagle's algorithm. Proxied browser traffic is dominated by small
	// writes (TLS records, HTTP request headers); with Nagle on, each small
	// segment waits for the previous one to be ACKed, and interacts with the
	// peer's delayed-ACK timer to add up to ~40 ms of latency per round trip —
	// the classic "browsing feels laggy through the proxy" symptom. The QUIC
	// inbound has no such buffering, so this is the only hop that needs it.
	if let Err(e) = stream.set_nodelay(true) {
		tracing::debug!(error = %e, "failed to set TCP_NODELAY on direct outbound");
	}

	// Enable TCP keepalive so the OS detects dead peers (e.g. network
	// partition, host crash) even when the relay is idle.  Without this an
	// idle connection appears healthy to `copy_bidirectional` forever.
	if let Some(ref ka) = opts.tcp_keepalive
		&& let Err(e) = apply_tcp_keepalive(&stream, ka)
	{
		tracing::debug!(error = %e, "failed to set TCP keepalive on direct outbound");
	}

	Ok(stream)
}

fn apply_tcp_keepalive(s: &tokio::net::TcpStream, ka: &TcpKeepalive) -> std::io::Result<()> {
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

async fn relay_udp_direct(opts: DirectOutboundOpts, resolver: Arc<dyn Resolver>, udp_stream: UdpStream) -> eyre::Result<()> {
	let UdpStream { tx, mut rx } = udp_stream;

	let relay_socket = Arc::new(bind_relay_socket(opts.routing_mark)?);
	// When the relay socket is dual-stack IPv6, IPv4 targets must be sent to
	// their IPv4-mapped form; capture the family once rather than per packet.
	let socket_is_v6 = relay_socket.local_addr()?.is_ipv6();

	// Both directions are inlined as plain futures rather than `tokio::spawn`ed
	// — `select!` then implicitly aborts whichever half-loop is still pending
	// when its sibling finishes. Previously each branch was its own spawned
	// task and `select!` only awaited the JoinHandles, so the surviving task
	// kept polling on the shared `relay_socket`/channel until the receiver
	// closed — i.e. forever for a long-lived `tx`. Result: one leaked task
	// per UDP association, plus a `relay_socket` that stayed bound until OS
	// FD-table pressure forced a recycle.
	let socket_send = relay_socket.clone();
	let send_fut = async move {
		while let Some(pkt) = rx.recv().await {
			let target_sa = match resolve_target_with_preference(&pkt.target, resolver.as_ref(), opts.ip_mode).await {
				Ok(sa) => sa,
				Err(err) => {
					tracing::warn!(target = %pkt.target, error = %err, "UDP resolve failed");
					continue;
				}
			};
			let send_target = map_target_for_socket(target_sa, socket_is_v6);
			if let Err(err) = socket_send.send_to(&pkt.payload, send_target).await {
				tracing::warn!(target = %target_sa, error = %err, "UDP send failed");
			}
		}
	};

	let socket_recv = relay_socket.clone();
	let recv_fut = async move {
		let mut buf = vec![0u8; 65535];
		loop {
			match socket_recv.recv_from(&mut buf).await {
				Ok((len, src_addr)) => {
					use bytes::Bytes;
					// A dual-stack relay socket reports an IPv4 responder as an
					// IPv4-mapped IPv6 address (`::ffff:a.b.c.d`). Unmap it so the
					// reply the client receives is attributed to the same address
					// family as the target it originally sent to.
					let src_addr = unmap_source(src_addr);
					let payload = Bytes::copy_from_slice(&buf[..len]);
					let pkt = UdpPacket {
						source: Some(TargetAddr::from(src_addr)),
						target: TargetAddr::from(src_addr),
						payload,
					};
					if tx.send(pkt).await.is_err() {
						break;
					}
				}
				Err(err) => {
					tracing::warn!(error = %err, "UDP recv error");
					break;
				}
			}
		}
	};

	tokio::select! {
		_ = send_fut => {}
		_ = recv_fut => {}
	}

	Ok(())
}

/// Bind the local UDP socket used to relay one association's outbound packets.
///
/// A single association can target a mix of IPv4 and IPv6 hosts, so the socket
/// must be able to reach both families. We bind an IPv6 socket with
/// `IPV6_V6ONLY=false` (dual-stack): IPv6 targets are reached directly and IPv4
/// targets via their IPv4-mapped form (`::ffff:a.b.c.d`). A host without IPv6
/// support falls back to an IPv4-only socket, which still reaches IPv4 targets.
///
/// When `routing_mark` is `Some`, `SO_MARK` is set on Linux for policy routing.
///
/// This mirrors `wind-socks`'s `udp_bind_random_port`. Previously this socket
/// was hard-bound to `0.0.0.0:0` (IPv4 only), so every IPv6 target failed
/// `send_to` with `EAFNOSUPPORT` ("Address family not supported by protocol").
fn bind_relay_socket(routing_mark: Option<u32>) -> std::io::Result<UdpSocket> {
	const V6_UNSPEC: SocketAddr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));
	const V4_UNSPEC: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));

	let socket = Socket::new(Domain::IPV6, Type::DGRAM, None)
		.and_then(|s| s.set_only_v6(false).map(|_| s))
		.and_then(|s| s.bind(&V6_UNSPEC.into()).map(|_| s))
		.or_else(|_| Socket::new(Domain::IPV4, Type::DGRAM, None).and_then(|s| s.bind(&V4_UNSPEC.into()).map(|_| s)))?;

	#[cfg(any(target_os = "linux", target_os = "android"))]
	if let Some(mark) = routing_mark {
		socket.set_mark(mark)?;
	}
	#[cfg(not(any(target_os = "linux", target_os = "android")))]
	let _ = routing_mark;

	socket.set_nonblocking(true)?;
	UdpSocket::from_std(socket.into())
}

/// Rewrite an outbound target into the form sendable on the relay socket.
///
/// On a dual-stack IPv6 socket, an IPv4 destination must be expressed as
/// IPv4-mapped IPv6; passing a bare `SocketAddr::V4` to `send_to` would itself
/// fail with `EAFNOSUPPORT`. IPv6 targets, and all targets on an IPv4-only
/// socket, are sent unchanged.
fn map_target_for_socket(target: SocketAddr, socket_is_v6: bool) -> SocketAddr {
	match target {
		SocketAddr::V4(v4) if socket_is_v6 => SocketAddr::V6(SocketAddrV6::new(v4.ip().to_ipv6_mapped(), v4.port(), 0, 0)),
		_ => target,
	}
}

/// Undo IPv4-mapped IPv6 (`::ffff:a.b.c.d`) so reply packets are attributed to
/// the responder's true address family. Real IPv6 and IPv4 sources pass
/// through unchanged.
fn unmap_source(addr: SocketAddr) -> SocketAddr {
	match addr {
		SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
			Some(v4) => SocketAddr::V4(SocketAddrV4::new(v4, v6.port())),
			None => addr,
		},
		_ => addr,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn v4_target_is_mapped_only_on_dual_stack_socket() {
		let v4: SocketAddr = "192.0.2.1:53".parse().unwrap();
		// Dual-stack v6 socket: must become IPv4-mapped v6, else EAFNOSUPPORT.
		let mapped = map_target_for_socket(v4, true);
		assert_eq!(mapped, "[::ffff:192.0.2.1]:53".parse().unwrap());
		assert!(mapped.is_ipv6());
		// IPv4-only socket: sent unchanged.
		assert_eq!(map_target_for_socket(v4, false), v4);
	}

	#[test]
	fn v6_target_is_never_rewritten() {
		// The exact target family from the bug report.
		let v6: SocketAddr = "[2404:3fc0:2:101::671c:3697]:27019".parse().unwrap();
		assert_eq!(map_target_for_socket(v6, true), v6);
		assert_eq!(map_target_for_socket(v6, false), v6);
	}

	#[test]
	fn unmap_restores_v4_from_mapped_v6() {
		let mapped: SocketAddr = "[::ffff:192.0.2.1]:53".parse().unwrap();
		assert_eq!(unmap_source(mapped), "192.0.2.1:53".parse().unwrap());
	}

	#[test]
	fn unmap_leaves_real_v6_and_v4_untouched() {
		let v6: SocketAddr = "[2001:db8::1]:443".parse().unwrap();
		assert_eq!(unmap_source(v6), v6);
		let v4: SocketAddr = "10.0.0.1:80".parse().unwrap();
		assert_eq!(unmap_source(v4), v4);
	}

	#[test]
	fn map_then_unmap_roundtrips_v4() {
		let v4: SocketAddr = "203.0.113.7:9000".parse().unwrap();
		assert_eq!(unmap_source(map_target_for_socket(v4, true)), v4);
	}
}
