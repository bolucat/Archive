use std::future::Future;

use async_trait::async_trait;

use crate::{Dispatcher, Router, flow::FlowContext, tcp::AbstractTcpStream, udp::UdpStream};

/// An inbound listener that hands every connection to the provided callback.
///
/// Implementations own the accept loop and must not return until the listener
/// is shut down. The callback is a concrete `&Dispatcher<R>` (never a trait
/// object), so the `R: Router` type parameter flows through from the App.
///
/// The trait stays object-safe via [`async_trait`] because an App stores a
/// heterogeneous collection of inbounds as `Box<dyn AbstractInbound<R>>`.
#[async_trait]
pub trait AbstractInbound<R: Router>: Send + Sync + 'static {
	/// Should not return!
	async fn listen(&self, cb: &Dispatcher<R>) -> eyre::Result<()>;
}

/// Callback handed to an [`AbstractInbound`] for every accepted connection.
///
/// Not object-safe — inbounds always talk to a concrete [`Dispatcher<R>`].
/// The TCP stream is passed by concrete type (no boxing), and `Clone` lets an
/// inbound hand the callback to spawned tasks. Return-position `impl Future`
/// with a `Send` bound keeps the futures spawnable without boxing.
pub trait InboundCallback: Send + Sync + Clone + 'static {
	fn handle_tcpstream(
		&self,
		ctx: FlowContext,
		stream: impl AbstractTcpStream + 'static,
	) -> impl Future<Output = eyre::Result<()>> + Send;
	fn handle_udpstream(&self, ctx: FlowContext, udp_stream: UdpStream) -> impl Future<Output = eyre::Result<()>> + Send;
}
