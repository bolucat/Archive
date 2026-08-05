//! Bevy-style `App` / `Plugin` builder for assembling a `wind` runtime.
//!
//! Register outbounds, a router, inbound factories, and [`hooks`](crate::hooks)
//! (auth / traffic / connection management), optionally grouped into
//! [`Plugin`]s, then [`App::run`] wires a [`Dispatcher`], spawns every inbound
//! plus the traffic-flush task, and drives graceful shutdown.
//!
//! The `App` is generic over the concrete [`Router`] type so the router is
//! dispatched statically (no vtable). Application crates typically wrap their
//! router in an `enum` (with a hand-written `Router` impl) so a single App type
//! can switch between routing policies without boxing.
//!
//! Inbounds are supplied as factory closures so the finalized [`InboundHooks`]
//! bundle can be threaded into each one's opts at `run` time — this keeps the
//! builder decoupled from the concrete protocol crates (no circular deps).

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use bytesize::ByteSize;
use tracing::{error, info, warn};

use crate::{
	AbstractInbound, AppContext, Dispatcher, Outbound, Router,
	hooks::{
		ConnectionHooks, FanOutConnectionHooks, InboundHooks, StatsCollector, TrafficSink, TuicAuthenticator,
		UserPassAuthenticator,
	},
};

type InboundFactory<R> = Box<dyn FnOnce(InboundHooks, Arc<AppContext>) -> Box<dyn AbstractInbound<R>> + Send>;

/// A composable unit of configuration, applied to the [`App`] via
/// [`App::add_plugin`].
///
/// The `build` method is async so plugins can perform I/O (DNS, QUIC
/// handshakes, etc.) during construction.  It is called once, immediately
/// inside [`App::add_plugin`].
pub trait Plugin<R: Router> {
	fn build(self, app: App<R>) -> impl Future<Output = eyre::Result<App<R>>> + Send;
}

/// The runtime builder. Construct with [`App::new`], register everything, then
/// [`App::run`].
pub struct App<R: Router> {
	ctx: Arc<AppContext>,
	outbounds: HashMap<String, Arc<dyn Outbound>>,
	router: Option<R>,
	tuic_auth: Option<Arc<dyn TuicAuthenticator>>,
	userpass_auth: Option<Arc<dyn UserPassAuthenticator>>,
	conn_hooks: Vec<Arc<dyn ConnectionHooks>>,
	traffic_sink: Option<Arc<dyn TrafficSink>>,
	/// Externally-owned collector shared with e.g. a REST API. When set, the
	/// inbound hooks and any flush task write into this exact instance instead
	/// of a private one created inside [`App::run`].
	stats_collector: Option<Arc<StatsCollector>>,
	flush_interval: Duration,
	inbounds: Vec<InboundFactory<R>>,
}

impl<R: Router> Default for App<R> {
	fn default() -> Self {
		Self::new()
	}
}

impl<R: Router> App<R> {
	pub fn new() -> Self {
		Self {
			ctx: Arc::new(AppContext::default()),
			outbounds: HashMap::new(),
			router: None,
			tuic_auth: None,
			userpass_auth: None,
			conn_hooks: Vec::new(),
			traffic_sink: None,
			stats_collector: None,
			flush_interval: Duration::from_secs(60),
			inbounds: Vec::new(),
		}
	}

	/// The shared [`AppContext`] (task tracker + cancellation token). Pass
	/// `app.context().clone()` to anything that needs to spawn or be cancelled
	/// alongside the App.
	pub fn context(&self) -> &Arc<AppContext> {
		&self.ctx
	}

	pub async fn add_plugin(self, plugin: impl Plugin<R>) -> eyre::Result<Self> {
		plugin.build(self).await
	}

	pub fn add_outbound(mut self, name: impl Into<String>, handler: Arc<dyn Outbound>) -> Self {
		self.outbounds.insert(name.into(), handler);
		self
	}

	pub fn set_router(mut self, router: R) -> Self {
		self.router = Some(router);
		self
	}

	pub fn set_tuic_authenticator(mut self, auth: Arc<dyn TuicAuthenticator>) -> Self {
		self.tuic_auth = Some(auth);
		self
	}

	pub fn set_userpass_authenticator(mut self, auth: Arc<dyn UserPassAuthenticator>) -> Self {
		self.userpass_auth = Some(auth);
		self
	}

	pub fn add_connection_hooks(mut self, hooks: Arc<dyn ConnectionHooks>) -> Self {
		self.conn_hooks.push(hooks);
		self
	}

	/// Register the traffic reporting sink. Setting it enables per-user stats
	/// collection (a shared [`StatsCollector`] + the periodic flush task),
	/// unless a collector was injected via [`App::set_stats_collector`].
	pub fn set_traffic_sink(mut self, sink: Arc<dyn TrafficSink>) -> Self {
		self.traffic_sink = Some(sink);
		self
	}

	/// Share an externally-owned [`StatsCollector`] (e.g. one handed to a REST
	/// API or metrics endpoint) with the runtime. The collector injected here
	/// is used as the inbound hooks' `stats` handle and, when a sink is also
	/// registered, as the data source of the periodic flush task — so the App,
	/// the [`TrafficSink`], the inbound hooks, and any external reader all see
	/// the same counters. Without an injection, stats stay enabled iff a sink
	/// was registered (a private collector is created inside [`App::run`]).
	pub fn set_stats_collector(mut self, stats: Arc<StatsCollector>) -> Self {
		self.stats_collector = Some(stats);
		self
	}

	/// Flush/sample cadence for traffic stats (default 60s).
	pub fn set_flush_interval(mut self, interval: Duration) -> Self {
		self.flush_interval = interval;
		self
	}

	pub fn add_inbound_with<I, F>(mut self, factory: F) -> Self
	where
		I: AbstractInbound<R> + Send + Sync + 'static,
		F: FnOnce(InboundHooks, Arc<AppContext>) -> I + Send + 'static,
	{
		self.inbounds.push(Box::new(move |hooks, ctx| {
			Box::new(factory(hooks, ctx)) as Box<dyn AbstractInbound<R>>
		}));
		self
	}

	/// Build the dispatcher, spawn the flush task and every inbound, then
	/// run until Ctrl-C (or the context token is cancelled), draining
	/// in-flight connection handlers on the way out.
	pub async fn run(self) -> eyre::Result<()> {
		let Self {
			ctx,
			outbounds,
			router,
			tuic_auth,
			userpass_auth,
			conn_hooks,
			traffic_sink,
			stats_collector,
			flush_interval,
			inbounds,
		} = self;
		let router = router.ok_or_else(|| eyre::eyre!("App::run: no router set"))?;

		// Stats are enabled iff a sink was registered or an external collector
		// was injected via `set_stats_collector`. An injected collector takes
		// priority so the inbound hooks and any external reader (e.g. a REST
		// API) share the exact same instance.
		let stats = match stats_collector {
			Some(collector) => Some(collector),
			None => traffic_sink.as_ref().map(|_| Arc::new(StatsCollector::new())),
		};

		// Finalize the hooks bundle from the registered pieces.
		let connection = match conn_hooks.len() {
			0 => None,
			1 => Some(conn_hooks[0].clone()),
			_ => Some(Arc::new(FanOutConnectionHooks(conn_hooks)) as Arc<dyn ConnectionHooks>),
		};
		let hooks = InboundHooks {
			tuic_auth,
			userpass_auth,
			connection,
			stats: stats.clone(),
			sample_interval: flush_interval,
		};

		let mut dispatcher = Dispatcher::new(router);
		for (name, handler) in &outbounds {
			dispatcher.add_handler(name.clone(), handler.clone());
		}

		// Periodic traffic flush (drains the collector → sink, restore on error,
		// final flush on shutdown).
		if let (Some(stats), Some(sink)) = (stats.clone(), traffic_sink.clone()) {
			let token = ctx.token.clone();
			let interval = flush_interval;
			ctx.tasks.spawn(async move {
				let mut tick = tokio::time::interval(interval);
				tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
				loop {
					tokio::select! {
						_ = tick.tick() => flush_once(sink.as_ref(), &stats).await,
						_ = token.cancelled() => {
							flush_once(sink.as_ref(), &stats).await;
							break;
						}
					}
				}
			});
		}

		// Spawn each inbound, materializing it with the finalized hooks bundle.
		for factory in inbounds {
			let inbound = factory(hooks.clone(), ctx.clone());
			let dispatcher = dispatcher.clone();
			ctx.tasks.spawn(async move {
				if let Err(e) = inbound.listen(&dispatcher).await {
					error!("inbound listen error: {e:?}");
				}
			});
		}

		// Run until a shutdown signal (Ctrl-C / SIGTERM) or an
		// externally-triggered cancellation.
		tokio::select! {
			_ = crate::shutdown_signal() => {
				info!("shutdown signal received, stopping");
			}
			_ = ctx.token.cancelled() => {
				info!("shutdown signalled, stopping");
			}
		}

		ctx.token.cancel();
		ctx.tasks.close();
		if tokio::time::timeout(Duration::from_secs(10), ctx.tasks.wait()).await.is_err() {
			warn!("timed out waiting for tasks to drain; forcing runtime drop");
		}
		Ok(())
	}
}

/// Drain the collector once and submit; restore the batch if the sink fails.
async fn flush_once(sink: &dyn TrafficSink, stats: &StatsCollector) {
	let batch = stats.reset_all();
	if batch.is_empty() {
		return;
	}

	let user_count = batch.len();
	let total_upload: u64 = batch.iter().map(|t| t.upload).sum();
	let total_download: u64 = batch.iter().map(|t| t.download).sum();
	let total_requests: u64 = batch.iter().map(|t| t.request_count).sum();

	if let Err(e) = sink.submit(batch.clone()).await {
		warn!(
			"traffic sink submit failed for {} user(s) ({}↑, {}↓, {} reqs): {e:?}",
			user_count,
			ByteSize::b(total_upload).display().si(),
			ByteSize::b(total_download).display().si(),
			total_requests
		);
		stats.restore(&batch);
	} else {
		info!(
			"traffic reported: {} user(s), {}↑, {}↓, {} reqs",
			user_count,
			ByteSize::b(total_upload).display().si(),
			ByteSize::b(total_download).display().si(),
			total_requests
		);
	}
}

#[cfg(test)]
mod tests {
	use std::{
		sync::{Arc, Mutex},
		time::Duration,
	};

	use async_trait::async_trait;

	use super::*;
	use crate::{FlowContext, RouteAction};

	/// Minimal router for unit tests — rejects everything.
	struct StubRouter;

	impl Router for StubRouter {
		async fn route(&self, _ctx: &FlowContext) -> eyre::Result<RouteAction> {
			Ok(RouteAction::Reject("unit test".into()))
		}
	}

	/// Inbound that exits immediately (no-op accept loop).
	struct CaptureInbound;

	#[async_trait]
	impl AbstractInbound<StubRouter> for CaptureInbound {
		async fn listen(&self, _cb: &Dispatcher<StubRouter>) -> eyre::Result<()> {
			Ok(())
		}
	}

	/// Run an `App` in a spawned task, wait until its inbound factory ran (so
	/// the finalized hooks are captured), then cancel the context token to
	/// unwind `run`. Returns the captured hooks.
	async fn run_and_capture_hooks(app: App<StubRouter>) -> InboundHooks {
		let captured: Arc<Mutex<Option<InboundHooks>>> = Arc::new(Mutex::new(None));
		let captured_for_factory = captured.clone();
		let app = app.add_inbound_with(move |hooks, _ctx| {
			*captured_for_factory.lock().unwrap() = Some(hooks);
			CaptureInbound
		});

		let ctx = app.context().clone();
		let run_handle = tokio::spawn(async move { app.run().await });

		// `run` materializes the inbounds synchronously; poll until the factory
		// has captured the hooks, then cancel to unwind `run`.
		let deadline = std::time::Instant::now() + Duration::from_secs(5);
		while captured.lock().unwrap().is_none() {
			assert!(std::time::Instant::now() < deadline, "inbound factory never ran");
			tokio::task::yield_now().await;
		}
		ctx.token.cancel();
		run_handle.await.expect("run task panicked").expect("run failed");

		captured.lock().unwrap().take().expect("inbound factory must have run")
	}

	#[tokio::test]
	async fn injected_stats_collector_reaches_inbound_hooks() {
		let injected = Arc::new(StatsCollector::new());
		let hooks = run_and_capture_hooks(App::new().set_router(StubRouter).set_stats_collector(injected.clone())).await;

		let stats = hooks.stats.expect("inbound hooks must carry a stats collector");
		assert!(
			Arc::ptr_eq(&injected, &stats),
			"injected collector must be the exact instance seen by the inbound hooks"
		);
	}

	#[tokio::test]
	async fn no_sink_and_no_injection_keeps_stats_disabled() {
		let hooks = run_and_capture_hooks(App::new().set_router(StubRouter)).await;

		assert!(
			hooks.stats.is_none(),
			"stats stay disabled without a sink or an injected collector"
		);
	}
}
