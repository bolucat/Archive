//! Registry of live connections, keyed by a process-unique connection id, used
//! for per-user connection limiting and active kicking.
//!
//! Shared (as a cheap `Arc` handle) between an inbound — which registers each
//! connection with a [`CancellationToken`] and deregisters on close — and a
//! host binary's hooks, which read the per-user count (for limits) and cancel a
//! user's connections (for kicks, e.g. when a panel removes a user). Used by
//! both the naive (per-CONNECT-tunnel) and TUIC (per-authenticated-connection)
//! inbounds.

use std::sync::Arc;

use dashmap::DashMap;
use tokio_util::sync::CancellationToken;

use crate::UserId;

#[derive(Clone, Default)]
pub struct ActiveConnections {
	inner: Arc<DashMap<u64, (UserId, CancellationToken)>>,
}

impl ActiveConnections {
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a live connection. Cancelling `token` closes the connection.
	pub fn register(&self, conn_id: u64, user: UserId, token: CancellationToken) {
		self.inner.insert(conn_id, (user, token));
	}

	/// Remove a connection from the registry (call on connection close).
	pub fn deregister(&self, conn_id: u64) {
		self.inner.remove(&conn_id);
	}

	/// Number of live connections currently attributed to `user`.
	pub fn count_for(&self, user: &UserId) -> usize {
		self.inner.iter().filter(|e| &e.value().0 == user).count()
	}

	/// Cancel every live connection belonging to `user`. Returns how many were
	/// kicked.
	pub fn kick_user(&self, user: &UserId) -> usize {
		let mut kicked = 0;
		for entry in self.inner.iter() {
			if &entry.value().0 == user {
				entry.value().1.cancel();
				kicked += 1;
			}
		}
		kicked
	}

	/// Total live connections (diagnostics).
	pub fn len(&self) -> usize {
		self.inner.len()
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_empty()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn count_and_kick() {
		let active = ActiveConnections::new();
		let u1 = UserId::from("u1");
		let u2 = UserId::from("u2");
		let t1 = CancellationToken::new();
		let t2 = CancellationToken::new();
		let t3 = CancellationToken::new();

		active.register(1, u1.clone(), t1.clone());
		active.register(2, u1.clone(), t2.clone());
		active.register(3, u2.clone(), t3.clone());

		assert_eq!(active.count_for(&u1), 2);
		assert_eq!(active.count_for(&u2), 1);

		// Kicking u1 cancels both of its tokens, not u2's.
		assert_eq!(active.kick_user(&u1), 2);
		assert!(t1.is_cancelled());
		assert!(t2.is_cancelled());
		assert!(!t3.is_cancelled());

		// Deregister mirrors a connection closing.
		active.deregister(1);
		active.deregister(2);
		assert_eq!(active.count_for(&u1), 0);
		assert_eq!(active.count_for(&u2), 1);
	}
}
