//! Wind framework [`Plugin`] for the wind binary.
//!
//! Assembles the configured outbounds (tuic / naive / load-balance), the
//! default router, and the SOCKS5 inbound into a single composable [`App`]
//! via [`wind_core::App`], matching how tuic-server / tuic-client wire their
//! runtimes.

use std::{collections::HashMap, sync::Arc};

use tracing::info;
use wind_base::load_balance::{LoadBalanceOpts, LoadBalanceOutbound};
use wind_core::{App, AppContext, InboundHooks, Outbound, Plugin, Router};
use wind_naive::NaiveOutbound;
use wind_socks::inbound::SocksInbound;
use wind_tuic::quinn::outbound::TuicOutbound;

use crate::conf::resolved::{ResolvedConfig, ResolvedInboundOpts, ResolvedOutbound};

/// Router — always forwards to the first outbound (TODO: ACL rules).
#[derive(Clone)]
pub struct DefaultRouter {
	pub default: String,
}

impl Router for DefaultRouter {
	#[allow(clippy::manual_async_fn)]
	fn route(
		&self,
		_ctx: &wind_core::FlowContext,
	) -> impl std::future::Future<Output = eyre::Result<wind_core::RouteAction>> + Send {
		async move { Ok(wind_core::RouteAction::Forward(self.default.clone())) }
	}
}

/// The application-level router: a closed set of routing policies for the wind
/// binary. Dispatched statically (hand-written `match`, no vtable).
#[derive(Clone)]
pub enum WindRouter {
	Default(DefaultRouter),
}

#[allow(clippy::manual_async_fn)]
impl Router for WindRouter {
	fn route(
		&self,
		ctx: &wind_core::FlowContext,
	) -> impl std::future::Future<Output = eyre::Result<wind_core::RouteAction>> + Send {
		async move {
			match self {
				WindRouter::Default(r) => r.route(ctx).await,
			}
		}
	}
}

/// Wind framework plugin that wires the configured runtime.
///
/// Outbounds arrive in [`ResolvedConfig`] dependency order, so a single pass
/// builds every outbound: a load-balance group always finds its child proxies
/// already registered.
pub struct WindPlugin {
	cfg: ResolvedConfig,
}

impl WindPlugin {
	pub fn new(cfg: ResolvedConfig) -> Self {
		Self { cfg }
	}
}

impl Plugin<WindRouter> for WindPlugin {
	async fn build(self, app: App<WindRouter>) -> eyre::Result<App<WindRouter>> {
		let ctx = app.context().clone();

		let mut handlers: HashMap<String, Arc<dyn Outbound>> = HashMap::new();
		for ob in self.cfg.outbounds {
			match ob {
				ResolvedOutbound::Tuic { tag, opts } => {
					let out = TuicOutbound::new(ctx.clone(), opts).await?;
					handlers.insert(tag.clone(), Arc::new(out));
					info!(target: "wind_boot", "outbound '{tag}' [tuic]");
				}
				ResolvedOutbound::Naive { tag, opts } => {
					let out = NaiveOutbound::new(opts).await?;
					handlers.insert(tag.clone(), Arc::new(out));
					info!(target: "wind_boot", "outbound '{tag}' [naive]");
				}
				ResolvedOutbound::LoadBalance { tag, opts } => {
					// Reference existence and ordering are guaranteed by
					// `ResolvedConfig`; the defensive error below is a
					// fail-safe, not a user-facing path.
					let children: Vec<Arc<dyn Outbound>> = opts
						.proxies
						.iter()
						.map(|t| {
							handlers.get(t).cloned().ok_or_else(|| {
								eyre::eyre!(
									"load-balance '{tag}' references proxy '{t}' that is not yet built (internal ordering \
									 error)"
								)
							})
						})
						.collect::<eyre::Result<Vec<_>>>()?;

					let lb_out = LoadBalanceOutbound::new(
						LoadBalanceOpts {
							strategy: opts.strategy,
							url: opts.url,
							interval: opts.interval,
							lazy: opts.lazy,
						},
						children,
					);
					let lb_arc = Arc::new(lb_out);
					if !opts.lazy {
						lb_arc.start_health_check(opts.interval);
					}
					handlers.insert(tag.clone(), lb_arc);
					info!(target: "wind_boot", "outbound '{tag}' [load-balance]");
				}
			}
		}

		let mut app = app.set_router(WindRouter::Default(DefaultRouter {
			default: self.cfg.default_tag,
		}));
		for (name, handler) in handlers {
			app = app.add_outbound(name, handler);
		}

		// Inbounds: materialized with the finalized hooks bundle + shared ctx.
		for ib in self.cfg.inbounds {
			let tag = ib.tag;
			match ib.opts {
				ResolvedInboundOpts::Socks(opts) => {
					let listen_addr = opts.listen_addr;
					app = app.add_inbound_with(move |hooks: InboundHooks, ctx: Arc<AppContext>| {
						let mut opts = opts;
						opts.hooks = hooks;
						SocksInbound::new(opts, ctx.token.child_token())
					});
					info!(target: "wind_boot", "inbound '{tag}' [socks] ({listen_addr})");
				}
			}
		}

		Ok(app)
	}
}

#[cfg(test)]
mod tests {
	use wind_core::{FlowContext, RouteAction, rule::NetworkType, types::TargetAddr};

	use super::*;

	#[tokio::test]
	async fn wind_router_forwards_to_configured_default() {
		let router = WindRouter::Default(DefaultRouter {
			default: "main".to_string(),
		});
		let ctx = FlowContext {
			target: TargetAddr::Domain("example.com".into(), 443),
			network: NetworkType::Tcp,
			source: None,
			inbound_tag: "socks".into(),
			protocol: wind_core::Protocol::Socks5,
			user: None,
			inbound_port: None,
			inbound_type: None,
		};
		let action = router.route(&ctx).await.unwrap();
		assert!(matches!(action, RouteAction::Forward(name) if name == "main"));
	}
}
