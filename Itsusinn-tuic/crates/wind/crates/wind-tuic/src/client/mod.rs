//! Backend-agnostic TUIC client plumbing.
//!
//! [`ClientTaskExt::handle_incoming`] spawns the accept loops (datagram / bi /
//! uni) for an established [`QuicConnection`] and returns receive channels the
//! outbound poll loop drains. Written once against the trait; the concrete
//! `TuicOutbound` (currently quinn-only) drives it.

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use crossfire::{AsyncRx, SendTimeoutError, spsc};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument as _, info, warn};
use wind_core::AppContext;
use wind_quic::QuicConnection;

use crate::Error;

/// Size of the single-producer single-consumer buffer for QUIC streams. Larger
/// buffers reduce backpressure stalls on bursty workloads at the cost of a
/// small amount of extra memory per connection.
const SPSC_BUFFER_SIZE: usize = 64;

/// Receivers returned by [`ClientTaskExt::handle_incoming`]: datagrams,
/// incoming bidirectional streams, and incoming unidirectional streams.
type IncomingRx<C> = (
	AsyncRx<spsc::Array<Bytes>>,
	AsyncRx<spsc::Array<(<C as QuicConnection>::SendStream, <C as QuicConnection>::RecvStream)>>,
	AsyncRx<spsc::Array<<C as QuicConnection>::RecvStream>>,
);

/// Spawn a task that drives an `accept`-style call and forwards each accepted
/// item to a channel.
async fn spawn_handler<C, T, F, Fut>(
	ctx: Arc<AppContext>,
	connection: C,
	cancel_token: CancellationToken,
	accept_fn: F,
	name: &'static str,
) -> AsyncRx<spsc::Array<T>>
where
	C: QuicConnection,
	T: Send + Unpin + 'static,
	F: Fn(C) -> Fut + Send + 'static,
	Fut: std::future::Future<Output = Result<T, wind_quic::QuicError>> + Send,
{
	let (tx, rx) = spsc::bounded_async(SPSC_BUFFER_SIZE);

	ctx.tasks.spawn(
		async move {
			loop {
				tokio::select! {
					res = accept_fn(connection.clone()) => {
						let item = match res {
							Err(e) => {
								warn!("Connection error in {} handler: {e:?}", name);
								break;
							}
							Ok(item) => item,
						};

						info!("Accepted new {}", name);
						// Distinguish a closed channel (consumer permanently gone —
						// exit) from a slow consumer (transient back-pressure — drop
						// the item and keep accepting).
						match tx.send_timeout(item, Duration::from_secs(1)).await {
							Ok(()) => {}
							Err(SendTimeoutError::Disconnected(_)) => {
								warn!("{name} consumer dropped; ending accept loop");
								break;
							}
							Err(SendTimeoutError::Timeout(_)) => {
								warn!("{name} consumer slow (>1s); dropping item and continuing");
								continue;
							}
						}
					}
					_ = cancel_token.cancelled() => {
						info!("Cancellation requested for {} task", name);
						break;
					}
				}
			}
		}
		.in_current_span(),
	);

	rx
}

pub trait ClientTaskExt: QuicConnection {
	fn handle_incoming(
		&self,
		ctx: Arc<AppContext>,
		cancel_token: CancellationToken,
	) -> impl std::future::Future<Output = Result<IncomingRx<Self>, Error>> + Send;
}

impl<C: QuicConnection> ClientTaskExt for C {
	async fn handle_incoming(&self, ctx: Arc<AppContext>, cancel_token: CancellationToken) -> Result<IncomingRx<Self>, Error> {
		let datagram_rx = spawn_handler(
			ctx.clone(),
			self.clone(),
			cancel_token.clone(),
			|conn| async move { conn.read_datagram().await },
			"datagram",
		)
		.await;

		let bi_rx = spawn_handler(
			ctx.clone(),
			self.clone(),
			cancel_token.clone(),
			|conn| async move { conn.accept_bi().await },
			"bi-directional stream",
		)
		.await;

		let uni_rx = spawn_handler(
			ctx.clone(),
			self.clone(),
			cancel_token,
			|conn| async move { conn.accept_uni().await },
			"uni-directional stream",
		)
		.await;

		Ok((datagram_rx, bi_rx, uni_rx))
	}
}
