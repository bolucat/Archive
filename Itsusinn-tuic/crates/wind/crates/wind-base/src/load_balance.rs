use std::{
	collections::{HashMap, hash_map::DefaultHasher},
	hash::{Hash, Hasher},
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicUsize, Ordering},
	},
	time::{Duration, Instant},
};

use async_trait::async_trait;
use tokio::{
	io::{AsyncReadExt, AsyncWriteExt},
	sync::Mutex,
};
use tracing::Instrument;
use wind_core::{
	FlowContext, Outbound, hooks::Protocol, rule::NetworkType, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream,
};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Load balancing strategy (mirrors clash.meta load-balance `strategy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
	/// Distribute requests across proxies in turn.
	RoundRobin,
	/// Map the same target address to the same proxy via a consistent-hash
	/// ring.  Each proxy owns a share of the ring, so dropping a proxy only
	/// remaps the targets that were hashed onto that proxy.
	ConsistentHashing,
	/// Cache target → proxy mappings for 10 minutes.
	StickySessions,
}

/// Options for a load-balance outbound.
#[derive(Clone, Debug)]
pub struct LoadBalanceOpts {
	/// Load balancing strategy.
	pub strategy: LoadBalanceStrategy,
	/// Health-check URL, e.g. `"https://www.gstatic.com/generate_204"`.
	pub url: String,
	/// Interval between successive health-check rounds.
	pub interval: Duration,
	/// When `true`, no periodic health checks are started and every child is
	/// treated as alive.
	pub lazy: bool,
}

// ---------------------------------------------------------------------------
// Internal per-proxy state
// ---------------------------------------------------------------------------

struct ProxyState {
	outbound: Arc<dyn Outbound>,
	alive: AtomicBool,
}

impl ProxyState {
	fn is_alive(&self) -> bool {
		self.alive.load(Ordering::Relaxed)
	}

	fn set_alive(&self, alive: bool) {
		self.alive.store(alive, Ordering::Relaxed);
	}
}

// ---------------------------------------------------------------------------
// LoadBalanceOutbound
// ---------------------------------------------------------------------------

/// Virtual ring nodes per child proxy.  More nodes make the share of the ring
/// owned by each proxy — and therefore the key distribution — more even.
const VNODES_PER_PROXY: u32 = 64;

/// Hash a target address over the full 64-bit hash space.  The value is used
/// as a ring position, never truncated to `usize`.
fn hash_target(target: &TargetAddr) -> u64 {
	let mut hasher = DefaultHasher::new();
	target.hash(&mut hasher);
	hasher.finish()
}

/// Build the consistent-hash ring: each proxy is spread over
/// [`VNODES_PER_PROXY`] virtual nodes, sorted by hash position.
fn build_hash_ring(proxy_count: usize) -> Vec<(u64, usize)> {
	let mut ring = Vec::with_capacity(proxy_count.saturating_mul(VNODES_PER_PROXY as usize));

	for index in 0..proxy_count {
		for vnode in 0..VNODES_PER_PROXY {
			let mut hasher = DefaultHasher::new();
			index.hash(&mut hasher);
			vnode.hash(&mut hasher);
			ring.push((hasher.finish(), index));
		}
	}

	ring.sort_unstable();
	ring
}

/// Outbound that distributes connections across multiple child outbounds
/// according to the configured [`LoadBalanceStrategy`], with optional
/// periodic health checks.
///
/// # Health checking
///
/// When not `lazy`, a background task periodically opens a TCP connection
/// *through* each child outbound to the configured `url`, sends a minimal
/// HTTP GET, and marks the proxy alive or dead.  The main selection logic
/// skips dead proxies; when **all** proxies are dead it falls back to the
/// full set so that a transient network blip doesn't cause a full outage.
///
/// When `lazy`, no health checking happens at all: [`Self::start_health_check`]
/// is a no-op and every child counts as alive.
pub struct LoadBalanceOutbound {
	proxies: Vec<ProxyState>,
	strategy: LoadBalanceStrategy,
	url: String,
	interval: Duration,
	lazy: bool,
	/// Sorted `(ring position, proxy index)` pairs used by
	/// [`LoadBalanceStrategy::ConsistentHashing`].
	ring: Vec<(u64, usize)>,
	round_robin_counter: AtomicUsize,
	sticky_cache: Mutex<HashMap<TargetAddr, (Instant, usize)>>,
}

impl LoadBalanceOutbound {
	/// Create a new load-balance outbound.
	///
	/// # Panics
	///
	/// Panics if `proxies` is empty.
	pub fn new(opts: LoadBalanceOpts, proxies: Vec<Arc<dyn Outbound>>) -> Self {
		assert!(!proxies.is_empty(), "LoadBalanceOutbound requires at least one child proxy");

		let ring = build_hash_ring(proxies.len());

		Self {
			proxies: proxies
				.into_iter()
				.map(|outbound| ProxyState {
					outbound,
					alive: AtomicBool::new(true),
				})
				.collect(),
			strategy: opts.strategy,
			url: opts.url,
			interval: opts.interval,
			lazy: opts.lazy,
			ring,
			round_robin_counter: AtomicUsize::new(0),
			sticky_cache: Mutex::new(HashMap::new()),
		}
	}

	/// Start the background health-check loop.
	///
	/// Call this **after** wrapping the outbound in an `Arc`.  No-op when
	/// [`LoadBalanceOpts::lazy`] is `true`; otherwise the loop runs at the
	/// configured [`LoadBalanceOpts::interval`].
	pub fn start_health_check(self: &Arc<Self>) {
		if self.lazy {
			tracing::debug!("lazy load-balance outbound: health checks stay disabled");
			return;
		}

		let this = self.clone();
		let interval = self.interval;
		tokio::spawn(async move {
			health_check_loop(this, interval).await;
		});
	}

	// ---- proxy selection -------------------------------------------------

	/// Return the list of indices that are currently considered alive.
	/// Falls back to all indices when every proxy is dead.
	fn alive_indices(&self) -> Vec<usize> {
		let alive: Vec<usize> = self
			.proxies
			.iter()
			.enumerate()
			.filter(|(_, p)| p.is_alive())
			.map(|(i, _)| i)
			.collect();

		if alive.is_empty() {
			tracing::warn!("all load-balance proxies are dead; falling back to full set");
			(0..self.proxies.len()).collect()
		} else {
			alive
		}
	}

	/// Walk the consistent-hash ring clockwise from `hash` and return the first
	/// entry whose proxy is alive.  Targets that do not hash onto a dead proxy
	/// therefore keep their proxy; only the dead proxy's own share of the ring
	/// is handed to its successors.  When every proxy is dead the ring entry at
	/// `hash` is returned, matching the full-set fallback of
	/// [`Self::alive_indices`].
	fn select_consistent_hash(&self, hash: u64) -> usize {
		let start = self.ring.partition_point(|(position, _)| *position < hash);

		for offset in 0..self.ring.len() {
			let (_, index) = self.ring[(start + offset) % self.ring.len()];
			if self.proxies[index].is_alive() {
				return index;
			}
		}

		tracing::warn!("all load-balance proxies are dead; falling back to full set");
		self.ring[start % self.ring.len()].1
	}

	/// Pick a proxy index for `target` according to the configured strategy.
	async fn select_index(&self, target: &TargetAddr) -> usize {
		match self.strategy {
			LoadBalanceStrategy::RoundRobin => {
				let alive = self.alive_indices();
				// Atomically increment and wrap.
				let c = self.round_robin_counter.fetch_add(1, Ordering::Relaxed);
				alive[c % alive.len()]
			}
			LoadBalanceStrategy::ConsistentHashing => self.select_consistent_hash(hash_target(target)),
			LoadBalanceStrategy::StickySessions => {
				let alive = self.alive_indices();

				// Check cache first; on miss, pick via round-robin and cache.
				let mut cache = self.sticky_cache.lock().await;

				// Evict expired entries (older than 10 minutes).
				let now = Instant::now();
				let ttl = Duration::from_secs(600);
				cache.retain(|_, (ts, _)| now.duration_since(*ts) < ttl);

				if let Some((_, idx)) = cache.get(target) {
					// If the cached index is no longer in the alive set, pick a
					// new one.
					if alive.contains(idx) {
						return *idx;
					}
				}

				let c = self.round_robin_counter.fetch_add(1, Ordering::Relaxed);
				let idx = alive[c % alive.len()];
				cache.insert(target.clone(), (now, idx));
				idx
			}
		}
	}
}

#[async_trait]
impl Outbound for LoadBalanceOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let idx = self.select_index(&ctx.target).await;
		let span = tracing::debug_span!("lb_tcp", target = %ctx.target, proxy_index = idx);
		async move {
			tracing::debug!("delegating to proxy {idx}");
			self.proxies[idx].outbound.handle_tcp(ctx, stream).await
		}
		.instrument(span)
		.await
	}

	async fn handle_udp(&self, ctx: FlowContext, stream: UdpStream) -> eyre::Result<()> {
		// UDP sessions are routed once (by the dispatcher) and stick to one
		// handler.  We sample the first packet's target for the selection so
		// the session is pinned to a single child outbound — matching clash
		// semantics where the proxy-group decision happens once per session.
		//
		// If the stream has no packets yet we can't select; defer to the
		// first available proxy.  In practice the dispatcher always replays
		// at least one packet before calling handle_udp.
		let idx = {
			// Try to peek the first packet's target without consuming it.
			// We can't actually peek mpsc, so use a simple fallback: for UDP
			// we use round-robin regardless of strategy, since the target
			// for subsequent packets may differ anyway.
			//
			// This is a design trade-off — the dispatcher already made the
			// routing decision based on the first packet's target, but we
			// don't have access to it here.  Round-robin is a reasonable
			// default for UDP.
			let alive = self.alive_indices();
			let c = self.round_robin_counter.fetch_add(1, Ordering::Relaxed);
			alive[c % alive.len()]
		};

		let span = tracing::debug_span!("lb_udp", proxy_index = idx);
		async move {
			tracing::debug!("delegating UDP to proxy {idx}");
			self.proxies[idx].outbound.handle_udp(ctx, stream).await
		}
		.instrument(span)
		.await
	}
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

/// Background loop that probes every child proxy at `interval`.
async fn health_check_loop(lb: Arc<LoadBalanceOutbound>, interval: Duration) {
	// Stagger the first probe so we don't hammer all proxies simultaneously.
	let stagger = interval.max(Duration::from_secs(10)) / lb.proxies.len().max(1) as u32;

	loop {
		for (i, proxy) in lb.proxies.iter().enumerate() {
			let alive = check_proxy_health(proxy.outbound.clone(), &lb.url).await;
			let prev = proxy.is_alive();
			proxy.set_alive(alive);

			match (prev, alive) {
				(false, true) => tracing::info!(proxy_index = i, "proxy recovered"),
				(true, false) => tracing::warn!(proxy_index = i, "proxy marked dead"),
				_ => {}
			}

			tokio::time::sleep(stagger).await;
		}
		tokio::time::sleep(interval).await;
	}
}

/// Check whether `proxy` can reach the URL host by tunnelling an HTTP GET
/// through it.  Returns `true` if we receive any HTTP response bytes within
/// the timeout.
async fn check_proxy_health(proxy: Arc<dyn Outbound>, url: &str) -> bool {
	let (host, port, path) = match parse_http_url(url) {
		Some(v) => v,
		None => {
			tracing::warn!(%url, "invalid health-check URL");
			return false;
		}
	};

	let target = TargetAddr::Domain(host.to_string(), port);
	let health_ctx = FlowContext {
		target,
		network: NetworkType::Tcp,
		source: None,
		inbound_tag: "health-check".into(),
		protocol: Protocol::Tunnel,
		user: None,
		inbound_port: None,
		inbound_type: None,
	};

	// Create a duplex pair: we write the HTTP request on one side and hand
	// the other side to the outbound.  The outbound connects through the
	// child proxy to the real host; our writes appear as data sent to the
	// host and the host's response flows back to us.
	let (mut client, server) = tokio::io::duplex(8192);

	// Spawn the outbound handler — it will consume `server`.
	tokio::spawn(async move {
		let _ = proxy.handle_tcp(health_ctx, Box::new(server)).await;
	});

	// Send a minimal HTTP/1.1 GET.
	let request = format!("GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", path, host);
	if client.write_all(request.as_bytes()).await.is_err() {
		return false;
	}

	// Read at least *something* back — any response means the proxy + target
	// path is functional.
	let mut buf = [0u8; 256];
	match tokio::time::timeout(Duration::from_secs(5), client.read(&mut buf)).await {
		Ok(Ok(n)) if n > 0 => true,
		Ok(Ok(_)) => {
			tracing::debug!("health check: empty response from {host}");
			false
		}
		Ok(Err(e)) => {
			tracing::debug!(error = %e, "health check: read error");
			false
		}
		Err(_timeout) => {
			tracing::debug!("health check: timeout connecting to {host}");
			false
		}
	}
}

/// Parse `http://host[:port][/path]` or `https://host[:port][/path]` into
/// (host, port, path).
fn parse_http_url(url: &str) -> Option<(&str, u16, &str)> {
	let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
	let default_port = if url.starts_with("https://") { 443 } else { 80 };

	let slash_pos = rest.find('/');
	let host_port = match slash_pos {
		Some(pos) => &rest[..pos],
		None => rest,
	};
	let path = match slash_pos {
		Some(pos) => &rest[pos..],
		None => "/",
	};

	let (host, port) = match host_port.rsplit_once(':') {
		Some((h, p)) => (h, p.parse::<u16>().ok()?),
		None => (host_port, default_port),
	};

	Some((host, port, path))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use std::sync::atomic::AtomicBool;

	use super::*;

	struct DummyOutbound {
		_called: AtomicBool,
	}

	impl DummyOutbound {
		fn new() -> Self {
			Self {
				_called: AtomicBool::new(false),
			}
		}
	}

	#[async_trait]
	impl Outbound for DummyOutbound {
		async fn handle_tcp(&self, _ctx: FlowContext, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
			Ok(())
		}

		async fn handle_udp(&self, _ctx: FlowContext, _stream: UdpStream) -> eyre::Result<()> {
			Ok(())
		}
	}

	fn make_opts(strategy: LoadBalanceStrategy) -> LoadBalanceOpts {
		LoadBalanceOpts {
			strategy,
			url: "https://example.com/health".into(),
			interval: Duration::from_secs(30),
			lazy: true,
		}
	}

	fn make_lb(strategy: LoadBalanceStrategy, n: usize) -> LoadBalanceOutbound {
		let proxies: Vec<Arc<dyn Outbound>> = (0..n).map(|_| Arc::new(DummyOutbound::new()) as Arc<dyn Outbound>).collect();
		LoadBalanceOutbound::new(make_opts(strategy), proxies)
	}

	fn make_lb_arc(opts: LoadBalanceOpts, n: usize) -> Arc<LoadBalanceOutbound> {
		let proxies: Vec<Arc<dyn Outbound>> = (0..n).map(|_| Arc::new(DummyOutbound::new()) as Arc<dyn Outbound>).collect();
		Arc::new(LoadBalanceOutbound::new(opts, proxies))
	}

	// ---- health-check scheduling -----------------------------------------

	/// `lazy: true` means no periodic probing happens, even if the health
	/// check is requested explicitly — the outbound itself owns that decision.
	#[tokio::test]
	async fn lazy_outbound_never_probes_its_children() {
		let opts = LoadBalanceOpts {
			lazy: true,
			interval: Duration::from_millis(10),
			..make_opts(LoadBalanceStrategy::RoundRobin)
		};
		let lb = make_lb_arc(opts, 2);

		lb.start_health_check();

		// The first probe of a started loop runs immediately, so 300 ms is
		// far more than enough to observe one.
		tokio::time::sleep(Duration::from_millis(300)).await;
		assert!(
			lb.proxies.iter().all(|p| p.is_alive()),
			"a lazy load-balancer must not probe its children"
		);
	}

	/// A non-lazy outbound probes its children (the dummy children never
	/// answer, so they must end up marked dead).
	#[tokio::test]
	async fn non_lazy_outbound_probes_its_children() {
		let opts = LoadBalanceOpts {
			lazy: false,
			interval: Duration::from_millis(10),
			..make_opts(LoadBalanceStrategy::RoundRobin)
		};
		let lb = make_lb_arc(opts, 1);

		lb.start_health_check();

		let deadline = Instant::now() + Duration::from_secs(5);
		while lb.proxies[0].is_alive() && Instant::now() < deadline {
			tokio::time::sleep(Duration::from_millis(10)).await;
		}
		assert!(!lb.proxies[0].is_alive(), "a non-lazy load-balancer must probe its children");
	}

	// ---- round-robin -----------------------------------------------------

	#[tokio::test]
	async fn round_robin_cycles_through_proxies() {
		let lb = make_lb(LoadBalanceStrategy::RoundRobin, 3);
		let target = TargetAddr::Domain("example.com".into(), 443);

		let a = lb.select_index(&target).await;
		let b = lb.select_index(&target).await;
		let c = lb.select_index(&target).await;
		let d = lb.select_index(&target).await;

		// With 3 proxies we expect indices 0,1,2,0,... (order guaranteed by
		// AtomicUsize fetch_add).
		assert_eq!(a, 0);
		assert_eq!(b, 1);
		assert_eq!(c, 2);
		assert_eq!(d, 0);
	}

	// ---- consistent-hashing ----------------------------------------------

	#[tokio::test]
	async fn consistent_hashing_same_target_same_proxy() {
		let lb = make_lb(LoadBalanceStrategy::ConsistentHashing, 5);
		let target = TargetAddr::Domain("test.example.com".into(), 443);

		let first = lb.select_index(&target).await;
		for _ in 0..20 {
			assert_eq!(lb.select_index(&target).await, first);
		}
	}

	#[tokio::test]
	async fn consistent_hashing_different_targets_may_differ() {
		let lb = make_lb(LoadBalanceStrategy::ConsistentHashing, 10);
		let t1 = TargetAddr::Domain("a.example.com".into(), 80);
		let t2 = TargetAddr::Domain("b.example.com".into(), 443);

		let i1 = lb.select_index(&t1).await;
		let i2 = lb.select_index(&t2).await;
		// They may or may not collide — both are valid.  We just assert the
		// deterministic property: each target always maps to the same index.
		for _ in 0..10 {
			assert_eq!(lb.select_index(&t1).await, i1);
			assert_eq!(lb.select_index(&t2).await, i2);
		}
	}

	/// A consistent-hash ring must only remap the keys that were owned by the
	/// proxy that went away; every other key keeps its proxy.  A plain
	/// `hash % alive.len()` re-shuffles almost everything instead.
	#[tokio::test]
	async fn consistent_hashing_only_remaps_keys_of_a_dead_proxy() {
		const PROXIES: usize = 8;
		const DEAD: usize = 5;
		const KEYS: usize = 2000;

		let lb = make_lb(LoadBalanceStrategy::ConsistentHashing, PROXIES);
		let targets: Vec<TargetAddr> = (0..KEYS)
			.map(|i| TargetAddr::Domain(format!("host{i}.example.com"), 443))
			.collect();

		let mut before = Vec::with_capacity(KEYS);
		for target in &targets {
			before.push(lb.select_index(target).await);
		}

		lb.proxies[DEAD].set_alive(false);

		let mut remapped = 0usize;
		for (target, old) in targets.iter().zip(&before) {
			let new = lb.select_index(target).await;
			if new != *old {
				assert_eq!(*old, DEAD, "consistent hashing must only remap keys owned by the dead proxy");
				remapped += 1;
			}
		}

		// Roughly `1 / PROXIES` of the keys belonged to the dead proxy.
		assert!(remapped > 0, "the dead proxy must have owned some keys");
		assert!(
			remapped * 4 < KEYS,
			"a single dead proxy must not remap more than a quarter of the keys, remapped {remapped}/{KEYS}"
		);
	}

	/// The ring itself must span the whole 64-bit hash space: a `u64 as usize`
	/// truncation would collapse it into the low 32 bits on 32-bit targets and
	/// silently halve the effective key space.
	#[test]
	fn consistent_hash_ring_covers_the_full_u64_space() {
		const PROXIES: usize = 4;
		let ring = build_hash_ring(PROXIES);

		assert_eq!(ring.len(), PROXIES * VNODES_PER_PROXY as usize);
		assert!(
			ring.windows(2).all(|w| w[0].0 <= w[1].0),
			"the ring must be sorted by position"
		);
		assert!(
			ring.iter().all(|(_, index)| *index < PROXIES),
			"every ring entry must belong to a proxy"
		);

		let min = ring.first().expect("the ring is non-empty").0;
		let max = ring.last().expect("the ring is non-empty").0;
		// A 32-bit truncation would confine every position to `0..2^32`, so the
		// span is the observable symptom of using the full `u64` hash.
		assert!(
			max - min > u64::from(u32::MAX),
			"the ring must span more than the low 32 bits, got span {}",
			max - min
		);
		// The upper half of the hash space must be reachable at all.
		let high = ring.iter().filter(|(position, _)| *position > u64::from(u32::MAX)).count();
		assert!(
			high * 4 > ring.len(),
			"at least a quarter of the ring must sit above 2^32, got {high}/{}",
			ring.len()
		);
	}

	/// With every proxy dead the ring walk must still return a usable index
	/// instead of panicking.
	#[tokio::test]
	async fn consistent_hashing_survives_all_proxies_going_dead() {
		let lb = make_lb(LoadBalanceStrategy::ConsistentHashing, 3);
		for proxy in &lb.proxies {
			proxy.set_alive(false);
		}

		let idx = lb.select_index(&TargetAddr::Domain("x.com".into(), 80)).await;
		assert!(idx < 3, "expected index in 0..3, got {idx}");
	}

	// ---- sticky-sessions -------------------------------------------------

	#[tokio::test]
	async fn sticky_sessions_caches_target() {
		let lb = make_lb(LoadBalanceStrategy::StickySessions, 5);
		let target = TargetAddr::Domain("sticky.example.com".into(), 443);

		let first = lb.select_index(&target).await;
		for _ in 0..10 {
			assert_eq!(lb.select_index(&target).await, first);
		}
	}

	#[tokio::test]
	async fn sticky_sessions_different_targets_independent() {
		let lb = make_lb(LoadBalanceStrategy::StickySessions, 10);
		let t1 = TargetAddr::Domain("a.com".into(), 80);
		let t2 = TargetAddr::Domain("b.com".into(), 80);

		let _i1 = lb.select_index(&t1).await;
		let _i2 = lb.select_index(&t2).await;
		// Both should be cached independently — just check they don't panic.
	}

	// ---- dead-proxy fallback ---------------------------------------------

	#[tokio::test]
	async fn all_dead_falls_back_to_full_set() {
		let lb = make_lb(LoadBalanceStrategy::RoundRobin, 3);
		for p in &lb.proxies {
			p.set_alive(false);
		}

		// Should not panic and should pick from the full 0..3 range.
		let idx = lb.select_index(&TargetAddr::Domain("x.com".into(), 80)).await;
		assert!(idx < 3, "expected index in 0..3, got {idx}");
	}

	// ---- URL parsing -----------------------------------------------------

	#[test]
	fn parse_https_url_with_path() {
		let (host, port, path) = parse_http_url("https://www.gstatic.com/generate_204").unwrap();
		assert_eq!(host, "www.gstatic.com");
		assert_eq!(port, 443);
		assert_eq!(path, "/generate_204");
	}

	#[test]
	fn parse_http_url_default_port() {
		let (host, port, path) = parse_http_url("http://example.com/").unwrap();
		assert_eq!(host, "example.com");
		assert_eq!(port, 80);
		assert_eq!(path, "/");
	}

	#[test]
	fn parse_url_with_custom_port() {
		let (host, port, path) = parse_http_url("https://example.com:8443/health").unwrap();
		assert_eq!(host, "example.com");
		assert_eq!(port, 8443);
		assert_eq!(path, "/health");
	}

	#[test]
	fn parse_url_no_path() {
		let (host, port, path) = parse_http_url("https://example.com").unwrap();
		assert_eq!(host, "example.com");
		assert_eq!(port, 443);
		assert_eq!(path, "/");
	}

	#[test]
	fn parse_url_invalid_scheme() {
		assert!(parse_http_url("ftp://example.com").is_none());
	}

	#[test]
	fn parse_url_empty_fails() {
		assert!(parse_http_url("").is_none());
	}

	// ---- edge cases ------------------------------------------------------

	#[test]
	#[should_panic(expected = "at least one")]
	fn empty_proxies_panics() {
		LoadBalanceOutbound::new(make_opts(LoadBalanceStrategy::RoundRobin), vec![]);
	}

	#[tokio::test]
	async fn single_proxy_always_chosen() {
		let lb = make_lb(LoadBalanceStrategy::RoundRobin, 1);
		assert_eq!(lb.select_index(&TargetAddr::Domain("x.com".into(), 80)).await, 0);
		assert_eq!(lb.select_index(&TargetAddr::Domain("y.com".into(), 80)).await, 0);
	}
}
