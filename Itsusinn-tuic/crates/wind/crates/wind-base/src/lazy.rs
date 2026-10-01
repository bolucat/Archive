//! Lazy outbound wrapper — defers initialisation until first connection.

use std::{future::Future, pin::Pin, sync::Arc};

use async_trait::async_trait;
use eyre;
use tokio::sync::Mutex;
use wind_core::{FlowContext, Outbound, tcp::AbstractTcpStream, udp::UdpStream};

/// Boxed future that produces the initialised [`Outbound`] handler.
type OutboundFactory = Pin<Box<dyn Future<Output = eyre::Result<Arc<dyn Outbound>>> + Send>>;

/// Wraps a future that produces an [`Outbound`] and only executes it
/// once, on the first call to [`handle_tcp`] or [`handle_udp`].  Subsequent
/// calls delegate to the already-initialised handler.
///
/// Concurrent first-use callers are serialised by the state lock: the caller
/// that finds the factory still pending polls it while holding that lock, and
/// the others queue on the same lock.  The factory future is owned by this
/// struct, so a caller that is cancelled (dropped) mid-initialisation only
/// releases the lock — the state stays `Uninit` with the partially advanced
/// factory, and the next caller resumes it instead of finding the outbound
/// permanently wedged.
pub struct LazyOutbound {
	/// Tri-state: the not-yet-finished factory, the ready handler, or a
	/// permanent error.
	state: Mutex<LazyState>,
}

enum LazyState {
	/// Factory has not completed.  It may already have been polled (and left
	/// pending) by a caller that was later cancelled; awaiting it again
	/// resumes it from where it stopped.
	Uninit(OutboundFactory),
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
		}
	}

	/// Ensure the inner handler is initialised and return a clone of the
	/// `Arc`.  This is the core synchronisation point — every public method
	/// calls it first.
	async fn get_or_init(&self) -> eyre::Result<Arc<dyn Outbound>> {
		// The lock is deliberately held across the factory await: it is what
		// serialises first-use callers, and because the factory stays owned by
		// `self`, a cancellation here cannot strand the state.  The quiche
		// connection handles in `wind-quic` use the same shape.
		let mut guard = self.state.lock().await;
		let outcome = match &mut *guard {
			LazyState::Initialized(handler) => return Ok(handler.clone()),
			LazyState::Failed(msg) => return Err(eyre::eyre!("LazyOutbound: {msg}")),
			// Await in place: if this caller is dropped, `factory` is *not*
			// dropped — it stays in `self.state` for the next caller.
			LazyState::Uninit(factory) => factory.as_mut().await,
		};
		match outcome {
			Ok(handler) => {
				*guard = LazyState::Initialized(handler.clone());
				Ok(handler)
			}
			Err(e) => {
				let msg = format!("{e:#}");
				*guard = LazyState::Failed(msg.clone());
				Err(eyre::eyre!("LazyOutbound init failed: {msg}"))
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use std::{
		sync::{
			Arc,
			atomic::{AtomicUsize, Ordering},
		},
		time::Duration,
	};

	use tokio::{sync::oneshot, time::timeout};
	use wind_core::{hooks::Protocol, rule::NetworkType, types::TargetAddr};

	use super::*;

	struct Dummy;

	#[async_trait]
	impl Outbound for Dummy {
		async fn handle_tcp(&self, _ctx: FlowContext, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
			Ok(())
		}

		async fn handle_udp(&self, _ctx: FlowContext, _stream: UdpStream) -> eyre::Result<()> {
			Ok(())
		}
	}

	/// Factory that counts its invocations and stays pending until `gate`
	/// fires, so a caller can be cancelled while it is genuinely in flight.
	fn gated_factory(calls: &Arc<AtomicUsize>, gate: oneshot::Receiver<()>) -> OutboundFactory {
		let calls = calls.clone();
		Box::pin(async move {
			calls.fetch_add(1, Ordering::SeqCst);
			let _ = gate.await;
			Ok(Arc::new(Dummy) as Arc<dyn Outbound>)
		})
	}

	fn test_flow_context() -> FlowContext {
		FlowContext {
			target: TargetAddr::Domain("example.com".into(), 443),
			network: NetworkType::Tcp,
			source: Some("192.168.1.5:12345".parse().unwrap()),
			inbound_tag: "lazy-test".into(),
			protocol: Protocol::Socks5,
			user: None,
			inbound_port: Some(1080),
			inbound_type: None,
		}
	}

	/// F2 regression: the first caller is dropped while the factory is in
	/// flight.  The timeout's first poll runs the factory synchronously up to
	/// the gate, so the wrapper is left mid-initialisation deterministically.
	#[tokio::test]
	async fn cancelled_first_caller_does_not_wedge_the_outbound() {
		let calls = Arc::new(AtomicUsize::new(0));
		let (gate_tx, gate_rx) = oneshot::channel();
		let lazy = LazyOutbound::new(gated_factory(&calls, gate_rx));

		let first = timeout(Duration::from_millis(50), lazy.get_or_init()).await;
		assert!(first.is_err(), "the first caller must be cancelled mid-initialisation");
		assert_eq!(
			calls.load(Ordering::SeqCst),
			1,
			"the factory must have been entered before the drop"
		);

		// Recovery: the factory future is still owned by the state, so
		// releasing the gate lets a later caller finish initialisation.
		let _ = gate_tx.send(());
		let second = timeout(Duration::from_secs(1), lazy.get_or_init())
			.await
			.expect("a later caller must not hang after a cancelled first init")
			.unwrap();
		assert_eq!(calls.load(Ordering::SeqCst), 1, "the factory must not run twice");

		let third = lazy.get_or_init().await.unwrap();
		assert!(Arc::ptr_eq(&second, &third), "later callers must get the cached handler");
	}

	/// Later callers queued on the state lock must all be served once the
	/// resumed factory completes.
	#[tokio::test]
	async fn callers_queued_behind_a_cancelled_driver_all_succeed() {
		let calls = Arc::new(AtomicUsize::new(0));
		let (gate_tx, gate_rx) = oneshot::channel();
		let lazy = LazyOutbound::new(gated_factory(&calls, gate_rx));

		let first = timeout(Duration::from_millis(50), lazy.get_or_init()).await;
		assert!(first.is_err(), "the driver must be cancelled mid-initialisation");

		// Release the factory while the three callers below are already queued
		// on the state lock, so each must observe the resumed factory finish.
		let releaser = tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(100)).await;
			let _ = gate_tx.send(());
		});
		let (a, b, c) = tokio::join!(
			timeout(Duration::from_secs(2), lazy.get_or_init()),
			timeout(Duration::from_secs(2), lazy.get_or_init()),
			timeout(Duration::from_secs(2), lazy.get_or_init()),
		);
		releaser.await.unwrap();

		let a = a.expect("caller a must not hang after a cancelled driver").unwrap();
		let b = b.expect("caller b must not hang after a cancelled driver").unwrap();
		let c = c.expect("caller c must not hang after a cancelled driver").unwrap();
		assert!(
			Arc::ptr_eq(&a, &b) && Arc::ptr_eq(&b, &c),
			"all callers must share one handler"
		);
		assert_eq!(calls.load(Ordering::SeqCst), 1, "the factory must run exactly once");
	}

	/// A genuinely failed factory stays a permanent error (unlike the
	/// cancellation case, which must be recoverable).
	#[tokio::test]
	async fn a_failed_factory_stays_permanent() {
		let calls = Arc::new(AtomicUsize::new(0));
		let factory = {
			let calls = calls.clone();
			Box::pin(async move {
				calls.fetch_add(1, Ordering::SeqCst);
				Err::<Arc<dyn Outbound>, _>(eyre::eyre!("synthetic factory failure"))
			})
		};
		let lazy = LazyOutbound::new(factory);

		async fn failure_message(lazy: &LazyOutbound) -> String {
			match lazy.get_or_init().await {
				Ok(_) => panic!("the factory must fail"),
				Err(error) => format!("{error:#}"),
			}
		}

		assert!(failure_message(&lazy).await.contains("synthetic factory failure"));
		assert!(failure_message(&lazy).await.contains("synthetic factory failure"));
		assert_eq!(calls.load(Ordering::SeqCst), 1, "a failed factory must not be retried");
	}

	/// The reported symptom, end to end through the [`Outbound`] trait.
	#[tokio::test]
	async fn handle_tcp_recovers_after_a_cancelled_first_init() {
		let calls = Arc::new(AtomicUsize::new(0));
		let (gate_tx, gate_rx) = oneshot::channel();
		let lazy = LazyOutbound::new(gated_factory(&calls, gate_rx));
		let ctx = test_flow_context();

		let (stream, _peer) = tokio::io::duplex(64);
		let first = timeout(Duration::from_millis(50), lazy.handle_tcp(ctx.clone(), Box::new(stream))).await;
		assert!(first.is_err(), "the first connection must be cancelled mid-init");

		let _ = gate_tx.send(());
		let (stream, _peer) = tokio::io::duplex(64);
		let second = timeout(Duration::from_secs(1), lazy.handle_tcp(ctx, Box::new(stream))).await;
		assert!(
			second.expect("a later connection must not hang").is_ok(),
			"the recovered outbound must serve the connection"
		);
		assert_eq!(calls.load(Ordering::SeqCst), 1);
	}
}
