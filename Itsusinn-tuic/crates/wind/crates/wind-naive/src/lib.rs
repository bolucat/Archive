//! Naive outbound using `cronet-rs` (Cronet-based CONNECT proxy).
//!
//! This crate provides [`NaiveOutbound`], an implementation of
//! [`wind_core::Outbound`] that tunnels TCP (and UDP-over-TCP) through
//! a NaiveProxy server via the Cronet HTTP/2 or QUIC CONNECT protocol with
//! padding.

use std::{
	io::{Read, Write},
	sync::{
		Arc,
		atomic::{AtomicUsize, Ordering},
	},
	thread::JoinHandle,
};

use async_trait::async_trait;
use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig, QuicCongestionControl as CronetCongestionControl};
use eyre::Context as _;
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
	sync::mpsc,
};
use tracing::{Instrument, info};
use wind_core::{
	FlowContext, Outbound, QuicCongestionControl,
	tcp::AbstractTcpStream,
	udp::{UdpPacket, UdpStream},
};

mod uot;

/// Map the transport-agnostic [`QuicCongestionControl`] onto the `cronet-rs`
/// representation expected by the underlying engine.
fn to_cronet(cc: QuicCongestionControl) -> CronetCongestionControl {
	match cc {
		QuicCongestionControl::Default => CronetCongestionControl::Default,
		QuicCongestionControl::Bbr => CronetCongestionControl::Bbr,
		QuicCongestionControl::BbrV2 => CronetCongestionControl::BbrV2,
		QuicCongestionControl::Cubic => CronetCongestionControl::Cubic,
		QuicCongestionControl::Reno => CronetCongestionControl::Reno,
	}
}

/// Configuration for the Naive outbound.
#[derive(Clone, Debug)]
pub struct NaiveOutboundOpts {
	/// NaiveProxy server address (host:port).
	pub server_address: String,

	/// Server name (SNI), defaults to server_address's host.
	pub server_name: Option<String>,

	/// Username for proxy authentication.
	pub username: Option<String>,

	/// Password for proxy authentication.
	pub password: Option<String>,

	/// Number of concurrent connections to the Cronet engine.
	/// 1 = single connection (default).
	pub concurrency: u32,

	/// Enable QUIC.
	pub quic_enabled: bool,

	/// QUIC congestion control algorithm.
	pub quic_congestion_control: QuicCongestionControl,

	/// PEM-encoded trusted root certificates.
	/// If `None`, the platform trust store is used.
	pub trusted_root_certificates: Option<String>,

	/// Enable ECH (Encrypted Client Hello).
	pub ech_enabled: bool,

	/// Extra headers to include in every CONNECT request.
	pub extra_headers: std::collections::HashMap<String, String>,

	/// Path to `libcronet.so` (or `.dylib`) shared library.
	///
	/// If `None`, the loader tries these locations in order:
	///   1. `LD_LIBRARY_PATH` / system default (`libcronet.so`)
	///   2. `./libcronet.so`
	///   3. `/usr/local/lib/libcronet.so`
	///   4. `/opt/cronet/libcronet.so`
	pub cronet_lib_path: Option<String>,
}

impl Default for NaiveOutboundOpts {
	fn default() -> Self {
		Self {
			server_address: String::new(),
			server_name: None,
			username: None,
			password: None,
			concurrency: 1,
			quic_enabled: false,
			quic_congestion_control: QuicCongestionControl::Default,
			trusted_root_certificates: None,
			ech_enabled: false,
			extra_headers: std::collections::HashMap::new(),
			cronet_lib_path: None,
		}
	}
}

/// An outbound that tunnels traffic through a NaiveProxy server via Cronet.
///
/// Uses `cronet-rs` (Chromium Cronet C API bindings) to establish HTTP/2
/// or QUIC CONNECT tunnels with NaiveProxy padding protocol.
///
/// The Cronet engine is shared across all connections (`RwLock` read-locked
/// only during dial), so concurrent tunnels are not serialized.
pub struct NaiveOutbound {
	/// Shared Cronet engine behind a read-write lock.
	///
	/// `dial()` takes `&self`, so concurrent dials acquire a **read** lock and
	/// run in parallel.  The write lock is only held during `start()`.
	client: Arc<tokio::sync::RwLock<NaiveClient>>,
}

/// Extract the host from a `host:port` authority, handling bracketed IPv6.
///
/// `[2001:db8::1]:443` -> `2001:db8::1`, `example.com:443` -> `example.com`,
/// `1.2.3.4:443` -> `1.2.3.4`, `example.com` -> `example.com`. A naive
/// `split(':').next()` would return `[2001` for the IPv6 case, producing a
/// bogus SNI.
fn host_from_authority(authority: &str) -> &str {
	if let Some(rest) = authority.strip_prefix('[') {
		// Bracketed IPv6: the host is everything up to the closing bracket.
		if let Some(end) = rest.find(']') {
			return &rest[..end];
		}
	}
	// Otherwise strip a trailing `:port` (the last colon); no colon means the
	// whole string is the host.
	authority.rsplit_once(':').map_or(authority, |(host, _)| host)
}

impl NaiveOutbound {
	/// Create and start a new `NaiveOutbound`.
	///
	/// This spawns the Cronet engine (a blocking FFI operation) in a
	/// background blocking thread so the async caller is not stalled.
	///
	/// `cronet_lib_path` controls how `libcronet` is loaded — see
	/// [`NaiveOutboundOpts::cronet_lib_path`].
	pub async fn new(opts: NaiveOutboundOpts) -> eyre::Result<Self> {
		// Load libcronet before touching any FFI.
		load_cronet(opts.cronet_lib_path.clone())?;

		let server_name = opts
			.server_name
			.clone()
			.unwrap_or_else(|| host_from_authority(&opts.server_address).to_string());

		let config = NaiveClientConfig {
			server_address: opts.server_address.clone(),
			server_name: Some(server_name),
			username: opts.username.clone(),
			password: opts.password.clone(),
			concurrency: opts.concurrency,
			extra_headers: opts.extra_headers.clone(),
			trusted_root_certificates: opts.trusted_root_certificates.clone(),
			quic_enabled: opts.quic_enabled,
			quic_congestion_control: to_cronet(opts.quic_congestion_control),
			ech_enabled: opts.ech_enabled,
			..Default::default()
		};

		let client = NaiveClient::new(config).map_err(|e| eyre::eyre!("Failed to create NaiveClient: {e}"))?;

		let client = Arc::new(tokio::sync::RwLock::new(client));

		// Start the Cronet engine (requires &mut → write lock).
		let client_for_start = client.clone();
		tokio::task::spawn_blocking(move || -> eyre::Result<()> {
			let mut guard = client_for_start.blocking_write();
			guard.start().map_err(|e| eyre::eyre!("Failed to start Cronet engine: {e}"))
		})
		.await
		.context("Spawn blocking for engine start failed")??;

		info!(target: "naive", "NaiveOutbound started, server={}", opts.server_address);

		Ok(Self { client })
	}
}

#[async_trait]
impl Outbound for NaiveOutbound {
	async fn handle_tcp(&self, ctx: FlowContext, stream: Box<dyn AbstractTcpStream + 'static>) -> eyre::Result<()> {
		let target_str = ctx.target.to_string();
		let client = self.client.clone();

		info!(target: "naive_tcp", "connecting to {target_str}");

		// Dial a CONNECT tunnel via the Cronet engine.
		//
		// `dial_and_handshake` takes `&self` so we only need a **read** lock;
		// concurrent calls are NOT serialized.
		let target_for_dial = target_str.clone();
		let naive_conn = tokio::task::spawn_blocking(move || -> eyre::Result<_> {
			let guard = client.blocking_read();
			guard
				.dial_and_handshake(&target_for_dial)
				.map_err(|e| eyre::eyre!("CONNECT to {target_for_dial} failed: {e}"))
		})
		.await
		.context("Spawn blocking for dial failed")??;

		info!(target: "naive_tcp", "CONNECT tunnel established to {target_str}");

		// Bridge the blocking NaiveConn to the async stream.
		naive_async_bridge(naive_conn, stream).await
	}

	/// Relay UDP through a **single** Naive CONNECT tunnel using UoT v2
	/// framing.
	///
	/// NaiveProxy's classic protocol only tunnels TCP, so all datagrams for
	/// this [`UdpStream`] are multiplexed over one CONNECT tunnel opened to
	/// the UoT magic authority ([`uot::MAGIC_ADDRESS`]); each packet carries
	/// its own destination/source address. This requires a UoT-v2-aware server
	/// (e.g. sing-box's naive inbound) on the other end.
	///
	/// This replaces the previous per-datagram fire-and-forget design, which
	/// paid a full TLS+CONNECT handshake per packet, could fan out unbounded
	/// tasks, and silently black-holed every reply.
	async fn handle_udp(&self, _ctx: FlowContext, udp_stream: UdpStream) -> eyre::Result<()> {
		let UdpStream { tx, mut rx } = udp_stream;

		// Defer opening the tunnel until the first datagram so idle UDP
		// associations cost nothing, and so the UoT request header can name a
		// concrete destination.
		let Some(first) = rx.recv().await else {
			return Ok(());
		};

		let client = self.client.clone();
		// UoT signals via the CONNECT authority; the port is informational
		// (the server matches on the magic host), so any value works.
		let magic = format!("{}:443", uot::MAGIC_ADDRESS);
		let naive_conn = tokio::task::spawn_blocking(move || -> eyre::Result<_> {
			let guard = client.blocking_read();
			guard
				.dial_and_handshake(&magic)
				.map_err(|e| eyre::eyre!("UoT CONNECT tunnel failed: {e}"))
		})
		.await
		.context("Spawn blocking for UoT dial failed")??;

		info!(target: "naive_udp", "UoT v{} tunnel established", uot::VERSION);

		naive_uot_bridge(naive_conn, first, rx, tx).await
	}
}

/// Number of bridge I/O threads that have been fully joined.
///
/// A parked reader blocks for a while after the relay stops (the Cronet
/// read timeout in `cronet-rs` bounds it), so this only ever *grows* long
/// after [`reap_blocking_thread`] was queued. It is exposed so a regression
/// test can prove the thread handle is joined rather than detached: with a
/// detached handle the count can never move.
static REAPED_IO_THREADS: AtomicUsize = AtomicUsize::new(0);

/// How many bridge I/O threads have finished and been joined.
#[cfg(test)]
fn reaped_io_threads() -> usize {
	REAPED_IO_THREADS.load(Ordering::Acquire)
}

/// Name of the bridge I/O thread, as reported by `std::thread::Builder`.
const BRIDGE_IO_THREAD_NAME: &str = "wind-naive-io";

/// Name of the UoT bridge I/O thread.
const UOT_IO_THREAD_NAME: &str = "wind-naive-uot-io";

/// Name of the plain thread that joins an I/O thread outside a tokio runtime.
const REAPER_THREAD_NAME: &str = "wind-naive-reaper";

/// Largest chunk a naive bridge relays in one read.
///
/// It is also the size of the **single** read buffer each relay owns (see
/// [`relay_local_and_naive`] and the TCP bridge's I/O thread) — never one
/// buffer per chunk.
const NAIVE_BRIDGE_CHUNK: usize = 65535;

/// Spawn a bridge I/O thread with an injectable strategy.
///
/// The bridges run in `Result`-returning functions, so a refused thread —
/// `EAGAIN` when the OS thread limit, the process limit, or the address space
/// for the stack is exhausted — must surface as an error instead of unwinding
/// the task. [`spawn_io_thread`] passes `std::thread::Builder::spawn`; the
/// strategy is a parameter only so a test can drive the refused-thread path
/// deterministically, which the OS cannot be asked to do.
fn spawn_io_thread_with<'a>(
	name: &str,
	spawn: impl FnOnce(&str, Box<dyn FnOnce() + Send + 'a>) -> std::io::Result<JoinHandle<()>> + 'a,
	body: impl FnOnce() + Send + 'a,
) -> std::io::Result<JoinHandle<()>> {
	spawn(name, Box::new(body))
}

/// Spawn one of the bridge I/O threads, reporting a refused thread as an error.
fn spawn_io_thread(name: &str, body: impl FnOnce() + Send + 'static) -> std::io::Result<JoinHandle<()>> {
	spawn_io_thread_with(
		name,
		|name, body| std::thread::Builder::new().name(name.to_string()).spawn(body),
		body,
	)
}

/// Join `handle` on a thread that is allowed to block, absorbing a refused
/// reaper thread instead of propagating it.
fn reap_blocking_thread_with(
	handle: JoinHandle<()>,
	spawn: impl FnOnce(&str, Box<dyn FnOnce() + Send + 'static>) -> std::io::Result<JoinHandle<()>>,
) {
	if let Ok(runtime) = tokio::runtime::Handle::try_current() {
		// Detached on purpose: this task only *waits* for `handle`, and it is
		// the runtime's blocking pool — not a `join()` on an async worker —
		// that pays for that wait.
		#[expect(
			clippy::let_underscore_future,
			reason = "the reaping task is intentionally detached; its only job is to join `handle`"
		)]
		let _ = runtime.spawn_blocking(move || {
			let _ = handle.join();
			REAPED_IO_THREADS.fetch_add(1, Ordering::AcqRel);
		});
	} else {
		// No reactor to hand the wait to; a plain thread keeps the reaping
		// behaviour identical for callers outside a tokio runtime.
		if let Err(e) = spawn(
			REAPER_THREAD_NAME,
			Box::new(move || {
				let _ = handle.join();
				REAPED_IO_THREADS.fetch_add(1, Ordering::AcqRel);
			}),
		) {
			// Reaping is best-effort: the relay it belongs to is already over,
			// and panicking here (from `Drop`, possibly during an unwind) would
			// abort the process instead of merely detaching the thread.
			tracing::warn!(error = %e, "could not spawn a thread to reap a wind-naive io thread");
		}
	}
}

/// Join a detached-able blocking thread from a thread that is allowed to block.
///
/// `Drop` runs on an async worker, so it cannot `join` inline: the Cronet
/// reader may sit in a blocking FFI read for up to its own timeout, and
/// parking a tokio worker for that long would leak the runtime's capacity.
/// The reaper lives on a blocking thread (or a plain one outside a runtime),
/// so the standard thread and the `NaiveConn` it owns — including its socket —
/// are reclaimed instead of being abandoned by a dropped [`JoinHandle`].
fn reap_blocking_thread(handle: JoinHandle<()>) {
	reap_blocking_thread_with(handle, |name, body| {
		std::thread::Builder::new().name(name.to_string()).spawn(body)
	});
}

/// Owns a bridge's blocking I/O thread for as long as the relay is running.
///
/// Dropping the previous `JoinHandle` *detached* the thread, which left the
/// thread and the socket inside its `NaiveConn` alive until the parked Cronet
/// read eventually timed out — and, because nothing ever joined, the kernel
/// thread itself was never reclaimed. Holding the handle here and reaping it
/// on drop keeps the "the relay is over ⇒ the I/O thread is reclaimed"
/// invariant explicit.
struct JoinSession {
	handle: Option<JoinHandle<()>>,
}

impl JoinSession {
	fn new(handle: JoinHandle<()>) -> Self {
		Self { handle: Some(handle) }
	}
}

impl Drop for JoinSession {
	fn drop(&mut self) {
		if let Some(handle) = self.handle.take() {
			reap_blocking_thread(handle);
		}
	}
}

/// Bridge a [`UdpStream`] across a blocking
/// [`cronet_rs::naive_conn::NaiveConn`] using UoT v2 framing.
///
/// A dedicated **std::thread** owns the `NaiveConn` (the Cronet C API requires
/// stream operations from a single thread) and relays framed bytes through
/// bounded `mpsc` channels:
///
/// * uplink — async `rx` → frame → channel → thread `write_all`s the bytes
/// * downlink — thread reads/parses frames → channel → async sends to `tx`
///
/// Like the TCP bridge, the thread interleaves "drain pending writes, then one
/// blocking framed read" on a single handle. While it is blocked awaiting a
/// downlink frame, freshly queued uplink packets wait for the next read to
/// return — an inherent limitation of the blocking, non-splittable handle.
///
/// When the relay ends the thread is handed to a [`JoinSession`], which joins
/// it off the async worker, so neither the thread nor the `NaiveConn`'s socket
/// outlives the relay.
async fn naive_uot_bridge(
	mut naive: cronet_rs::naive_conn::NaiveConn,
	first: UdpPacket,
	mut rx: mpsc::Receiver<UdpPacket>,
	tx: mpsc::Sender<UdpPacket>,
) -> eyre::Result<()> {
	const QUEUE: usize = 64;
	let (uplink_tx, mut uplink_rx) = mpsc::channel::<Vec<u8>>(QUEUE);
	let (downlink_tx, mut downlink_rx) = mpsc::channel::<UdpPacket>(QUEUE);

	// Coalesce the one-shot request header and the first datagram into the
	// opening write.
	let mut initial = uot::encode_request(&first.target)?;
	uot::encode_packet_into(&mut initial, &first.target, &first.payload)?;

	let io_handle = spawn_io_thread(UOT_IO_THREAD_NAME, move || {
		if naive.write_all(&initial).is_err() {
			return;
		}
		let _ = naive.flush();

		loop {
			let mut wrote = false;
			loop {
				match uplink_rx.try_recv() {
					Ok(frame) => {
						if naive.write_all(&frame).is_err() {
							return;
						}
						wrote = true;
					}
					// The relay dropped the uplink sender: stop now instead
					// of parking in one more read before it notices.
					Err(mpsc::error::TryRecvError::Disconnected) => return,
					Err(mpsc::error::TryRecvError::Empty) => break,
				}
			}
			if wrote {
				let _ = naive.flush();
			}

			match uot::read_packet(&mut naive) {
				Ok((source, payload)) => {
					let packet = UdpPacket {
						source: Some(source.clone()),
						target: source,
						payload: payload.into(),
					};
					if downlink_tx.blocking_send(packet).is_err() {
						return;
					}
				}
				Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
					tracing::debug!("UoT tunnel EOF");
					return;
				}
				Err(e) => {
					tracing::debug!(error = %e, "UoT tunnel read error");
					return;
				}
			}
		}
	})
	.context("spawn wind-naive-uot-io thread")?;

	let uplink = async move {
		while let Some(packet) = rx.recv().await {
			match uot::encode_packet(&packet.target, &packet.payload) {
				Ok(frame) => {
					if uplink_tx.send(frame).await.is_err() {
						break;
					}
				}
				Err(e) => {
					tracing::warn!(target = %packet.target, error = %e, "dropping un-encodable UDP packet");
				}
			}
		}
	};

	let downlink = async move {
		while let Some(packet) = downlink_rx.recv().await {
			if tx.send(packet).await.is_err() {
				break;
			}
		}
	};

	tokio::select! {
		_ = uplink => {}
		_ = downlink => {}
	}

	// Hold the I/O thread in a session guard until this relay returns, then
	// reap it: the guard's `Drop` queues the `join` on a blocking thread (see
	// [`reap_blocking_thread`]). Dropping the channels makes the thread's next
	// send fail *and* its next drain see `Disconnected`, so it unwinds
	// promptly and drops the `NaiveConn` — cancelling the Cronet stream and
	// closing its socket. Detaching the handle instead (the previous
	// behaviour) left that thread and socket alive for as long as the parked
	// Cronet read took, with no owner left to ever join them.
	let _session = JoinSession::new(io_handle);
	Ok(())
}

/// Relay a bridge's local stream and its blocking I/O thread until either side
/// ends.
///
/// Owns the relay's **single** read buffer: `read` fills it from the start and
/// reports the filled prefix, so the next read reuses it as is — no fresh
/// allocation and no re-zeroing. The previous code assigned
/// `local_buf = vec![0u8; NAIVE_BRIDGE_CHUNK]` after every successful read,
/// which allocated *and* zeroed 64 KiB per chunk (the old buffer is still alive
/// while the new one is allocated, so the allocator cannot hand back the same
/// block).
///
/// Split out of [`naive_async_bridge`] so the buffer's lifetime — one per
/// relay, not one per chunk — can be asserted without a Cronet engine.
async fn relay_local_and_naive(
	stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
	naive_write_tx: &mpsc::Sender<Vec<u8>>,
	naive_read_rx: &mut mpsc::Receiver<Vec<u8>>,
) {
	let mut local_buf = vec![0u8; NAIVE_BRIDGE_CHUNK];

	loop {
		tokio::select! {
			result = stream.read(&mut local_buf) => {
				match result {
					Ok(0) => break,
					Ok(n) => {
						if naive_write_tx.send(local_buf[..n].to_vec()).await.is_err() {
							break;
						}
					}
					Err(e) => {
						tracing::debug!(error = %e, "local stream read error");
						break;
					}
				}
			}
			Some(data) = naive_read_rx.recv() => {
				if let Err(e) = stream.write_all(&data).await {
					tracing::debug!(error = %e, "local stream write error");
					break;
				}
				let _ = stream.flush().await;
			}
			else => break,
		}
	}
}

/// Bridge data between a blocking Cronet connection
/// ([`cronet_rs::naive_conn::NaiveConn`]) and a tokio [`AsyncRead`] +
/// [`AsyncWrite`] stream.
///
/// Spawns a dedicated **std::thread** that owns the `NaiveConn` and relays
/// data through `mpsc` channels.  The Cronet C API expects all stream
/// operations from the same thread.
///
/// The thread is reaped through a [`JoinSession`] when the bridge returns, so
/// the thread and the connection's socket do not leak past the relay.
///
/// Generic over the connection so the relay's shutdown contract — including
/// that the I/O thread is handed to a [`JoinSession`] instead of being
/// detached — can be driven by a test double without a Cronet engine.
async fn naive_async_bridge<C>(naive: C, mut stream: impl AsyncRead + AsyncWrite + Unpin) -> eyre::Result<()>
where
	C: Read + Write + Send + 'static,
{
	let span = tracing::debug_span!("naive_bridge");

	async move {
		let mut naive = naive;

		// Bounded channels apply back-pressure to producers when the I/O
		// thread or the async reader can't keep up. The previous
		// `unbounded_channel` would let a stalled consumer accrete an
		// unbounded queue, causing the per-connection bridge to OOM under a
		// busy upstream. 64 entries × MAX chunk size keeps a tight ceiling
		// while remaining deep enough to absorb single-RTT bursts.
		const NAIVE_BRIDGE_QUEUE: usize = 64;
		let (naive_write_tx, mut naive_write_rx) = mpsc::channel::<Vec<u8>>(NAIVE_BRIDGE_QUEUE);
		let (naive_read_tx, mut naive_read_rx) = mpsc::channel::<Vec<u8>>(NAIVE_BRIDGE_QUEUE);

		let io_handle = spawn_io_thread(BRIDGE_IO_THREAD_NAME, move || {
			let mut read_buf = [0u8; NAIVE_BRIDGE_CHUNK];

			loop {
				loop {
					match naive_write_rx.try_recv() {
						Ok(data) => {
							if naive.write_all(&data).is_err() {
								return;
							}
							let _ = naive.flush();
						}
						// The relay dropped the uplink sender: stop now
						// instead of parking in one more read.
						Err(mpsc::error::TryRecvError::Disconnected) => return,
						Err(mpsc::error::TryRecvError::Empty) => break,
					}
				}

				match naive.read(&mut read_buf) {
					Ok(0) => {
						tracing::debug!("naive conn EOF");
						return;
					}
					Ok(n) => {
						// I/O thread is sync; use `blocking_send` so back-
						// pressure naturally stalls reads from `naive`
						// instead of OOMing the queue.
						if naive_read_tx.blocking_send(read_buf[..n].to_vec()).is_err() {
							return;
						}
					}
					Err(e) => {
						tracing::debug!(error = %e, "naive conn read error");
						return;
					}
				}
			}
		})
		.context("spawn wind-naive-io thread")?;

		// One reusable read buffer lives inside the relay (see
		// [`relay_local_and_naive`]); nothing here allocates per chunk.
		relay_local_and_naive(&mut stream, &naive_write_tx, &mut naive_read_rx).await;

		// `join` must not run inline here: the I/O thread may be parked in a
		// blocking `naive.read()`, and joining it from an async worker would
		// pin that worker until the remote speaks or the Cronet read times
		// out. The session guard reaps it on a blocking thread instead, so the
		// thread and the `NaiveConn`'s socket are actually reclaimed rather
		// than detached (same strategy as the UoT relay above).
		let _session = JoinSession::new(io_handle);
		Ok(())
	}
	.instrument(span)
	.await
}

/// Default search paths for `libcronet` (dynamic loading only).
#[cfg(feature = "dynamic")]
const CRONET_SEARCH_PATHS: &[&str] = &[
	"libcronet.so",
	"./libcronet.so",
	"/usr/local/lib/libcronet.so",
	"/opt/cronet/libcronet.so",
];

/// Load the `libcronet` shared library.
///
/// If `path` is `Some(...)`, that exact path is tried first.  On failure, or
/// when `path` is `None`, the default search paths are tried.
fn load_cronet(path: Option<String>) -> eyre::Result<()> {
	// With `static-link`, libcronet is linked at compile time — there is
	// nothing to dlopen, so skip the search entirely.
	#[cfg(not(feature = "dynamic"))]
	{
		let _ = path;
		return Ok(());
	}

	#[cfg(feature = "dynamic")]
	{
		load_cronet_dynamic(path)
	}
}

#[cfg(feature = "dynamic")]
fn load_cronet_dynamic(path: Option<String>) -> eyre::Result<()> {
	use cronet_rs::sys::load_library;

	// Candidate order: an explicit `cronet_lib_path` wins, then the prebuilt
	// fetched by `cronet-sys`'s build script under `--features download`, then
	// the implicit system search paths.
	let mut paths: Vec<String> = Vec::new();
	if let Some(p) = path {
		paths.push(p);
	}
	#[cfg(feature = "download")]
	if let Some(p) = cronet_rs::sys::downloaded_library_path() {
		paths.push(p.to_string());
	}
	paths.extend(CRONET_SEARCH_PATHS.iter().map(|s| (*s).to_string()));

	for lib_path in &paths {
		match unsafe { load_library(lib_path) } {
			Ok(()) => {
				info!(target: "naive", "Loaded libcronet from {lib_path}");
				return Ok(());
			}
			Err(e) => {
				let msg = format!("{e}");
				// "already loaded" means success.
				if msg.contains("already loaded") {
					return Ok(());
				}
				tracing::debug!(target: "naive", "libcronet not found at {lib_path}: {msg}");
			}
		}
	}

	let mut msg = format!(
		"Cannot load libcronet. Tried: {}. Please install libcronet and set `cronet_lib_path` in config, or place \
		 libcronet.so in LD_LIBRARY_PATH.",
		paths.join(", "),
	);
	#[cfg(feature = "download")]
	msg.push_str(
		" The `download` feature was enabled but no verified prebuilt was available; check the build output for the \
		 download/checksum error.",
	);
	#[cfg(not(feature = "download"))]
	msg.push_str(" Rebuild with `--features download` to fetch and SHA-256-verify a prebuilt libcronet.");
	Err(eyre::eyre!("{msg}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn host_from_authority_handles_ipv6_and_domains() {
		assert_eq!(host_from_authority("[2001:db8::1]:443"), "2001:db8::1");
		assert_eq!(host_from_authority("[::1]:8443"), "::1");
		assert_eq!(host_from_authority("example.com:443"), "example.com");
		assert_eq!(host_from_authority("1.2.3.4:443"), "1.2.3.4");
		assert_eq!(host_from_authority("example.com"), "example.com");
	}

	#[test]
	fn test_opts_default() {
		let opts = NaiveOutboundOpts::default();
		assert!(opts.server_address.is_empty());
		assert!(opts.server_name.is_none());
		assert!(opts.username.is_none());
		assert!(opts.password.is_none());
		assert_eq!(opts.concurrency, 1);
		assert!(!opts.quic_enabled);
		assert!(opts.trusted_root_certificates.is_none());
		assert!(!opts.ech_enabled);
		assert!(opts.extra_headers.is_empty());
		assert!(opts.cronet_lib_path.is_none());
	}

	#[test]
	fn test_opts_custom() {
		let opts = NaiveOutboundOpts {
			server_address: "proxy.example.com:443".into(),
			server_name: Some("proxy.example.com".into()),
			username: Some("user".into()),
			password: Some("pass".into()),
			concurrency: 2,
			quic_enabled: true,
			quic_congestion_control: QuicCongestionControl::Bbr,
			cronet_lib_path: Some("/opt/cronet/libcronet.so.119".into()),
			..Default::default()
		};
		assert_eq!(opts.server_address, "proxy.example.com:443");
		assert_eq!(opts.server_name.unwrap(), "proxy.example.com");
		assert_eq!(opts.username.unwrap(), "user");
		assert_eq!(opts.password.unwrap(), "pass");
		assert_eq!(opts.concurrency, 2);
		assert!(opts.quic_enabled);
		assert_eq!(opts.cronet_lib_path.unwrap(), "/opt/cronet/libcronet.so.119");
	}

	#[test]
	fn test_opts_with_auth() {
		let opts = NaiveOutboundOpts {
			server_address: "server.com:443".into(),
			username: Some("naive".into()),
			password: Some("+naive_password".into()),
			..Default::default()
		};
		assert_eq!(opts.server_address, "server.com:443");
		assert!(opts.username.is_some());
		assert!(opts.password.is_some());
	}

	#[test]
	fn test_opts_extra_headers() {
		let mut headers = std::collections::HashMap::new();
		headers.insert("X-Custom".into(), "value".into());

		let opts = NaiveOutboundOpts {
			server_address: "s.example.com:443".into(),
			extra_headers: headers,
			..Default::default()
		};
		assert_eq!(opts.extra_headers.get("X-Custom").unwrap(), "value");
	}

	/// With `download` enabled, `cronet-sys`'s build script must have fetched a
	/// prebuilt and exposed a real, existing path that the loader will try.
	#[cfg(feature = "download")]
	#[test]
	fn download_feature_exposes_a_prebuilt_library() {
		let path = cronet_rs::sys::downloaded_library_path().expect("`download` feature should expose a prebuilt library path");
		assert!(
			std::path::Path::new(path).exists(),
			"downloaded libcronet should exist on disk: {path}"
		);
	}

	/// A stand-in for the blocking Cronet connection: its first `read` parks
	/// until the test releases it (then reports EOF), and writes are discarded.
	struct BlockingIo {
		connected: std::sync::mpsc::Sender<()>,
		release: std::sync::mpsc::Receiver<()>,
	}

	impl std::io::Read for BlockingIo {
		fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
			let _ = self.connected.send(());
			// A closed sender (test finished) or the timeout both mean "go on".
			let _ = self.release.recv_timeout(std::time::Duration::from_secs(5));
			Ok(0)
		}
	}

	impl std::io::Write for BlockingIo {
		fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
			Ok(buf.len())
		}

		fn flush(&mut self) -> std::io::Result<()> {
			Ok(())
		}
	}

	/// The TCP bridge must hand its blocking I/O thread to a [`JoinSession`]
	/// when the relay ends, instead of detaching the handle.
	///
	/// The connection double parks in its first `read` until the test releases
	/// it, so the thread provably cannot finish on its own while the relay
	/// runs. A detached handle leaves `reaped_io_threads` frozen and this test
	/// fails; the session's reaper moves it.
	///
	/// A multi-thread runtime is required: the test blocks its own thread while
	/// waiting for the I/O thread to park, so the bridge task and that thread
	/// must be able to make progress in parallel.
	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn bridge_hands_its_io_thread_to_the_session() {
		let (connected_tx, connected_rx) = std::sync::mpsc::channel::<()>();
		let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();

		// A live local stream keeps the relay's `select!` pending until the
		// test drops the peer, so the bridge future does not return before the
		// I/O thread has been created and parked.
		let (local, peer) = tokio::io::duplex(64);

		let before = reaped_io_threads();
		let bridge = tokio::spawn(naive_async_bridge(
			BlockingIo {
				connected: connected_tx,
				release: release_rx,
			},
			local,
		));

		connected_rx
			.recv_timeout(std::time::Duration::from_secs(10))
			.expect("the bridge must have spawned its I/O thread and parked it in read");

		// The thread is parked in its first `read`, so neither the relay nor a
		// detached handle can have reaped anything yet.
		assert_eq!(
			reaped_io_threads(),
			before,
			"a parked io thread must not be reported as reaped"
		);

		// End the relay, then let the I/O thread unwind.
		drop(peer);
		bridge
			.await
			.expect("bridge task should not panic")
			.expect("bridge should end cleanly");
		let _ = release_tx.send(());

		let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
		while reaped_io_threads() == before && std::time::Instant::now() < deadline {
			// A plain sleep: reaping happens on the blocking pool, so nothing
			// has to be driven on this runtime for it to progress.
			std::thread::sleep(std::time::Duration::from_millis(10));
		}
		assert_eq!(
			reaped_io_threads(),
			before + 1,
			"the bridge must join its io thread instead of detaching the handle"
		);
	}

	/// A local-stream double that records the **address** of the buffer every
	/// read is handed, then replays `chunks` and reports EOF.
	///
	/// An entry in a `ReadBuf` is the slice the caller passed to `read`, so
	/// with nothing filled yet `filled().as_ptr()` is the address of that
	/// buffer: two reads that observe the same address were handed the same
	/// buffer.
	struct ReadBufAddressProbe {
		chunks: std::collections::VecDeque<Vec<u8>>,
		bases: Arc<std::sync::Mutex<Vec<usize>>>,
	}

	impl tokio::io::AsyncRead for ReadBufAddressProbe {
		fn poll_read(
			self: std::pin::Pin<&mut Self>,
			_cx: &mut std::task::Context<'_>,
			buf: &mut tokio::io::ReadBuf<'_>,
		) -> std::task::Poll<std::io::Result<()>> {
			let this = self.get_mut();
			this.bases.lock().unwrap().push(buf.filled().as_ptr() as usize);
			if let Some(chunk) = this.chunks.pop_front() {
				buf.put_slice(&chunk);
			}
			// An unfilled `ReadBuf` is how a reader reports EOF.
			std::task::Poll::Ready(Ok(()))
		}
	}

	impl tokio::io::AsyncWrite for ReadBufAddressProbe {
		fn poll_write(
			self: std::pin::Pin<&mut Self>,
			_cx: &mut std::task::Context<'_>,
			buf: &[u8],
		) -> std::task::Poll<std::io::Result<usize>> {
			std::task::Poll::Ready(Ok(buf.len()))
		}

		fn poll_flush(
			self: std::pin::Pin<&mut Self>,
			_cx: &mut std::task::Context<'_>,
		) -> std::task::Poll<std::io::Result<()>> {
			std::task::Poll::Ready(Ok(()))
		}

		fn poll_shutdown(
			self: std::pin::Pin<&mut Self>,
			_cx: &mut std::task::Context<'_>,
		) -> std::task::Poll<std::io::Result<()>> {
			std::task::Poll::Ready(Ok(()))
		}
	}

	/// One read buffer must serve the whole relay, not one per chunk.
	///
	/// The previous loop body re-created the buffer after every successful read
	/// (`local_buf = vec![0u8; 65535]`), allocating *and* zeroing 64 KiB per
	/// chunk. That new allocation is taken while the buffer it replaces is
	/// still alive, so consecutive reads cannot observe the same address — the
	/// same probe passes only when the relay keeps one buffer.
	///
	/// This drives the real relay loop ([`relay_local_and_naive`]) with a
	/// stream double; no Cronet engine, I/O thread, or runtime thread is
	/// involved, so nothing else in this test binary can perturb the
	/// observation.
	#[tokio::test]
	async fn relay_reuses_one_read_buffer_instead_of_reallocating_per_chunk() {
		const CHUNKS: usize = 8;

		let bases = Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
		let chunks: std::collections::VecDeque<Vec<u8>> = (0..CHUNKS).map(|i| vec![i as u8 + 1; 64]).collect();
		let mut stream = ReadBufAddressProbe {
			chunks,
			bases: Arc::clone(&bases),
		};

		let (write_tx, mut write_rx) = mpsc::channel::<Vec<u8>>(64);
		// Held (and silent) so the downlink branch never becomes ready.
		let (_read_tx, mut read_rx) = mpsc::channel::<Vec<u8>>(64);

		relay_local_and_naive(&mut stream, &write_tx, &mut read_rx).await;

		let observed = bases.lock().unwrap().clone();
		assert_eq!(
			observed.len(),
			CHUNKS + 1,
			"every chunk and the EOF read must have been handed a buffer"
		);
		assert!(
			observed.iter().all(|base| *base == observed[0]),
			"one buffer must serve the whole relay, but the reads saw distinct buffers: {observed:?}"
		);

		// The chunks must still arrive whole and in order.
		let received: Vec<Vec<u8>> = std::iter::from_fn(|| write_rx.try_recv().ok()).collect();
		assert_eq!(received.len(), CHUNKS);
		for (i, chunk) in received.iter().enumerate() {
			assert_eq!(chunk, &vec![i as u8 + 1; 64], "chunk {i} must arrive unchanged");
		}
	}

	/// A refused bridge I/O thread must be reported through the caller's
	/// `eyre::Result`, not turned into a task-wide panic.
	///
	/// Both bridge spawn sites used `Builder::spawn(..).expect(..)`, which
	/// unwound as soon as the OS refused a thread. `spawn_io_thread_with` only
	/// became able to report that as a `Result` in this change, so this
	/// fails-before: the error path did not exist, and the spawn could not be
	/// *offered* the refused thread at all.
	#[test]
	fn a_refused_io_thread_is_reported_instead_of_panicking() {
		let mut offered: Vec<String> = Vec::new();

		let e = spawn_io_thread_with(
			BRIDGE_IO_THREAD_NAME,
			|name, _body| {
				offered.push(name.to_string());
				Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "thread limit reached"))
			},
			|| unreachable!("a refused thread must not be started"),
		)
		.context("spawn wind-naive-io thread")
		.expect_err("a refused thread must surface as an error");

		assert!(
			e.to_string().starts_with("spawn wind-naive-io thread"),
			"the spawn failure must keep the bridge's context: {e}"
		);
		// `eyre` renders only the outermost context with `Display`, but keeps
		// the refused-thread error as the report's cause, so the operator can
		// still see *why* the thread was refused.
		let chain = format!("{e:?}");
		assert!(
			chain.contains("thread limit reached"),
			"the OS error must stay in the report's cause chain: {chain}"
		);
		assert_eq!(
			offered,
			vec![BRIDGE_IO_THREAD_NAME.to_string()],
			"the bridge's thread name must be used"
		);
	}

	/// Failing to spawn the *reaper* must not panic either: `JoinSession::drop`
	/// calls it on the async worker's unwind-free path, and a panic there
	/// aborts the process when the drop is itself running during an unwind.
	///
	/// This test runs outside a tokio runtime on purpose — that is the branch
	/// that spawns the plain reaper thread, and so the only one where the
	/// refusal has to be absorbed.
	#[test]
	fn a_refused_reaper_thread_is_logged_instead_of_panicking() {
		let (tx, rx) = std::sync::mpsc::channel::<()>();
		let handle = std::thread::spawn(move || {
			let _ = tx.send(());
		});
		let _ = rx.recv_timeout(std::time::Duration::from_secs(5));

		let attempted = std::cell::Cell::new(false);
		reap_blocking_thread_with(handle, |_name, _body| {
			attempted.set(true);
			Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "thread limit reached"))
		});
		assert!(attempted.get(), "the reaper must have attempted its plain spawn");
	}
}
