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
	/// Per-user tally of `inner`, maintained incrementally by `register` /
	/// `deregister` so that `count_for` is a single lookup instead of a scan
	/// over every live connection — a host's per-user limit hook runs it on
	/// every authentication.
	///
	/// Invariant: for every user, `per_user[user]` equals the number of `inner`
	/// entries owned by that user. `kick_user` cancels tokens *without*
	/// removing entries, so a kicked connection keeps counting until it
	/// deregisters. Entries are dropped once they reach zero so one-shot
	/// users cannot grow this map without bound.
	///
	/// A host's per-user limit is necessarily best-effort: `count_for` is
	/// advisory, and a caller that checks it and registers afterwards (no
	/// caller can hold this map's locks across both steps) leaves a window in
	/// which peers authenticating together can each observe the same count.
	/// The tally itself never drifts from `inner` — see the test module.
	per_user: Arc<DashMap<UserId, usize>>,
}

impl ActiveConnections {
	pub fn new() -> Self {
		Self::default()
	}

	/// Register a live connection. Cancelling `token` closes the connection.
	pub fn register(&self, conn_id: u64, user: UserId, token: CancellationToken) {
		// Bump the tally first so a concurrent limit check can only over-count
		// (fail-closed), never miss a connection that is already registered.
		*self.per_user.entry(user.clone()).or_insert(0) += 1;
		// A reused id would otherwise leave the previous owner double-counted.
		if let Some((previous, _)) = self.inner.insert(conn_id, (user, token)) {
			self.release(&previous);
		}
	}

	/// Remove a connection from the registry (call on connection close).
	pub fn deregister(&self, conn_id: u64) {
		// `DashMap::remove` hands back the key as well as the value.
		if let Some((_, (user, _))) = self.inner.remove(&conn_id) {
			self.release(&user);
		}
	}

	/// Number of live connections currently attributed to `user`. O(1).
	pub fn count_for(&self, user: &UserId) -> usize {
		self.per_user.get(user).map_or(0, |count| *count)
	}

	/// Drop one connection from `user`'s tally, removing the tally entry once
	/// the user has no live connection left.
	fn release(&self, user: &UserId) {
		if let Some(mut count) = self.per_user.get_mut(user) {
			*count = count.saturating_sub(1);
		}
		// Re-check under the shard lock instead of trusting the value read
		// above: a concurrent `register` may have already re-added the user.
		self.per_user.remove_if(user, |_, count| *count == 0);
	}

	/// Cancel every live connection belonging to `user`. Returns how many were
	/// kicked.
	///
	/// Deliberately *keeps* the entries registered: the kicked connections own
	/// their registry rows until they finish unwinding and call `deregister`,
	/// so `count_for` keeps counting them meanwhile. That is the fail-closed
	/// direction for a per-user limit — a kick must not open budget for a
	/// replacement connection while the kicked one can still be relaying
	/// traffic. Removing the rows here would instead make the limit
	/// momentarily permissive. A connection that never deregisters (for
	/// example a handler that panicked before its cleanup) therefore keeps
	/// counting until the process restarts; recovery is the caller's job.
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

	/// Oracle for the incremental tally: the definition `count_for` used to
	/// compute on every call, recomputed straight from the registry.
	fn scanned_count(active: &ActiveConnections, user: &UserId) -> usize {
		active.inner.iter().filter(|entry| &entry.value().0 == user).count()
	}

	#[test]
	fn counts_stay_equal_to_the_registry_under_churn() {
		let active = ActiveConnections::new();
		let users: Vec<UserId> = (0..4).map(|i| UserId::from(format!("user-{i}"))).collect();
		let user_at = |conn_id: u64| users[(conn_id % 4) as usize].clone();

		for conn_id in 0..500u64 {
			active.register(conn_id, user_at(conn_id), CancellationToken::new());
		}
		for user in &users {
			assert_eq!(active.count_for(user), scanned_count(&active, user), "drifted for {user}");
		}

		// Re-registering a live id hands it to another owner: the previous
		// owner's tally must drop, the new owner's must rise.
		for conn_id in 0..500u64 {
			active.register(conn_id, user_at(conn_id + 1), CancellationToken::new());
		}
		// Deregistering an id that was never registered must not disturb any
		// tally.
		active.deregister(9_999);
		for user in &users {
			assert_eq!(active.count_for(user), scanned_count(&active, user), "drifted for {user}");
		}
		assert_eq!(active.count_for(&UserId::from("nobody")), 0);

		for conn_id in 0..500u64 {
			active.deregister(conn_id);
		}
		for user in &users {
			assert_eq!(active.count_for(user), 0);
			assert_eq!(scanned_count(&active, user), 0);
			// A one-shot user must not leave a tally entry behind.
			assert!(!active.per_user.contains_key(user), "leaked a tally entry for {user}");
		}
		assert_eq!(active.len(), 0);
		assert!(active.is_empty());
		assert!(active.per_user.is_empty());
	}

	/// Pins the deliberate half of `kick_user`'s contract: cancelling the token
	/// does not hand the user's registered rows (or its tally) back early, so a
	/// cancelled-but-still-unwinding connection cannot be replaced under a
	/// per-user limit. The rows disappear only when the connection itself
	/// deregisters.
	#[test]
	fn a_kicked_connection_keeps_counting_until_it_deregisters() {
		let active = ActiveConnections::new();
		let user = UserId::from("u1");
		let token = CancellationToken::new();
		active.register(7, user.clone(), token.clone());

		assert_eq!(active.kick_user(&user), 1);
		assert!(token.is_cancelled());
		// The registry row survives the kick, and the tally still mirrors it.
		assert!(active.inner.contains_key(&7));
		assert_eq!(active.len(), 1);
		assert_eq!(active.count_for(&user), 1);
		assert_eq!(scanned_count(&active, &user), 1);
		// A repeated kick finds the same row and changes nothing else.
		assert_eq!(active.kick_user(&user), 1);
		assert_eq!(active.count_for(&user), 1);

		// The kick does not disturb other users.
		let other = UserId::from("u2");
		let other_token = CancellationToken::new();
		active.register(8, other.clone(), other_token.clone());
		assert_eq!(active.kick_user(&user), 1);
		assert!(!other_token.is_cancelled());
		assert_eq!(active.count_for(&other), 1);

		// Only the connection's own deregistration frees the budget again.
		active.deregister(7);
		assert_eq!(active.count_for(&user), 0);
		assert!(!active.inner.contains_key(&7));
		assert!(!active.per_user.contains_key(&user), "leaked a tally entry for {user}");
		// A late second deregistration for the same id stays harmless.
		active.deregister(7);
		assert_eq!(active.count_for(&other), 1);
		assert_eq!(active.len(), 1);
	}
}
