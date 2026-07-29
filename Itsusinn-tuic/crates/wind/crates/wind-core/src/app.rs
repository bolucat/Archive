//! Bevy-style `App` / `Plugin` builder for assembling a `wind` runtime.
//!
//! Register outbounds, a router, inbound factories, and [`hooks`](crate::hooks)
//! (auth / traffic / connection management), optionally grouped into
//! [`Plugin`]s, then [`App::run`] wires a [`Dispatcher`], spawns every inbound
//! plus the traffic-flush task, and drives graceful shutdown.
//!
//! Inbounds are supplied as factory closures so the finalized [`InboundHooks`]
//! bundle can be threaded into each one's opts at `run` time — this keeps the
//! builder decoupled from the concrete protocol crates (no circular deps).

use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};

use async_trait::async_trait;
use bytesize::ByteSize;
use tracing::{error, info, warn};

use crate::{
	AbstractInbound, AppContext, Dispatcher, OutboundAction, RouteAction, Router,
	hooks::{
		ConnectionHooks, FanOutConnectionHooks, InboundHooks, StatsCollector, TrafficSink, TuicAuthenticator,
		UserPassAuthenticator,
	},
	types::TargetAddr,
};

/// Object-safe form of [`Router`] so the App can store a `dyn` router.
#[async_trait]
pub trait DynRouter: Send + Sync + 'static {
	async fn route_dyn(&self, target: &TargetAddr, is_tcp: bool) -> eyre::Result<RouteAction>;
}

#[async_trait]
impl<R: Router> DynRouter for R {
	async fn route_dyn(&self, target: &TargetAddr, is_tcp: bool) -> eyre::Result<RouteAction> {
		self.route(target, is_tcp).await
	}
}

/// Adapts an `Arc<dyn DynRouter>` back into a concrete [`Router`], so the App's
/// dispatcher type is the single concrete `Dispatcher<ArcRouter>`.
#[derive(Clone)]
pub struct ArcRouter(Arc<dyn DynRouter>);

impl Router for ArcRouter {
	fn route(&self, target: &TargetAddr, is_tcp: bool) -> impl Future<Output = eyre::Result<RouteAction>> + Send {
		let inner = self.0.clone();
		let target = target.clone();
		async move { inner.route_dyn(&target, is_tcp).await }
	}
}

/// Object-safe form of [`AbstractInbound`] bound to the App's dispatcher type.
#[async_trait]
pub trait DynInbound: Send + Sync {
	async fn listen_dyn(&self, cb: Dispatcher<ArcRouter>) -> eyre::Result<()>;
}

#[async_trait]
impl<T: AbstractInbound + Send + Sync> DynInbound for T {
	async fn listen_dyn(&self, cb: Dispatcher<ArcRouter>) -> eyre::Result<()> {
		self.listen(&cb).await
	}
}

type InboundFactory = Box<dyn FnOnce(InboundHooks, Arc<AppContext>) -> Box<dyn DynInbound> + Send>;

/// A composable unit of configuration, applied to the [`App`] via
/// [`App::add_plugin`].
pub trait Plugin {
	fn build(self, app: App) -> App;
}

/// The runtime builder. Construct with [`App::new`], register everything, then
/// [`App::run`].
pub struct App {
	ctx: Arc<AppContext>,
	outbounds: HashMap<String, Arc<dyn OutboundAction>>,
	router: Option<Arc<dyn DynRouter>>,
	tuic_auth: Option<Arc<dyn TuicAuthenticator>>,
	userpass_auth: Option<Arc<dyn UserPassAuthenticator>>,
	conn_hooks: Vec<Arc<dyn ConnectionHooks>>,
	traffic_sink: Option<Arc<dyn TrafficSink>>,
	flush_interval: Duration,
	inbounds: Vec<InboundFactory>,
}

impl Default for App {
	fn default() -> Self {
		Self::new()
	}
}

impl App {
	pub fn new() -> Self {
		Self {
			ctx: Arc::new(AppContext::default()),
			outbounds: HashMap::new(),
			router: None,
			tuic_auth: None,
			userpass_auth: None,
			conn_hooks: Vec::new(),
			traffic_sink: None,
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

	pub fn add_plugin(self, plugin: impl Plugin) -> Self {
		plugin.build(self)
	}

	pub fn add_outbound(mut self, name: impl Into<String>, handler: Arc<dyn OutboundAction>) -> Self {
		self.outbounds.insert(name.into(), handler);
		self
	}

	pub fn set_router(mut self, router: impl Router) -> Self {
		self.router = Some(Arc::new(router) as Arc<dyn DynRouter>);
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
	/// collection (a shared [`StatsCollector`] + the periodic flush task).
	pub fn set_traffic_sink(mut self, sink: Arc<dyn TrafficSink>) -> Self {
		self.traffic_sink = Some(sink);
		self
	}

	/// Flush/sample cadence for traffic stats (default 60s).
	pub fn set_flush_interval(mut self, interval: Duration) -> Self {
		self.flush_interval = interval;
		self
	}

	/// Register an inbound via a factory that receives the finalized hooks
	/// bundle and the shared context.
	pub fn add_inbound_with<I, F>(mut self, factory: F) -> Self
	where
		I: AbstractInbound + Send + Sync + 'static,
		F: FnOnce(InboundHooks, Arc<AppContext>) -> I + Send + 'static,
	{
		self.inbounds.push(Box::new(move |hooks, ctx| {
			Box::new(factory(hooks, ctx)) as Box<dyn DynInbound>
		}));
		self
	}

	/// Finalize the hooks bundle from the registered pieces.
	fn build_hooks(&self, stats: Option<Arc<StatsCollector>>) -> InboundHooks {
		let connection = match self.conn_hooks.len() {
			0 => None,
			1 => Some(self.conn_hooks[0].clone()),
			_ => Some(Arc::new(FanOutConnectionHooks(self.conn_hooks.clone())) as Arc<dyn ConnectionHooks>),
		};
		InboundHooks {
			tuic_auth: self.tuic_auth.clone(),
			userpass_auth: self.userpass_auth.clone(),
			connection,
			stats,
			sample_interval: self.flush_interval,
		}
	}

	/// Build the dispatcher, spawn the flush task and every inbound, then run
	/// until Ctrl-C (or the context token is cancelled), draining in-flight
	/// connection handlers on the way out.
	pub async fn run(self) -> eyre::Result<()> {
		let router = self.router.clone().ok_or_else(|| eyre::eyre!("App::run: no router set"))?;

		// Stats are enabled iff a sink was registered.
		let stats = self.traffic_sink.as_ref().map(|_| Arc::new(StatsCollector::new()));
		let hooks = self.build_hooks(stats.clone());

		let mut dispatcher = Dispatcher::new(ArcRouter(router));
		for (name, handler) in &self.outbounds {
			dispatcher.add_handler(name.clone(), handler.clone());
		}

		// Periodic traffic flush (drains the collector → sink, restore on error,
		// final flush on shutdown).
		if let (Some(stats), Some(sink)) = (stats.clone(), self.traffic_sink.clone()) {
			let token = self.ctx.token.clone();
			let interval = self.flush_interval;
			self.ctx.tasks.spawn(async move {
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
		for factory in self.inbounds {
			let inbound = factory(hooks.clone(), self.ctx.clone());
			let dispatcher = dispatcher.clone();
			self.ctx.tasks.spawn(async move {
				if let Err(e) = inbound.listen_dyn(dispatcher).await {
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
			_ = self.ctx.token.cancelled() => {
				info!("shutdown signalled, stopping");
			}
		}

		self.ctx.token.cancel();
		self.ctx.tasks.close();
		if tokio::time::timeout(Duration::from_secs(10), self.ctx.tasks.wait())
			.await
			.is_err()
		{
			warn!("shutdown drain timed out after 10s; some tasks may not have finished");
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
