use std::net::SocketAddr;

use eyre::eyre;
use fast_socks5::{
	Socks5Command,
	client::{Config, Socks5Stream},
};
use wind_core::{AbstractOutbound, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream};

pub struct SocksOutboundOpt {
	pub server_addr: SocketAddr,
	pub auth: Option<(String, String)>,
}

pub struct SocksOutbound {
	opts: SocksOutboundOpt,
}

impl SocksOutbound {
	pub fn new(opts: SocksOutboundOpt) -> Self {
		Self { opts }
	}
}

impl AbstractOutbound for SocksOutbound {
	async fn handle_tcp(
		&self,
		target_addr: TargetAddr,
		mut stream: impl AbstractTcpStream,
		via: Option<impl AbstractOutbound + Sized + Send>,
	) -> eyre::Result<()> {
		let socks_config = Config::default();

		let socks_addr = crate::convert_to_socks_addr(&target_addr);

		let auth = self
			.opts
			.auth
			.as_ref()
			.map(|(user, pass)| fast_socks5::AuthenticationMethod::Password {
				username: user.clone(),
				password: pass.clone(),
			});

		if let Some(via) = via {
			let (local_stream, remote_stream) = tokio::io::duplex(8192);

			let server_addr_target = match self.opts.server_addr.ip() {
				std::net::IpAddr::V4(v4) => TargetAddr::IPv4(v4, self.opts.server_addr.port()),
				std::net::IpAddr::V6(v6) => TargetAddr::IPv6(v6, self.opts.server_addr.port()),
			};

			let via_future = via.handle_tcp(server_addr_target, remote_stream, None::<SocksOutbound>);

			let socks_future = async move {
				let mut p = Socks5Stream::use_stream(local_stream, auth, socks_config)
					.await
					.map_err(|e| eyre!(e))?;
				p.request(Socks5Command::TCPConnect, socks_addr).await.map_err(|e| eyre!(e))?;
				let mut p: Box<dyn AbstractTcpStream> = Box::new(p);
				if let (_, _, Some(e)) = wind_core::io::copy_io(&mut stream, &mut p).await {
					return Err(eyre!(e));
				}
				Ok::<(), eyre::Report>(())
			};

			let (res_via, res_socks) = tokio::join!(via_future, socks_future);
			res_via?;
			res_socks?;
		} else {
			let tcp_stream = tokio::net::TcpStream::connect(self.opts.server_addr).await?;
			// Disable Nagle on the hop to the SOCKS proxy (small-write latency).
			if let Err(e) = tcp_stream.set_nodelay(true) {
				tracing::debug!(error = %e, "failed to set TCP_NODELAY on socks5 outbound");
			}
			let mut p = Socks5Stream::use_stream(tcp_stream, auth, socks_config)
				.await
				.map_err(|e| eyre!(e))?;
			p.request(Socks5Command::TCPConnect, socks_addr).await.map_err(|e| eyre!(e))?;
			let mut proxy_stream: Box<dyn AbstractTcpStream> = Box::new(p);
			if let (_, _, Some(e)) = wind_core::io::copy_io(&mut stream, &mut proxy_stream).await {
				return Err(eyre!(e));
			}
		}

		Ok(())
	}

	async fn handle_udp(&self, _udp_stream: UdpStream, _via: Option<impl AbstractOutbound + Sized + Send>) -> eyre::Result<()> {
		Err(eyre!("SOCKS5 UDP outbound is not yet supported"))
	}
}
