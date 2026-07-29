//! Naive outbound using `cronet-rs` (Cronet-based CONNECT proxy).
//!
//! This crate provides [`NaiveOutbound`], an implementation of
//! [`wind_core::AbstractOutbound`] that tunnels TCP (and UDP-over-TCP) through
//! a NaiveProxy server via the Cronet HTTP/2 or QUIC CONNECT protocol with
//! padding.

use std::{
	io::{Read, Write},
	sync::Arc,
};

use cronet_rs::naive_client::{NaiveClient, NaiveClientConfig, QuicCongestionControl as CronetCongestionControl};
use eyre::Context as _;
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
	sync::mpsc,
};
use tracing::{Instrument, info};
use wind_core::{
	AbstractOutbound, QuicCongestionControl,
	tcp::AbstractTcpStream,
	types::TargetAddr,
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

impl AbstractOutbound for NaiveOutbound {
	async fn handle_tcp(
		&self,
		target_addr: TargetAddr,
		stream: impl AbstractTcpStream,
		_via: Option<impl AbstractOutbound + Sized + Send>,
	) -> eyre::Result<()> {
		let target_str = target_addr.to_string();
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
	async fn handle_udp(&self, udp_stream: UdpStream, _via: Option<impl AbstractOutbound + Sized + Send>) -> eyre::Result<()> {
		let UdpStream { tx, mut rx } = udp_stream;

		// Defer opening the tunnel until the first datagram so idle UDP
		// associations cost nothing, and so the UoT request header can name a
		// concrete destination.
		let first = match rx.recv().await {
			Some(p) => p,
			None => return Ok(()),
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

	let io_handle = std::thread::Builder::new()
		.name("wind-naive-uot-io".into())
		.spawn(move || {
			if naive.write_all(&initial).is_err() {
				return;
			}
			let _ = naive.flush();

			loop {
				let mut wrote = false;
				while let Ok(frame) = uplink_rx.try_recv() {
					if naive.write_all(&frame).is_err() {
						return;
					}
					wrote = true;
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
		.expect("spawn wind-naive-uot-io thread");

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

	// Do not `join` the I/O thread: it may be parked in a blocking read with no
	// downlink traffic. Dropping the channels makes its next write fail and the
	// `NaiveConn` is cancelled when the thread finally unwinds and drops it.
	drop(io_handle);
	Ok(())
}

/// Bridge data between a blocking [`cronet_rs::naive_conn::NaiveConn`] and a
/// tokio [`AsyncRead`] + [`AsyncWrite`] stream.
///
/// Spawns a dedicated **std::thread** that owns the `NaiveConn` and relays
/// data through `mpsc` channels.  The Cronet C API expects all stream
/// operations from the same thread.
async fn naive_async_bridge(
	mut naive: cronet_rs::naive_conn::NaiveConn,
	mut stream: impl AsyncRead + AsyncWrite + Unpin,
) -> eyre::Result<()> {
	let span = tracing::debug_span!("naive_bridge");

	async move {
		// Bounded channels apply back-pressure to producers when the I/O
		// thread or the async reader can't keep up. The previous
		// `unbounded_channel` would let a stalled consumer accrete an
		// unbounded queue, causing the per-connection bridge to OOM under a
		// busy upstream. 64 entries × MAX chunk size keeps a tight ceiling
		// while remaining deep enough to absorb single-RTT bursts.
		const NAIVE_BRIDGE_QUEUE: usize = 64;
		let (naive_write_tx, mut naive_write_rx) = mpsc::channel::<Vec<u8>>(NAIVE_BRIDGE_QUEUE);
		let (naive_read_tx, mut naive_read_rx) = mpsc::channel::<Vec<u8>>(NAIVE_BRIDGE_QUEUE);

		let io_handle = std::thread::Builder::new()
			.name("wind-naive-io".into())
			.spawn(move || {
				let mut read_buf = [0u8; 65535];

				loop {
					while let Ok(data) = naive_write_rx.try_recv() {
						if naive.write_all(&data).is_err() {
							return;
						}
						let _ = naive.flush();
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
			.expect("spawn wind-naive-io thread");

		let mut local_buf = vec![0u8; 65535];

		loop {
			tokio::select! {
				result = stream.read(&mut local_buf) => {
					match result {
						Ok(0) => break,
						Ok(n) => {
							if naive_write_tx.send(local_buf[..n].to_vec()).await.is_err() {
								break;
							}
							local_buf = vec![0u8; 65535];
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

		// Do not `join` the I/O thread from this async task: it may be parked in
		// a blocking `naive.read()` with no traffic, and `join()` would pin a
		// tokio worker thread until the remote finally speaks or times out.
		// Dropping the channels instead makes the thread's next send/recv fail,
		// and the `NaiveConn` is cancelled when the thread unwinds and drops it
		// (same strategy as the UoT relay above).
		drop(io_handle);
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
	// With `static-link`, libcronet is linked at compile time — there is nothing
	// to dlopen, so skip the search entirely.
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

	let paths: Vec<&str> = if let Some(ref p) = path {
		let mut v = vec![p.as_str()];
		v.extend(CRONET_SEARCH_PATHS);
		v
	} else {
		CRONET_SEARCH_PATHS.to_vec()
	};


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

	Err(eyre::eyre!(
		"Cannot load libcronet. Tried: {}. Please install libcronet and set `cronet_lib_path` in config, or place \
		 libcronet.so in LD_LIBRARY_PATH.",
		paths.join(", "),
	))
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
}
