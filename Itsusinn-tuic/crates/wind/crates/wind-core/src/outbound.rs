use crate::{tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream};

pub trait AbstractOutbound {
	/// TCP traffic which needs handled by outbound
	fn handle_tcp(
		&self,
		target_addr: TargetAddr,
		stream: impl AbstractTcpStream,
		via: Option<impl AbstractOutbound + Sized + Send>,
	) -> impl Future<Output = eyre::Result<()>> + Send;
	/// UDP traffic which needs handled by outbound
	fn handle_udp(
		&self,
		udp_stream: UdpStream,
		via: Option<impl AbstractOutbound + Sized + Send>,
	) -> impl Future<Output = eyre::Result<()>> + Send;
}
