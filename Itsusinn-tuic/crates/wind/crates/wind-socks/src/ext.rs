use std::{
	io,
	net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use fast_socks5::{
	server::{ErrorContext as _, Socks5ServerProtocol, SocksServerError, states, wait_on_tcp},
	util::target_addr::TargetAddr,
};
use futures_util::TryFutureExt;
use socket2::{Domain, Socket, Type};
use tokio::{
	io::{AsyncRead, AsyncWrite},
	try_join,
};
use tracing::{debug, error, warn};

use crate::Error;

macro_rules! try_notify {
    ($proto:expr, $e:expr) => {
        match $e {
            Ok(res) => res,
            Err(err) => {
                if let Err(rep_err) = $proto.reply_error(&err.to_reply_error()).await {
                    error!(
                        name: "socks",
                        "extra error while reporting an error to the client: {}",
                        rep_err
                    );
                }
                return Err(err.into());
            }
        }
    };
}

fn udp_bind_random_port(addr: Option<IpAddr>) -> io::Result<Socket> {
	if let Some(addr) = addr {
		let sock_addr = SocketAddr::new(addr, 0);
		let socket = Socket::new(Domain::for_address(sock_addr), Type::DGRAM, None)?;
		socket.bind(&sock_addr.into())?;
		return socket.set_nonblocking(true).map(|_| socket);
	}

	const V4_UNSPEC: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
	const V6_UNSPEC: SocketAddr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0);
	Socket::new(Domain::IPV6, Type::DGRAM, None)
		.and_then(|socket| socket.set_only_v6(false).map(|_| socket))
		.and_then(|socket| socket.bind(&V6_UNSPEC.into()).map(|_| socket))
		.or_else(|_| {
			Socket::new(Domain::IPV4, Type::DGRAM, None).and_then(|socket| socket.bind(&V4_UNSPEC.into()).map(|_| socket))
		})
		.and_then(|socket| socket.set_nonblocking(true).map(|_| socket))
}

pub async fn run_udp_proxy<T, F, R>(
	proto: Socks5ServerProtocol<T, states::CommandRead>,
	_addr: &TargetAddr,
	peer_bind_ip: Option<IpAddr>,
	reply_ip: IpAddr,
	transfer: F,
) -> Result<T, Error>
where
	T: AsyncRead + AsyncWrite + Unpin,
	F: FnOnce(Socket) -> R,
	R: Future<Output = Result<(), Error>>,
{
	let peer_sock = try_notify!(
		proto,
		udp_bind_random_port(peer_bind_ip).err_when("binding client udp socket")
	);

	let peer_addr = try_notify!(proto, peer_sock.local_addr().err_when("getting peer's local addr"));

	let reply_port = peer_addr.as_socket().ok_or(SocksServerError::Bug("addr not IP"))?.port();

	let mut inner = proto.reply_success(SocketAddr::new(reply_ip, reply_port)).await?;

	let udp_fut = transfer(peer_sock);
	let tcp_fut = wait_on_tcp(&mut inner).map_err(Error::from);

	match try_join!(udp_fut, tcp_fut) {
		Ok(_) => {
			// `try_join!` only reaches this arm if BOTH futures complete with
			// `Ok`. `wait_on_tcp` only returns on EOF, and `transfer` returns
			// on the upstream channel closing — both are legitimate clean
			// shutdowns, not "unreachable" anomalies.
			debug!("SOCKS5 UDP proxy completed cleanly")
		}
		Err(Error::Socks {
			source: SocksServerError::EOF,
			backtrace: _,
		}) => {
			debug!("EOF on controlling TCP stream, closed UDP proxy")
		}
		Err(err) => warn!("while UDP proxying: {err}"),
	}
	Ok(inner)
}
