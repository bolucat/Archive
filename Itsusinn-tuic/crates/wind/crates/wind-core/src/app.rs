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

/// How long [`App::run`] waits for in-flight tasks after cancelling the context
/// token, unless overridden with [`App::set_shutdown_timeout`].
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

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
	/// Explicit cadence for the inbounds' traffic sampler. `None` keeps the
	/// historic behavior of following [`App::set_flush_interval`].
	sample_interval: Option<Duration>,
	shutdown_timeout: Duration,
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
			sample_interval: None,
			shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
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

	/// Cadence for the periodic traffic-stats flush to the sink (default 60s).
	///
	/// This also sets the inbounds' traffic-sampler cadence
	/// ([`InboundHooks::sample_interval`]) unless it is given its own value
	/// with [`App::set_sample_interval`] — the historic coupling, kept so
	/// existing callers keep their behavior. Set
	/// [`App::set_sample_interval`] when the sampling window and the
	/// reporting window should differ (they pull in opposite directions: a
	/// short flush window bounds reporting latency, while a short sampling
	/// window only adds `byte_stats()` polling overhead).
	pub fn set_flush_interval(mut self, interval: Duration) -> Self {
		self.flush_interval = interval;
		self
	}

	/// Cadence at which each inbound samples traffic into the shared
	/// [`StatsCollector`] (default: whatever [`App::set_flush_interval`] is,
	/// itself 60s by default).
	///
	/// Sampling is independent of the flush task: samples accumulate in the
	/// collector until the next flush reports them, so this never changes what
	/// is reported, only how finely a connection's traffic is sampled.
	pub fn set_sample_interval(mut self, interval: Duration) -> Self {
		self.sample_interval = Some(interval);
		self
	}

	/// How long [`App::run`] may spend draining in-flight connection handlers
	/// after the context token is cancelled (default 10s).
	///
	/// Once the deadline passes `run` stops waiting, logs a warning, and lets
	/// the runtime drop whatever is still registered — so a lower value
	/// shortens shutdown at the cost of cutting long-lived handlers short.
	/// Tasks that finish earlier are not delayed: the deadline is an upper
	/// bound, not a fixed wait.
	pub fn set_shutdown_timeout(mut self, timeout: Duration) -> Self {
		self.shutdown_timeout = timeout;
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
			sample_interval,
			shutdown_timeout,
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
			// An explicit sampler cadence wins; otherwise keep the historic
			// coupling to the flush interval.
			sample_interval: sample_interval.unwrap_or(flush_interval),
		};

		let mut dispatcher = Dispatcher::new(router).context(ctx.clone());
		for (name, handler) in &outbounds {
			dispatcher.add_handler(name.clone(), handler.clone());
		}

		// Periodic traffic flush (drains the collector → sink, restore on
		// error, retried final flush on shutdown).
		if let (Some(stats), Some(sink)) = (stats.clone(), traffic_sink.clone()) {
			let token = ctx.token.clone();
			let interval = flush_interval;
			ctx.tasks.spawn(async move {
				let mut tick = tokio::time::interval(interval);
				tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
				loop {
					tokio::select! {
						_ = tick.tick() => {
							flush_once(sink.as_ref(), &stats).await;
						}
						_ = token.cancelled() => {
							flush_final(sink.as_ref(), &stats).await;
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
		if tokio::time::timeout(shutdown_timeout, ctx.tasks.wait()).await.is_err() {
			warn!("timed out waiting for tasks to drain; forcing runtime drop");
		}
		Ok(())
	}
}

/// Attempts for the shutdown flush. The periodic flush can hand a rejected
/// batch to the next cycle; the shutdown flush has no later cycle, so it
/// retries in place instead of restoring into a collector that is about to be
/// dropped.
const FINAL_FLUSH_ATTEMPTS: u32 = 3;

/// Backoff between shutdown-flush attempts (bounded: at most
/// `FINAL_FLUSH_ATTEMPTS - 1` of these delay process exit).
const FINAL_FLUSH_RETRY_DELAY: Duration = Duration::from_millis(100);

/// What one flush pass did with the drained batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlushOutcome {
	/// Nothing was pending, so nothing was submitted.
	Idle,
	/// The sink accepted the batch.
	Delivered,
	/// The sink rejected the batch; it was restored into the collector.
	Restored,
}

/// Drain the collector once and submit; restore the batch if the sink fails.
///
/// A restored batch rolls into the next flush cycle, which is why the periodic
/// flush gets a single attempt per tick.
async fn flush_once(sink: &dyn TrafficSink, stats: &StatsCollector) -> FlushOutcome {
	let batch = stats.reset_all();
	if batch.is_empty() {
		return FlushOutcome::Idle;
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

		FlushOutcome::Restored
	} else {
		info!(
			"traffic reported: {} user(s), {}↑, {}↓, {} reqs",
			user_count,
			ByteSize::b(total_upload).display().si(),
			ByteSize::b(total_download).display().si(),
			total_requests
		);

		FlushOutcome::Delivered
	}
}

/// The last flush before the runtime drops the collector.
///
/// Nothing submits a restored batch again once this returns, so a rejected
/// batch is retried in place. If every attempt fails the batch stays in the
/// collector — an externally shared one (`App::set_stats_collector`) keeps
/// reporting those counters — but it is reported as an error, because the sink
/// will never receive it.
async fn flush_final(sink: &dyn TrafficSink, stats: &StatsCollector) {
	for attempt in 1..=FINAL_FLUSH_ATTEMPTS {
		match flush_once(sink, stats).await {
			FlushOutcome::Idle | FlushOutcome::Delivered => return,
			FlushOutcome::Restored => {
				if attempt == FINAL_FLUSH_ATTEMPTS {
					error!(
						"final traffic flush failed after {FINAL_FLUSH_ATTEMPTS} attempts; the batch stays in a collector \
						 that nothing flushes again, so {} user(s) of traffic never reach the sink",
						stats.user_count()
					);

					return;
				}

				warn!(
					"final traffic flush attempt {attempt}/{FINAL_FLUSH_ATTEMPTS} failed; retrying in \
					 {FINAL_FLUSH_RETRY_DELAY:?}"
				);
				tokio::time::sleep(FINAL_FLUSH_RETRY_DELAY).await;
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::{
		sync::{
			Arc, Mutex,
			atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
		},
		time::Duration,
	};

	use async_trait::async_trait;

	use super::*;
	use crate::{FlowContext, RouteAction, UserId, UserTraffic};

	/// Minimal router for unit tests — rejects everything.
	struct StubRouter;

	impl Router for StubRouter {
		async fn route(&self, _ctx: &FlowContext) -> eyre::Result<RouteAction> {
			Ok(RouteAction::Reject("unit test".into()))
		}
	}

	/// Traffic sink that rejects the next `remaining_failures` submits, then
	/// accepts and records what it received.
	struct FlakySink {
		remaining_failures: AtomicUsize,
		attempts: AtomicUsize,
		delivered_upload: AtomicU64,
	}

	impl FlakySink {
		/// A sink that rejects every submit.
		fn always_failing() -> Self {
			Self::failing(usize::MAX)
		}

		fn failing(remaining_failures: usize) -> Self {
			Self {
				remaining_failures: AtomicUsize::new(remaining_failures),
				attempts: AtomicUsize::new(0),
				delivered_upload: AtomicU64::new(0),
			}
		}

		fn attempts(&self) -> usize {
			self.attempts.load(Ordering::SeqCst)
		}

		fn delivered_upload(&self) -> u64 {
			self.delivered_upload.load(Ordering::SeqCst)
		}
	}

	#[async_trait]
	impl TrafficSink for FlakySink {
		async fn submit(&self, batch: Vec<UserTraffic>) -> eyre::Result<()> {
			self.attempts.fetch_add(1, Ordering::SeqCst);
			if self.remaining_failures.load(Ordering::SeqCst) > 0 {
				self.remaining_failures.fetch_sub(1, Ordering::SeqCst);

				return Err(eyre::eyre!("sink unavailable"));
			}

			self.delivered_upload
				.fetch_add(batch.iter().map(|t| t.upload).sum::<u64>(), Ordering::SeqCst);

			Ok(())
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

	/// The historic coupling: without an explicit sampler cadence, the flush
	/// interval still drives sampling, so existing callers keep their behavior.
	#[tokio::test]
	async fn flush_interval_still_drives_the_sampler_when_sample_interval_is_unset() {
		let hooks = run_and_capture_hooks(App::new().set_router(StubRouter).set_flush_interval(Duration::from_secs(7))).await;

		assert_eq!(
			hooks.sample_interval,
			Duration::from_secs(7),
			"an unset sample interval must keep following the flush interval"
		);
	}

	/// Regression (W33): setting the flush interval must not silently move the
	/// sampler once the sampler has its own cadence.
	#[tokio::test]
	async fn sample_interval_is_independent_of_the_flush_interval() {
		let hooks = run_and_capture_hooks(
			App::new()
				.set_router(StubRouter)
				.set_sample_interval(Duration::from_secs(5))
				.set_flush_interval(Duration::from_secs(3600)),
		)
		.await;

		assert_eq!(
			hooks.sample_interval,
			Duration::from_secs(5),
			"the flush interval must not override an explicit sampler cadence"
		);
	}

	#[test]
	fn default_sample_interval_stays_at_sixty_seconds() {
		assert_eq!(
			App::<StubRouter>::new().sample_interval,
			None,
			"an unset sampler cadence is what preserves the historic flush-interval coupling"
		);
		assert_eq!(
			App::<StubRouter>::new().flush_interval,
			Duration::from_secs(60),
			"the pre-W33 sampler cadence is the backward-compatible default"
		);
	}

	#[test]
	fn default_shutdown_timeout_stays_at_ten_seconds() {
		assert_eq!(
			App::<StubRouter>::new().shutdown_timeout,
			Duration::from_secs(10),
			"the pre-W31 hardcoded drain deadline is the backward-compatible default"
		);
	}

	/// A task registered on the app's `TaskTracker` that never returns, so the
	/// final drain can only end on the shutdown deadline.
	#[tokio::test]
	async fn configured_shutdown_timeout_bounds_the_final_drain() {
		let started_task = Arc::new(AtomicBool::new(false));
		let started_task_in_factory = started_task.clone();
		let app = App::new()
			.set_router(StubRouter)
			.set_shutdown_timeout(Duration::from_millis(250))
			.add_inbound_with(move |_hooks, ctx| {
				ctx.tasks.spawn(std::future::pending::<()>());
				started_task_in_factory.store(true, Ordering::SeqCst);
				CaptureInbound
			});

		let ctx = app.context().clone();
		let run_handle = tokio::spawn(async move { app.run().await });
		let deadline = std::time::Instant::now() + Duration::from_secs(5);
		while !started_task.load(Ordering::SeqCst) {
			assert!(std::time::Instant::now() < deadline, "inbound factory never ran");
			tokio::task::yield_now().await;
		}

		let cancelled_at = std::time::Instant::now();
		ctx.token.cancel();
		run_handle.await.expect("run task panicked").expect("run failed");
		let elapsed = cancelled_at.elapsed();

		assert!(
			elapsed >= Duration::from_millis(200),
			"the drain must wait for undrained tasks until the deadline, waited only {elapsed:?}"
		);
		assert!(
			elapsed < Duration::from_secs(5),
			"the configured shutdown timeout must replace the hardcoded 10 s drain, waited {elapsed:?}"
		);
	}

	#[tokio::test]
	async fn a_drained_app_does_not_wait_for_the_shutdown_timeout() {
		let started_at = std::time::Instant::now();
		run_and_capture_hooks(
			App::new()
				.set_router(StubRouter)
				.set_shutdown_timeout(Duration::from_secs(30)),
		)
		.await;

		assert!(
			started_at.elapsed() < Duration::from_secs(5),
			"the shutdown timeout is an upper bound, not a fixed wait: {:?}",
			started_at.elapsed()
		);
	}

	/// An app whose traffic is collected into `stats` and reported to `sink`,
	/// with the periodic flush pushed far out so only the shutdown flush runs.
	fn app_with_traffic(stats: Arc<StatsCollector>, sink: Arc<FlakySink>) -> App<StubRouter> {
		App::new()
			.set_router(StubRouter)
			.set_stats_collector(stats)
			.set_traffic_sink(sink)
			.set_flush_interval(Duration::from_secs(3600))
	}

	/// The periodic flush can let the next tick pick the batch up, so it stays
	/// single-attempt per tick.
	#[tokio::test]
	async fn periodic_flush_leaves_a_rejected_batch_for_the_next_cycle() {
		let sink = Arc::new(FlakySink::always_failing());
		let stats = StatsCollector::new();
		stats.record_upload(&UserId::from("alice"), 4096);

		assert_eq!(flush_once(sink.as_ref(), &stats).await, FlushOutcome::Restored);
		assert_eq!(sink.attempts(), 1, "the periodic flush must not retry in place");
		assert_eq!(
			stats
				.snapshot_user(&UserId::from("alice"))
				.expect("a rejected batch must be restored")
				.upload,
			4096,
			"a rejected batch must roll into the next cycle intact"
		);
	}

	/// The shutdown flush has no later cycle to retry a restored batch in, so
	/// it must retry in place instead of handing the batch to a dying
	/// collector.
	#[tokio::test]
	async fn final_flush_retries_a_transient_sink_failure() {
		let sink = Arc::new(FlakySink::failing(1));
		let stats = Arc::new(StatsCollector::new());
		stats.record_upload(&UserId::from("alice"), 4096);

		let _hooks = run_and_capture_hooks(app_with_traffic(stats.clone(), sink.clone())).await;

		assert_eq!(
			sink.delivered_upload(),
			4096,
			"the shutdown flush must retry a rejected batch in place: {} attempt(s)",
			sink.attempts()
		);
		assert_eq!(sink.attempts(), 2);
		assert!(
			stats.snapshot().is_empty(),
			"a delivered batch must not stay in the collector"
		);
	}

	/// When every shutdown attempt fails the batch cannot be delivered, but it
	/// must stay readable on a shared collector and must not be counted twice.
	#[tokio::test]
	async fn final_flush_restores_an_undeliverable_batch_exactly_once() {
		let sink = Arc::new(FlakySink::always_failing());
		let stats = Arc::new(StatsCollector::new());
		stats.record_upload(&UserId::from("alice"), 4096);

		let _hooks = run_and_capture_hooks(app_with_traffic(stats.clone(), sink.clone())).await;

		assert!(
			sink.attempts() >= 2,
			"the shutdown flush must retry before declaring the batch lost"
		);
		assert_eq!(sink.delivered_upload(), 0);
		let batch = stats.snapshot();
		assert_eq!(batch.len(), 1, "the undeliverable batch must stay readable");
		assert_eq!(batch[0].upload, 4096, "retrying must not double count the restored batch");
	}
}
