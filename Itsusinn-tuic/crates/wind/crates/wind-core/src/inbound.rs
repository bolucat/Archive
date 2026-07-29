use std::future::Future;

use crate::{tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream};

pub trait AbstractInbound {
	/// Should not return!
	fn listen(&self, cb: &impl InboundCallback) -> impl Future<Output = eyre::Result<()>> + Send;
}

pub trait InboundCallback: Send + Sync + Clone + 'static {
	fn handle_tcpstream(
		&self,
		target_addr: TargetAddr,
		stream: impl AbstractTcpStream + 'static,
	) -> impl Future<Output = eyre::Result<()>> + Send;
	fn handle_udpstream(&self, udp_stream: UdpStream) -> impl Future<Output = eyre::Result<()>> + Send;
}
