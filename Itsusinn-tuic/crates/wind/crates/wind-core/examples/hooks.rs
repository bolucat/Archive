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
use wind_acl::AclEngine;
use wind_core::{
	App, ConnInfo, ConnectDecision, ConnectionHooks, FlowContext, Outbound, TrafficSink, TuicAuthenticator, UserId,
	UserTraffic, tcp::AbstractTcpStream, udp::UdpStream,
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
///
/// `active` holds only users with at least one live connection: the last
/// `on_disconnect` removes the row, so the map stays proportional to the live
/// connections rather than to every user ever seen.
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
		// `get_mut` above only counts down, so a user whose last connection
		// closed would keep a zero entry forever: the map would grow by one row
		// per distinct user ever seen, for the life of the process. Drop the
		// row once the user has no live connection left. `remove_if`
		// re-checks the value under the shard lock, so it cannot erase
		// a count that a concurrent `on_authenticated` has already
		// raised again — in that case the row is left in place and the
		// map still matches the live count.
		if let Some(user) = user {
			self.active.remove_if(user, |_, count| *count == 0);
		}
	}
}

/// A no-op outbound so the example assembles a complete dispatcher.
struct NoopOutbound;

#[async_trait]
impl Outbound for NoopOutbound {
	async fn handle_tcp(&self, _ctx: FlowContext, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		Ok(())
	}

	async fn handle_udp(&self, _ctx: FlowContext, _stream: UdpStream) -> eyre::Result<()> {
		Ok(())
	}
}

fn main() {
	let mut users = HashMap::new();
	users.insert(Uuid::nil(), Arc::from(b"super-secret".as_slice()));

	let app = App::<AclEngine>::new()
		.set_router(AclEngine::builder("direct").build().unwrap())
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

#[cfg(test)]
mod tests {
	//! The limit hook is example code, so it is not covered by the library
	//! tests; these cases pin the two properties a copy of it must keep: the
	//! veto, and a book-keeping map that does not grow with the number of users
	//! ever seen.

	use std::sync::atomic::Ordering;

	use wind_core::Protocol;

	use super::*;

	fn limiter(limit: usize) -> PerUserLimit {
		PerUserLimit {
			limit,
			active: DashMap::new(),
			rejected: AtomicUsize::new(0),
		}
	}

	fn info(conn_id: u64) -> ConnInfo {
		ConnInfo {
			remote_addr: "127.0.0.1:1000".parse().unwrap(),
			protocol: Protocol::Tuic,
			conn_id,
		}
	}

	/// A user's last disconnect must leave no row behind, otherwise a
	/// long-lived process accumulates one entry per user ever seen.
	#[tokio::test]
	async fn disconnect_of_the_last_connection_drops_the_entry() {
		let limit = limiter(2);
		let user = UserId::from("alice");

		assert!(limit.on_authenticated(&info(1), &user).await.is_accept());
		limit.on_disconnect(&info(1), Some(&user)).await;
		assert!(limit.active.is_empty(), "a fully disconnected user must not stay in the map");

		// A one-shot user must not accumulate either.
		for conn_id in 2..100 {
			let one_shot = UserId::from(format!("user-{conn_id}"));
			assert!(limit.on_authenticated(&info(conn_id), &one_shot).await.is_accept());
			limit.on_disconnect(&info(conn_id), Some(&one_shot)).await;
		}
		assert_eq!(limit.active.len(), 0);
	}

	/// Dropping the zero row must not hand a still-connected user's budget back
	/// early: with the limit reached, a new connection is still vetoed.
	#[tokio::test]
	async fn dropping_zero_rows_keeps_the_limit_enforced() {
		let limit = limiter(1);
		let user = UserId::from("bob");

		assert!(limit.on_authenticated(&info(1), &user).await.is_accept());
		// Another user disconnecting in between must not clear bob's count.
		let other = UserId::from("carol");
		assert!(limit.on_authenticated(&info(2), &other).await.is_accept());
		limit.on_disconnect(&info(2), Some(&other)).await;

		assert!(!limit.on_authenticated(&info(3), &user).await.is_accept());
		assert_eq!(limit.rejected.load(Ordering::Relaxed), 1);
		assert_eq!(limit.active.get(&user).map(|c| *c), Some(1));

		// Only bob's own last disconnect frees the budget.
		limit.on_disconnect(&info(1), Some(&user)).await;
		assert!(limit.active.is_empty());
		assert!(limit.on_authenticated(&info(4), &user).await.is_accept());
	}

	/// A disconnect that never authenticated (`user == None`) and a disconnect
	/// for an unknown user must stay harmless: no insertion, no panic.
	#[tokio::test]
	async fn disconnect_without_a_user_is_a_no_op() {
		let limit = limiter(1);
		limit.on_disconnect(&info(1), None).await;
		limit.on_disconnect(&info(2), Some(&UserId::from("ghost"))).await;
		assert!(limit.active.is_empty());
	}
}
