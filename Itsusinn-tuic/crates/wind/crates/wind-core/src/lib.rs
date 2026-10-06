pub mod active;
pub mod app;
pub mod dispatcher;
pub mod flow;
pub mod hooks;
pub mod inbound;
mod interface;
pub mod io;
mod outbound;
pub mod resolve;
pub use wind_rule as rule;
pub mod signal;
pub mod types;

pub use active::ActiveConnections;
pub use app::{App, Plugin};
pub use dispatcher::{Dispatcher, RouteAction, Router};
pub use flow::FlowContext;
pub use hooks::{
	ConnInfo, ConnectDecision, ConnectionHooks, InboundHooks, Protocol, StaticTuicAuth, StaticUserPass, StatsCollector,
	TrafficSink, TuicAuthenticator, UserId, UserPassAuthenticator, UserTraffic,
};
pub use inbound::*;
pub use interface::*;
pub use outbound::*;
pub use resolve::{Resolver, SystemResolver};
pub use signal::shutdown_signal;
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub mod quic;
pub mod tcp;
pub mod udp;
pub mod utils;

pub use quic::{QuicCongestionControl, parse_congestion_control};
pub use utils::{StackPrefer, is_private_ip};

#[cfg(test)]
mod udp_tests;

/// Shared task ownership and cancellation for one runtime.
///
/// Everything spawned through `tasks` is drained by the owning `App::run` on
/// shutdown, so any component that spawns connection or session work must be
/// given the live context — inbounds receive it from their factory, and a
/// [`Dispatcher`](crate::Dispatcher) takes it through
/// [`Dispatcher::context`](crate::Dispatcher::context). A component left on a
/// private default context is cancelled but never awaited.
pub struct AppContext {
	pub tasks: TaskTracker,
	pub token: CancellationToken,
}

impl Default for AppContext {
	fn default() -> Self {
		Self {
			tasks: TaskTracker::new(),
			token: CancellationToken::new(),
		}
	}
}
