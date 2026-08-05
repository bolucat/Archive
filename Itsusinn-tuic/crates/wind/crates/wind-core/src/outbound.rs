use async_trait::async_trait;

use crate::{flow::FlowContext, tcp::AbstractTcpStream, udp::UdpStream};

/// Object-safe outbound handler.
///
/// Each concrete outbound strategy (direct connect, SOCKS5 proxy, TUIC, …)
/// implements this trait.  The stream types are erased via trait objects so
/// handlers can be stored in a `HashMap` and shared behind `Arc<dyn Outbound>`.
#[async_trait]
pub trait Outbound: Send + Sync + 'static {
	/// Handle an inbound TCP stream.
	///
	/// The stream is boxed and `'static` so it can be stored or sent across
	/// tasks.  All concrete `AbstractTcpStream` implementations (owned
	/// `TcpStream`, `Socks5Stream<TcpStream>`, …) satisfy this bound.
	async fn handle_tcp(&self, ctx: FlowContext, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()>;

	/// Handle an inbound UDP session.
	async fn handle_udp(&self, ctx: FlowContext, stream: UdpStream) -> eyre::Result<()>;
}
