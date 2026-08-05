//! Lazy outbound wrapper — defers initialisation until first connection.

use std::{future::Future, pin::Pin, sync::Arc};

use async_trait::async_trait;
use eyre;
use tokio::sync::{Mutex, Notify};
use wind_core::{FlowContext, Outbound, tcp::AbstractTcpStream, udp::UdpStream};

/// Boxed future that produces the initialised [`Outbound`] handler.
type OutboundFactory = Pin<Box<dyn Future<Output = eyre::Result<Arc<dyn Outbound>>> + Send>>;

/// Wraps a future that produces an [`Outbound`] and only executes it
/// once, on the first call to [`handle_tcp`] or [`handle_udp`].  Subsequent
/// calls delegate to the already-initialised handler.
///
/// Concurrent first-use callers are serialised: one runs the factory while
/// others wait on a notification.
pub struct LazyOutbound {
	/// Quad-state: the async factory, a sentinel meaning "another task
	/// is initialising", or the ready handler / a permanent error.
	state: Mutex<LazyState>,
	/// Signalled once initialisation finishes (success or failure) so that
	/// waiters can re-check `state`.
	ready: Notify,
}

enum LazyState {
	/// Factory has not yet been called.
	Uninit(OutboundFactory),
	/// One caller is currently running the factory; others must wait.
	Initializing,
	/// The handler is ready.
	Initialized(Arc<dyn Outbound>),
	/// Permanent failure — all future calls will return this error.
	Failed(String),
}

impl LazyOutbound {
	/// Wrap `factory` — an async closure that builds the real outbound —
	/// so that it runs at most once, on first use.
	pub fn new(factory: OutboundFactory) -> Self {
		Self {
			state: Mutex::new(LazyState::Uninit(factory)),
			ready: Notify::new(),
		}
	}

	/// Ensure the inner handler is initialised and return a clone of the
	/// `Arc`.  This is the core synchronisation point — every public method
	/// calls it first.
	async fn get_or_init(&self) -> eyre::Result<Arc<dyn Outbound>> {
		loop {
			{
				let guard = self.state.lock().await;
				match &*guard {
					LazyState::Initialized(h) => return Ok(h.clone()),
					LazyState::Failed(msg) => return Err(eyre::eyre!("LazyOutbound: {msg}")),
					LazyState::Uninit(_) => {}
					LazyState::Initializing => {}
				}
			}

			// Re-check: under the lock we may have seen Uninit or
			// Initializing.  Handle each case *outside* the lock so we
			// don't hold it across the (possibly slow) factory future.
			let future = {
				let mut guard = self.state.lock().await;
				match &*guard {
					LazyState::Initialized(h) => return Ok(h.clone()),
					LazyState::Failed(msg) => return Err(eyre::eyre!("LazyOutbound: {msg}")),
					LazyState::Initializing => {
						// Another task is running the factory — wait.
						drop(guard);
						self.ready.notified().await;
						continue;
					}
					LazyState::Uninit(_) => {
						// Take ownership of the future.
						let LazyState::Uninit(fut) = std::mem::replace(&mut *guard, LazyState::Initializing) else {
							unreachable!()
						};
						fut
					}
				}
			};

			// Run the factory *without* holding the lock.
			let result = future.await;
			let mut guard = self.state.lock().await;
			match result {
				Ok(handler) => {
					*guard = LazyState::Initialized(handler.clone());
					self.ready.notify_waiters();
					return Ok(handler);
				}
				Err(e) => {
					let msg = format!("{e:#}");
					*guard = LazyState::Failed(msg.clone());
					self.ready.notify_waiters();
					return Err(eyre::eyre!("LazyOutbound init failed: {msg}"));
				}
			}
		}
	}
}

#[async_trait]
impl Outbound for LazyOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let handler = self.get_or_init().await?;
		handler.handle_tcp(ctx, stream).await
	}

	async fn handle_udp(&self, ctx: FlowContext, stream: UdpStream) -> eyre::Result<()> {
		let handler = self.get_or_init().await?;
		handler.handle_udp(ctx, stream).await
	}
}
