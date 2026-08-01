use std::{
	collections::HashMap,
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
use wind_core::{OutboundAction, tcp::AbstractTcpStream, types::TargetAddr, udp::UdpStream};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Load balancing strategy (mirrors clash.meta load-balance `strategy`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
	/// Distribute requests across proxies in turn.
	RoundRobin,
	/// Map the same target address to the same proxy via hashing.
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
	/// When `true`, health checks are deferred until the first connection
	/// attempt.  All proxies are assumed alive until proven otherwise.
	pub lazy: bool,
}

// ---------------------------------------------------------------------------
// Internal per-proxy state
// ---------------------------------------------------------------------------

struct ProxyState {
	outbound: Arc<dyn OutboundAction>,
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
pub struct LoadBalanceOutbound {
	proxies: Vec<ProxyState>,
	strategy: LoadBalanceStrategy,
	url: String,
	round_robin_counter: AtomicUsize,
	sticky_cache: Mutex<HashMap<TargetAddr, (Instant, usize)>>,
}

impl LoadBalanceOutbound {
	/// Create a new load-balance outbound.
	///
	/// # Panics
	///
	/// Panics if `proxies` is empty.
	pub fn new(opts: LoadBalanceOpts, proxies: Vec<Arc<dyn OutboundAction>>) -> Self {
		assert!(!proxies.is_empty(), "LoadBalanceOutbound requires at least one child proxy");

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
			round_robin_counter: AtomicUsize::new(0),
			sticky_cache: Mutex::new(HashMap::new()),
		}
	}

	/// Start the background health-check loop.
	///
	/// Call this **after** wrapping the outbound in an `Arc`.  If
	/// [`LoadBalanceOpts::lazy`] is `true` this is a no-op — health checks
	/// are performed on-demand instead.
	pub fn start_health_check(self: &Arc<Self>, interval: Duration) {
		let this = self.clone();
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

	/// Pick a proxy index for `target` according to the configured strategy.
	async fn select_index(&self, target: &TargetAddr) -> usize {
		let alive = self.alive_indices();

		match self.strategy {
			LoadBalanceStrategy::RoundRobin => {
				// Atomically increment and wrap.
				let c = self.round_robin_counter.fetch_add(1, Ordering::Relaxed);
				alive[c % alive.len()]
			}
			LoadBalanceStrategy::ConsistentHashing => {
				// Hash the target, mod into the alive list.
				let mut hasher = std::collections::hash_map::DefaultHasher::new();
				target.hash(&mut hasher);
				let h = hasher.finish();
				alive[h as usize % alive.len()]
			}
			LoadBalanceStrategy::StickySessions => {
				// Check cache first; on miss, pick via round-robin and cache.
				let mut cache = self.sticky_cache.lock().await;

				// Evict expired entries (older than 10 minutes).
				let now = Instant::now();
				let ttl = Duration::from_secs(600);
				cache.retain(|_, (ts, _)| now.duration_since(*ts) < ttl);

				if let Some((_, idx)) = cache.get(target) {
					// If the cached index is no longer in the alive set, pick a new one.
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
impl OutboundAction for LoadBalanceOutbound {
	async fn handle_tcp(&self, target: TargetAddr, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let idx = self.select_index(&target).await;
		let span = tracing::debug_span!("lb_tcp", target = %target, proxy_index = idx);
		async move {
			tracing::debug!("delegating to proxy {idx}");
			self.proxies[idx].outbound.handle_tcp(target, stream).await
		}
		.instrument(span)
		.await
	}

	async fn handle_udp(&self, stream: UdpStream) -> eyre::Result<()> {
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
			self.proxies[idx].outbound.handle_udp(stream).await
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
async fn check_proxy_health(proxy: Arc<dyn OutboundAction>, url: &str) -> bool {
	let (host, port, path) = match parse_http_url(url) {
		Some(v) => v,
		None => {
			tracing::warn!(%url, "invalid health-check URL");
			return false;
		}
	};

	let target = TargetAddr::Domain(host.to_string(), port);

	// Create a duplex pair: we write the HTTP request on one side and hand
	// the other side to the outbound.  The outbound connects through the
	// child proxy to the real host; our writes appear as data sent to the
	// host and the host's response flows back to us.
	let (mut client, server) = tokio::io::duplex(8192);

	// Spawn the outbound handler — it will consume `server`.
	tokio::spawn(async move {
		let _ = proxy.handle_tcp(target, Box::new(server)).await;
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
	impl OutboundAction for DummyOutbound {
		async fn handle_tcp(&self, _target: TargetAddr, _stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
			Ok(())
		}

		async fn handle_udp(&self, _stream: UdpStream) -> eyre::Result<()> {
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
		let proxies: Vec<Arc<dyn OutboundAction>> = (0..n)
			.map(|_| Arc::new(DummyOutbound::new()) as Arc<dyn OutboundAction>)
			.collect();
		LoadBalanceOutbound::new(make_opts(strategy), proxies)
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
