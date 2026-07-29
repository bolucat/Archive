//! Customizing `wind`'s inbound behavior via the hooks / `App` builder.
//!
//! Demonstrates the three downstream extension points when `wind` is used as a
//! library:
//!
//! 1. **Authentication** — a custom [`TuicAuthenticator`] (here a static map,
//!    but it could hit a database or an external service).
//! 2. **Traffic statistics** — a [`TrafficSink`] that receives periodic
//!    per-user batches drained from the central collector.
//! 3. **Connection management** — a [`ConnectionHooks`] enforcing a per-user
//!    concurrent-connection limit.
//!
//! Run with: `cargo run -p wind-core --example hooks`

use std::{
	collections::HashMap,
	sync::{
		Arc,
		atomic::{AtomicUsize, Ordering},
	},
};

use async_trait::async_trait;
use dashmap::DashMap;
use uuid::Uuid;
use wind_core::{
	AclRouter, App, ConnInfo, ConnectDecision, ConnectionHooks, OutboundAction, TrafficSink, TuicAuthenticator, UserId,
	UserTraffic, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream,
};

/// Authentication backed by an in-memory map (stand-in for a DB lookup).
struct MyAuth {
	users: HashMap<Uuid, Arc<[u8]>>,
}

#[async_trait]
impl TuicAuthenticator for MyAuth {
	async fn lookup(&self, uuid: &Uuid) -> Option<(UserId, Arc<[u8]>)> {
		self.users.get(uuid).map(|pw| (UserId::from(*uuid), pw.clone()))
	}
}

/// Traffic sink that just logs each flush cycle's batch (a real one would write
/// to a metrics system / billing panel).
struct LoggingSink;

#[async_trait]
impl TrafficSink for LoggingSink {
	async fn submit(&self, batch: Vec<UserTraffic>) -> eyre::Result<()> {
		for t in batch {
			println!(
				"[traffic] user={} up={} down={} requests={}",
				t.user_id, t.upload, t.download, t.request_count
			);
		}
		Ok(())
	}
}

/// Rejects a user's connection once it already has `limit` concurrent ones.
struct PerUserLimit {
	limit: usize,
	active: DashMap<UserId, usize>,
	rejected: AtomicUsize,
}

#[async_trait]
impl ConnectionHooks for PerUserLimit {
	async fn on_authenticated(&self, _info: &ConnInfo, user: &UserId) -> ConnectDecision {
		let mut count = self.active.entry(user.clone()).or_insert(0);
		if *count >= self.limit {
			self.rejected.fetch_add(1, Ordering::Relaxed);
			return ConnectDecision::Reject(format!("user {user} over connection limit"));
		}
		*count += 1;
		ConnectDecision::Accept
	}

	async fn on_disconnect(&self, _info: &ConnInfo, user: Option<&UserId>) {
		if let Some(user) = user
			&& let Some(mut count) = self.active.get_mut(user)
		{
			*count = count.saturating_sub(1);
		}
	}
}

/// A no-op outbound so the example assembles a complete dispatcher.
struct NoopOutbound;

#[async_trait]
impl OutboundAction for NoopOutbound {
	async fn handle_tcp(&self, _target: TargetAddr, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		Ok(())
	}

	async fn handle_udp(&self, _stream: UdpStream) -> eyre::Result<()> {
		Ok(())
	}
}

fn main() {
	let mut users = HashMap::new();
	users.insert(Uuid::nil(), Arc::from(b"super-secret".as_slice()));

	let app = App::new()
		.set_router(AclRouter::new(Vec::new(), "direct"))
		.add_outbound("direct", Arc::new(NoopOutbound))
		.set_tuic_authenticator(Arc::new(MyAuth { users })) // feature 1
		.set_traffic_sink(Arc::new(LoggingSink)) // feature 2
		.add_connection_hooks(Arc::new(PerUserLimit {
			limit: 2,
			active: DashMap::new(),
			rejected: AtomicUsize::new(0),
		})); // feature 3
	// A real program would also register inbounds and drive the runtime:
	//
	//     app.add_inbound_with(|hooks, ctx| {
	//         TuicInbound::new(ctx, TuicInboundOpts { hooks, ..tuic_opts })
	//     })
	//     .run().await?;
	let _ = app;

	println!("App configured with auth, traffic-stats, and per-user connection-limit hooks.");
}
