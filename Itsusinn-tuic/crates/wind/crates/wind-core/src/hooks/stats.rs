//! Lock-free per-user traffic statistics.
//!
//! Ported from naive-rs (`deps/xpanel/xpanel-core/src/stats.rs`) and re-keyed
//! on [`UserId`] instead of an `i64` panel id. The collector is written to
//! continuously as bytes flow (one atomic `fetch_add` per IO buffer / packet /
//! sampler tick), and drained periodically by a flush task via [`reset_all`],
//! which atomically swaps each user's counters to zero and returns the deltas.
//! A failed submit can be rolled back with [`restore`] so no bytes are lost.
//!
//! [`reset_all`]: StatsCollector::reset_all
//! [`restore`]: StatsCollector::restore

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use dashmap::DashMap;

use super::UserId;

/// Per-user counters held in the collector's map.
#[derive(Debug, Default)]
struct UserStatsData {
	upload_bytes: AtomicU64,
	download_bytes: AtomicU64,
	request_count: AtomicU64,
}

/// One flush cycle's traffic for a single user.
///
/// `upload` is client→proxy, `download` is proxy→client.
#[derive(Debug, Clone)]
pub struct UserTraffic {
	pub user_id: UserId,
	pub upload: u64,
	pub download: u64,
	pub request_count: u64,
}

/// Lock-free traffic statistics collector.
///
/// Cheap concurrent writes (sharded `DashMap` + per-user atomics) and a
/// serialized, swap-based drain. Share it as `Arc<StatsCollector>`.
pub struct StatsCollector {
	stats: DashMap<UserId, UserStatsData>,
	/// Serializes concurrent `reset_all` callers so two drains can't race.
	resetting: AtomicBool,
}

impl Default for StatsCollector {
	fn default() -> Self {
		Self::new()
	}
}

/// Resets `resetting` to `false` on drop, even if `reset_all` panics mid-drain.
struct ResetGuard<'a>(&'a AtomicBool);

impl Drop for ResetGuard<'_> {
	fn drop(&mut self) {
		self.0.store(false, Ordering::Release);
	}
}

impl StatsCollector {
	pub fn new() -> Self {
		Self {
			stats: DashMap::new(),
			resetting: AtomicBool::new(false),
		}
	}

	/// Record a proxy request (one TCP connect / UDP associate) for a user.
	pub fn record_request(&self, user: &UserId) {
		self.entry(user).request_count.fetch_add(1, Ordering::Relaxed);
	}

	/// Record upload bytes (client → proxy).
	pub fn record_upload(&self, user: &UserId, bytes: u64) {
		self.entry(user).upload_bytes.fetch_add(bytes, Ordering::Relaxed);
	}

	/// Record download bytes (proxy → client).
	pub fn record_download(&self, user: &UserId, bytes: u64) {
		self.entry(user).download_bytes.fetch_add(bytes, Ordering::Relaxed);
	}

	/// Record upload + download in a single shard-lock acquisition.
	pub fn record_bytes(&self, user: &UserId, upload: u64, download: u64) {
		let entry = self.entry(user);
		entry.upload_bytes.fetch_add(upload, Ordering::Relaxed);
		entry.download_bytes.fetch_add(download, Ordering::Relaxed);
	}

	/// Non-resetting read of one user's cumulative counters.
	pub fn snapshot_user(&self, user: &UserId) -> Option<UserTraffic> {
		self.stats.get(user).map(|entry| {
			let d = entry.value();
			UserTraffic {
				user_id: user.clone(),
				upload: d.upload_bytes.load(Ordering::Relaxed),
				download: d.download_bytes.load(Ordering::Relaxed),
				request_count: d.request_count.load(Ordering::Relaxed),
			}
		})
	}

	/// Non-resetting read of every user's cumulative counters (e.g. a metrics
	/// endpoint). Does not touch the `resetting` lock.
	pub fn snapshot(&self) -> Vec<UserTraffic> {
		self.stats
			.iter()
			.map(|entry| {
				let d = entry.value();
				UserTraffic {
					user_id: entry.key().clone(),
					upload: d.upload_bytes.load(Ordering::Relaxed),
					download: d.download_bytes.load(Ordering::Relaxed),
					request_count: d.request_count.load(Ordering::Relaxed),
				}
			})
			.collect()
	}

	/// Add a batch back into the collector — used after a failed submit so the
	/// data accumulates into the next cycle (zero loss). Atomic adds, so
	/// concurrent writers are safe.
	pub fn restore(&self, batch: &[UserTraffic]) {
		for t in batch {
			let entry = self.entry(&t.user_id);
			entry.upload_bytes.fetch_add(t.upload, Ordering::Relaxed);
			entry.download_bytes.fetch_add(t.download, Ordering::Relaxed);
			entry.request_count.fetch_add(t.request_count, Ordering::Relaxed);
		}
	}

	/// Atomically drain all stats and return the non-zero deltas.
	///
	/// Single-pass swap-and-collect: swap each counter to 0 and collect the old
	/// value. Writes racing the drain are either counted in this snapshot or
	/// accumulate for the next one — no data loss. Concurrent `reset_all` calls
	/// are serialized; the loser returns an empty vec.
	pub fn reset_all(&self) -> Vec<UserTraffic> {
		if self
			.resetting
			.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
			.is_err()
		{
			return Vec::new();
		}
		let _guard = ResetGuard(&self.resetting);

		let mut batch = Vec::new();
		for entry in self.stats.iter() {
			let d = entry.value();
			let upload = d.upload_bytes.swap(0, Ordering::AcqRel);
			let download = d.download_bytes.swap(0, Ordering::AcqRel);
			let request_count = d.request_count.swap(0, Ordering::AcqRel);
			if upload > 0 || download > 0 || request_count > 0 {
				batch.push(UserTraffic {
					user_id: entry.key().clone(),
					upload,
					download,
					request_count,
				});
			}
		}

		// Single-pass prune: `retain` holds the shard write lock during the
		// check, so a concurrent write cannot race between check and remove.
		self.stats.retain(|_, d| {
			d.upload_bytes.load(Ordering::Relaxed) != 0
				|| d.download_bytes.load(Ordering::Relaxed) != 0
				|| d.request_count.load(Ordering::Relaxed) != 0
		});

		batch
	}

	/// Number of users currently tracked.
	pub fn user_count(&self) -> usize {
		self.stats.len()
	}

	fn entry(&self, user: &UserId) -> dashmap::mapref::one::RefMut<'_, UserId, UserStatsData> {
		// `entry` needs an owned key; clone is a cheap `Arc` bump.
		self.stats.entry(user.clone()).or_default()
	}
}

#[cfg(test)]
mod tests {
	use std::{sync::Arc, thread};

	use super::*;

	fn uid(s: &str) -> UserId {
		UserId::from(s)
	}

	#[test]
	fn record_and_snapshot() {
		let c = StatsCollector::new();
		c.record_upload(&uid("a"), 100);
		c.record_download(&uid("a"), 200);
		c.record_upload(&uid("a"), 50);
		c.record_request(&uid("a"));

		let s = c.snapshot_user(&uid("a")).unwrap();
		assert_eq!(s.upload, 150);
		assert_eq!(s.download, 200);
		assert_eq!(s.request_count, 1);
	}

	#[test]
	fn reset_all_drains_and_prunes() {
		let c = StatsCollector::new();
		c.record_upload(&uid("a"), 100);
		c.record_download(&uid("a"), 200);
		c.record_request(&uid("a"));

		let batch = c.reset_all();
		assert_eq!(batch.len(), 1);
		assert_eq!(batch[0].upload, 100);
		assert_eq!(batch[0].download, 200);
		assert_eq!(batch[0].request_count, 1);
		assert!(c.snapshot_user(&uid("a")).is_none());
	}

	#[test]
	fn restore_round_trips() {
		let c = StatsCollector::new();
		c.record_upload(&uid("a"), 100);
		let batch = c.reset_all();
		c.restore(&batch);
		assert_eq!(c.snapshot_user(&uid("a")).unwrap().upload, 100);
	}

	/// Strict invariant under concurrent writers + drains: collected via resets
	/// plus the final remaining equals total written (no data loss).
	#[test]
	fn reset_all_no_data_loss_strict() {
		for _ in 0..50 {
			let c = Arc::new(StatsCollector::new());
			let per_thread = 2000u64;
			let writers = 5u64;
			let expected = per_thread * writers;

			let mut handles = vec![];
			for _ in 0..writers {
				let c = c.clone();
				handles.push(thread::spawn(move || {
					for _ in 0..per_thread {
						c.record_upload(&uid("a"), 1);
					}
				}));
			}

			let cc = c.clone();
			let drain = thread::spawn(move || {
				let mut collected = 0u64;
				for _ in 0..200 {
					for t in cc.reset_all() {
						collected += t.upload;
					}
					thread::yield_now();
				}
				collected
			});

			for h in handles {
				h.join().unwrap();
			}
			let collected = drain.join().unwrap();
			let remaining: u64 = c.reset_all().iter().map(|t| t.upload).sum();
			assert_eq!(
				collected + remaining,
				expected,
				"data loss: {collected}+{remaining}!={expected}"
			);
		}
	}
}
