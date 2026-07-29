//! A protocol-agnostic [`ApplicationOverQuic`] that turns the sans-IO quiche
//! worker loop into the handle-based async [`QuicConnection`] surface.
//!
//! This generalizes `wind-tuiche`'s `TuicheDriver`: instead of decoding TUIC
//! inside the worker, it exposes *raw* QUIC streams and datagrams through
//! channels. A [`QuicheConnection`](crate::quiche::conn::QuicheConnection)
//! handle talks to this driver over a command channel and per-stream byte
//! channels, so all protocol logic lives above the abstraction.
//!
//! ## Model
//!
//! tokio-quiche runs one worker task per connection and calls our methods
//! synchronously with `&mut quiche::Connection`:
//!
//! * [`process_reads`] drains readable streams / datagrams into per-stream
//!   inbound channels (and surfaces peer-initiated streams to the accept
//!   queues).
//! * [`process_writes`] flushes queued opens, datagrams, stream data + FINs,
//!   stream shutdowns, keying-material exports, and connection close.
//! * [`wait_for_data`] parks until a handle sends a command or a stream's
//!   outbound back-channel produces data.
//!
//! [`process_reads`]: ApplicationOverQuic::process_reads
//! [`process_writes`]: ApplicationOverQuic::process_writes
//! [`wait_for_data`]: ApplicationOverQuic::wait_for_data
//! [`QuicConnection`]: crate::traits::QuicConnection

use std::{
	collections::{HashMap, VecDeque},
	future::Future,
	net::SocketAddr,
	pin::Pin,
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
	},
	task::{Context, Poll},
};

use bytes::{Buf, Bytes};
use futures_util::{StreamExt, stream::FuturesUnordered};
use tokio::sync::{Notify, mpsc, mpsc::error::TrySendError, oneshot};
use tokio_quiche::{
	ApplicationOverQuic, QuicResult,
	metrics::Metrics,
	quic::{HandshakeInfo, QuicheConnection},
	quiche::{self, Shutdown},
};
use tracing::{Span, debug, trace};

use crate::quiche::{
	conn::QuicheConnection as Handle,
	stream::{QuicheRecv, QuicheSend},
};

/// Per-stream byte-channel capacity (chunks).
pub(crate) const STREAM_CHAN_CAP: usize = 64;
/// Worker scratch buffer size.
const READ_BUF_SIZE: usize = 64 * 1024;
/// Soft cap on buffered inbound bytes per stream before we stop draining the
/// QUIC stream (lets flow control back-pressure the peer).
const MAX_PENDING_IN: usize = 256 * 1024;
/// Soft cap on buffered *outbound* bytes per stream. Once `out_queue` reaches
/// this, the driver stops draining the handle's send back-channel until
/// `write_stream` flushes below the cap — so a slow/flow-controlled peer
/// back-pressures the local writer instead of letting `out_queue` grow without
/// bound (previously a single slow peer could OOM the process).
const MAX_PENDING_OUT: usize = 256 * 1024;
/// Hard cap on queued outbound datagrams. Datagrams are unreliable, so the
/// oldest are dropped past this — bounding memory when the app queues faster
/// than the peer drains (the command channel itself is unbounded).
const MAX_OUT_DATAGRAMS: usize = 2048;

/// An item delivered on a stream's inbound channel: either a chunk of peer
/// data, or `Err(code)` signaling the peer reset the stream (RESET_STREAM) so
/// the receiver can surface an error instead of a clean EOF. A clean FIN is
/// still signaled by the sender being dropped (channel closed).
pub(crate) type InboundItem = Result<Bytes, u64>;

/// Command channel sender from a handle to its driver.
pub(crate) type CmdTx = mpsc::UnboundedSender<DriverCommand>;
type AcceptBiTx = mpsc::UnboundedSender<(QuicheSend, QuicheRecv)>;
type AcceptUniTx = mpsc::UnboundedSender<QuicheRecv>;
type DgramInTx = mpsc::UnboundedSender<Bytes>;
/// A queued keying-material export request: `(out_len, label, context, reply)`.
type ExportReq = (usize, Vec<u8>, Vec<u8>, oneshot::Sender<Option<Vec<u8>>>);

/// A command issued by a [`QuicheConnection`](Handle) handle, executed by the
/// worker where it has `&mut quiche::Connection` access.
pub(crate) enum DriverCommand {
	/// Open a local bidirectional stream; reply with its halves.
	OpenBi(oneshot::Sender<(QuicheSend, QuicheRecv)>),
	/// Open a local unidirectional (send-only) stream; reply with its send
	/// half.
	OpenUni(oneshot::Sender<QuicheSend>),
	/// Queue a datagram for sending.
	SendDatagram(Bytes),
	/// Export keying material (RFC 5705).
	Export {
		out_len: usize,
		label: Vec<u8>,
		context: Vec<u8>,
		reply: oneshot::Sender<Option<Vec<u8>>>,
	},
	/// Shut down one direction of a stream with an error code.
	StreamShutdown { sid: u64, write: bool, code: u64 },
	/// Close the connection.
	Close { code: u32, reason: Vec<u8> },
	/// Wake the worker so it re-flushes a stream's buffered inbound bytes after
	/// the handle drained its recv channel. Without this, buffered `pending_in`
	/// data (and the FIN that follows it) could stall until some unrelated
	/// event happened to wake the worker — e.g. on a pure-upload stream with
	/// no reverse traffic.
	FlushInbound(u64),
}

/// State shared between a driver and its handle(s).
pub(crate) struct Shared {
	/// Max writable datagram length (0 == datagrams unavailable).
	pub max_dgram: AtomicUsize,
	/// Set once the connection has closed.
	pub closed: AtomicBool,
	/// Notified when `closed` transitions to true.
	pub closed_notify: Notify,
	/// Cumulative wire bytes the local endpoint has sent (server→client =
	/// download). Refreshed each worker pass and finalized in `on_conn_close`,
	/// so handles can read it without a driver round-trip and the final count
	/// survives connection close.
	pub sent_bytes: AtomicU64,
	/// Cumulative wire bytes received from the peer (client→server = upload).
	/// See [`sent_bytes`](Self::sent_bytes).
	pub recv_bytes: AtomicU64,
}

/// Per-stream bridge state held by the driver.
struct StreamIo {
	/// Driver → handle (peer's data). `None` once EOF/reset has been delivered.
	inbound_tx: Option<mpsc::Sender<InboundItem>>,
	/// Bytes read from the stream but not yet accepted by `inbound_tx`.
	pending_in: VecDeque<Bytes>,
	pending_in_len: usize,
	/// Peer FIN observed.
	in_fin: bool,
	/// Peer RESET_STREAM code observed on the recv side. When set, an
	/// `Err(code)` is delivered after `pending_in` drains, so the handle sees
	/// an error rather than a clean EOF for a truncated stream.
	in_reset: Option<u64>,
	/// Handle → driver data awaiting `stream_send`.
	out_queue: VecDeque<Bytes>,
	/// Total bytes currently buffered in `out_queue` (for the outbound cap).
	out_queue_len: usize,
	/// The handle's send back-channel receiver, parked here when `out_queue`
	/// hit [`MAX_PENDING_OUT`]. Re-armed by `write_stream` once the queue
	/// drains below the cap, so the local writer sees back-pressure meanwhile.
	parked_out_rx: Option<mpsc::Receiver<Bytes>>,
	/// Handle closed its send half; emit a FIN once `out_queue` drains.
	out_done: bool,
	fin_sent: bool,
	/// The peer refused our send half (STOP_SENDING) or the stream errored on
	/// `stream_send`. Once set, the back-channel is closed instead of re-armed
	/// so the local writer's next `poll_write` fails rather than silently
	/// dropping data.
	send_failed: bool,
	/// Whether this stream has a local send half (bidi, or locally-opened uni).
	has_send: bool,
}

impl StreamIo {
	fn new(inbound_tx: Option<mpsc::Sender<InboundItem>>, has_send: bool) -> Self {
		Self {
			inbound_tx,
			pending_in: VecDeque::new(),
			pending_in_len: 0,
			in_fin: false,
			in_reset: None,
			out_queue: VecDeque::new(),
			out_queue_len: 0,
			parked_out_rx: None,
			out_done: false,
			fin_sent: false,
			send_failed: false,
			has_send,
		}
	}
}

/// A pending local stream open awaiting the worker.
enum PendingOpen {
	Bi(oneshot::Sender<(QuicheSend, QuicheRecv)>),
	Uni(oneshot::Sender<QuicheSend>),
}

/// Future that resolves when a stream's outbound back-channel yields a chunk
/// (or closes), returning the receiver so the driver can re-arm it. Modeled on
/// `wind-tuiche`'s `WaitTcpBack`.
struct WaitOut {
	sid: u64,
	rx: Option<mpsc::Receiver<Bytes>>,
}

impl WaitOut {
	fn new(sid: u64, rx: mpsc::Receiver<Bytes>) -> Self {
		Self { sid, rx: Some(rx) }
	}
}

impl Future for WaitOut {
	type Output = (u64, Option<Bytes>, mpsc::Receiver<Bytes>);

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let sid = self.sid;
		let rx = self.rx.as_mut().expect("WaitOut polled after completion");
		match rx.poll_recv(cx) {
			Poll::Ready(data) => {
				let rx = self.rx.take().unwrap();
				Poll::Ready((sid, data, rx))
			}
			Poll::Pending => Poll::Pending,
		}
	}
}

/// The protocol-agnostic quiche driver.
pub(crate) struct BridgeDriver {
	is_server: bool,
	span: Span,
	established: bool,
	buffer: Vec<u8>,

	streams: HashMap<u64, StreamIo>,
	waiters: FuturesUnordered<WaitOut>,
	next_bi: u64,
	next_uni: u64,

	cmd_rx: mpsc::UnboundedReceiver<DriverCommand>,
	cmd_tx_handles: CmdTx,
	accept_bi_tx: AcceptBiTx,
	accept_uni_tx: AcceptUniTx,
	dgram_in_tx: DgramInTx,

	out_datagrams: VecDeque<Bytes>,
	pending_opens: VecDeque<PendingOpen>,
	pending_exports: VecDeque<ExportReq>,
	pending_shutdowns: VecDeque<(u64, bool, u64)>,
	pending_close: Option<(u32, Vec<u8>)>,

	shared: Arc<Shared>,
	established_tx: Option<oneshot::Sender<Handle>>,
	handle: Option<Handle>,
}

impl BridgeDriver {
	/// Build a driver and the receiver that yields the connection handle once
	/// the handshake completes.
	pub(crate) fn new(is_server: bool, peer_addr: SocketAddr, span: Span) -> (Self, oneshot::Receiver<Handle>) {
		let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
		let (accept_bi_tx, accept_bi_rx) = mpsc::unbounded_channel();
		let (accept_uni_tx, accept_uni_rx) = mpsc::unbounded_channel();
		let (dgram_in_tx, dgram_in_rx) = mpsc::unbounded_channel();
		let shared = Arc::new(Shared {
			max_dgram: AtomicUsize::new(0),
			closed: AtomicBool::new(false),
			closed_notify: Notify::new(),
			sent_bytes: AtomicU64::new(0),
			recv_bytes: AtomicU64::new(0),
		});
		let handle = Handle::new(
			cmd_tx.clone(),
			accept_bi_rx,
			accept_uni_rx,
			dgram_in_rx,
			shared.clone(),
			peer_addr,
		);
		let (est_tx, est_rx) = oneshot::channel();
		let driver = Self {
			is_server,
			span,
			established: false,
			buffer: vec![0u8; READ_BUF_SIZE],
			streams: HashMap::new(),
			waiters: FuturesUnordered::new(),
			next_bi: 0,
			next_uni: 0,
			cmd_rx,
			cmd_tx_handles: cmd_tx,
			accept_bi_tx,
			accept_uni_tx,
			dgram_in_tx,
			out_datagrams: VecDeque::new(),
			pending_opens: VecDeque::new(),
			pending_exports: VecDeque::new(),
			pending_shutdowns: VecDeque::new(),
			pending_close: None,
			shared,
			established_tx: Some(est_tx),
			handle: Some(handle),
		};
		(driver, est_rx)
	}

	/// Allocate the next locally-initiated stream id of the given
	/// directionality.
	///
	/// QUIC stream ids encode the initiator in bit 0 (0 = client, 1 = server)
	/// and directionality in bit 1 (0 = bidi, 1 = uni); the sequence number
	/// occupies the upper bits.
	fn alloc_id(&mut self, uni: bool) -> u64 {
		let init = if self.is_server { 1 } else { 0 };
		let dir = if uni { 2 } else { 0 };
		let seq = if uni {
			let s = self.next_uni;
			self.next_uni += 1;
			s
		} else {
			let s = self.next_bi;
			self.next_bi += 1;
			s
		};
		(seq << 2) | dir | init
	}

	fn is_peer_initiated(&self, sid: u64) -> bool {
		let init = if self.is_server { 1 } else { 0 };
		(sid & 1) != init
	}

	fn open_local_bi(&mut self) -> (QuicheSend, QuicheRecv) {
		let sid = self.alloc_id(false);
		let (in_tx, in_rx) = mpsc::channel(STREAM_CHAN_CAP);
		let (out_tx, out_rx) = mpsc::channel(STREAM_CHAN_CAP);
		self.streams.insert(sid, StreamIo::new(Some(in_tx), true));
		self.waiters.push(WaitOut::new(sid, out_rx));
		(
			QuicheSend::new(sid, self.cmd_tx_handles.clone(), out_tx),
			QuicheRecv::new(sid, self.cmd_tx_handles.clone(), in_rx),
		)
	}

	fn open_local_uni(&mut self) -> QuicheSend {
		let sid = self.alloc_id(true);
		let (out_tx, out_rx) = mpsc::channel(STREAM_CHAN_CAP);
		// Send-only: no inbound channel.
		self.streams.insert(sid, StreamIo::new(None, true));
		self.waiters.push(WaitOut::new(sid, out_rx));
		QuicheSend::new(sid, self.cmd_tx_handles.clone(), out_tx)
	}

	/// Register a newly-observed peer-initiated stream and surface it to the
	/// matching accept queue.
	fn ensure_incoming(&mut self, sid: u64) {
		if self.streams.contains_key(&sid) {
			return;
		}
		let uni = sid & 2 != 0;
		let (in_tx, in_rx) = mpsc::channel(STREAM_CHAN_CAP);
		if uni {
			self.streams.insert(sid, StreamIo::new(Some(in_tx), false));
			let recv = QuicheRecv::new(sid, self.cmd_tx_handles.clone(), in_rx);
			let _ = self.accept_uni_tx.send(recv);
		} else {
			let (out_tx, out_rx) = mpsc::channel(STREAM_CHAN_CAP);
			self.streams.insert(sid, StreamIo::new(Some(in_tx), true));
			self.waiters.push(WaitOut::new(sid, out_rx));
			let send = QuicheSend::new(sid, self.cmd_tx_handles.clone(), out_tx);
			let recv = QuicheRecv::new(sid, self.cmd_tx_handles.clone(), in_rx);
			let _ = self.accept_bi_tx.send((send, recv));
		}
	}

	fn read_stream(&mut self, qconn: &mut QuicheConnection, sid: u64) {
		loop {
			let over_cap = self
				.streams
				.get(&sid)
				.map(|st| st.pending_in_len >= MAX_PENDING_IN)
				.unwrap_or(true);
			if over_cap {
				break;
			}
			match qconn.stream_recv(sid, &mut self.buffer) {
				Ok((n, fin)) => {
					if let Some(st) = self.streams.get_mut(&sid) {
						if n > 0 && st.inbound_tx.is_some() {
							st.pending_in.push_back(Bytes::copy_from_slice(&self.buffer[..n]));
							st.pending_in_len += n;
						}
						if fin {
							st.in_fin = true;
						}
					}
					if fin {
						break;
					}
				}
				Err(quiche::Error::Done) => break,
				Err(quiche::Error::StreamReset(code)) => {
					// The peer aborted the stream. Record the code so the handle
					// sees an error after any already-buffered bytes drain, rather
					// than a clean EOF that would make a truncated stream look
					// complete.
					trace!(stream = sid, code, "stream reset by peer");
					if let Some(st) = self.streams.get_mut(&sid) {
						st.in_reset = Some(code);
						st.in_fin = true;
					}
					break;
				}
				Err(e) => {
					trace!(stream = sid, "stream_recv error: {e}");
					if let Some(st) = self.streams.get_mut(&sid) {
						st.in_fin = true;
					}
					break;
				}
			}
		}
		self.flush_inbound(sid);
	}

	/// Move buffered inbound bytes into the handle's recv channel; drop the
	/// sender on EOF.
	fn flush_inbound(&mut self, sid: u64) {
		if let Some(st) = self.streams.get_mut(&sid) {
			if let Some(tx) = st.inbound_tx.clone() {
				while let Some(front) = st.pending_in.front().cloned() {
					match tx.try_send(Ok(front)) {
						Ok(()) => {
							let b = st.pending_in.pop_front().unwrap();
							st.pending_in_len -= b.len();
						}
						Err(TrySendError::Full(_)) => break,
						Err(TrySendError::Closed(_)) => {
							st.pending_in.clear();
							st.pending_in_len = 0;
							break;
						}
					}
				}
			}
			if st.in_fin && st.pending_in.is_empty() {
				// All buffered data delivered. If the stream was reset, deliver the
				// reset code as a final `Err` before closing the channel; otherwise
				// a dropped sender = clean EOF. If the channel is momentarily full,
				// keep the sender and retry on the next flush (the handle nudges us
				// via `FlushInbound` when it drains).
				match st.in_reset {
					Some(code) => {
						if let Some(tx) = st.inbound_tx.clone() {
							match tx.try_send(Err(code)) {
								Ok(()) => {
									st.inbound_tx = None;
									st.in_reset = None;
								}
								Err(TrySendError::Full(_)) => {}
								Err(TrySendError::Closed(_)) => {
									st.inbound_tx = None;
									st.in_reset = None;
								}
							}
						} else {
							st.in_reset = None;
						}
					}
					None => st.inbound_tx = None,
				}
			}
		}
		self.maybe_cleanup(sid);
	}

	fn write_stream(&mut self, qconn: &mut QuicheConnection, sid: u64) {
		let mut rearm: Option<mpsc::Receiver<Bytes>> = None;
		if let Some(st) = self.streams.get_mut(&sid)
			&& st.has_send
		{
			while let Some(front) = st.out_queue.front_mut() {
				if front.is_empty() {
					st.out_queue.pop_front();
					continue;
				}
				match qconn.stream_send(sid, front.as_ref(), false) {
					Ok(0) => break,
					Ok(n) => {
						front.advance(n);
						st.out_queue_len -= n;
						if front.is_empty() {
							st.out_queue.pop_front();
						}
					}
					Err(quiche::Error::Done) => break,
					Err(e) => {
						// The peer refused our send half (STOP_SENDING) or the stream
						// is otherwise unwritable. Drop the queued data and mark the
						// send side failed so the back-channel is closed rather than
						// re-armed — the local writer's next `poll_write` then fails
						// instead of silently succeeding into a black hole.
						debug!(stream = sid, "stream_send error: {e}");
						st.out_queue.clear();
						st.out_queue_len = 0;
						st.out_done = true;
						st.send_failed = true;
						// If the back-channel was parked, close it now; otherwise the
						// `Ev::Out` handler drops it when the next write arrives.
						st.parked_out_rx = None;
						break;
					}
				}
			}
			if st.out_done && st.out_queue.is_empty() && !st.fin_sent {
				match qconn.stream_send(sid, b"", true) {
					Ok(_) => st.fin_sent = true,
					Err(quiche::Error::Done) => {}
					Err(e) => {
						debug!(stream = sid, "stream_send fin error: {e}");
						st.fin_sent = true;
					}
				}
			}
			// Resume draining the handle's back-channel now that we're back under
			// the outbound cap (see `MAX_PENDING_OUT`).
			if st.out_queue_len < MAX_PENDING_OUT
				&& let Some(rx) = st.parked_out_rx.take()
			{
				rearm = Some(rx);
			}
		}
		if let Some(rx) = rearm {
			self.waiters.push(WaitOut::new(sid, rx));
		}
		self.maybe_cleanup(sid);
	}

	fn maybe_cleanup(&mut self, sid: u64) {
		let done = if let Some(st) = self.streams.get(&sid) {
			let recv_done = st.inbound_tx.is_none() && st.pending_in.is_empty();
			let send_done = !st.has_send || st.send_failed || (st.out_done && st.out_queue.is_empty() && st.fin_sent);
			recv_done && send_done
		} else {
			false
		};
		if done {
			self.streams.remove(&sid);
		}
	}

	fn handle_cmd(&mut self, cmd: DriverCommand) {
		match cmd {
			DriverCommand::OpenBi(reply) => self.pending_opens.push_back(PendingOpen::Bi(reply)),
			DriverCommand::OpenUni(reply) => self.pending_opens.push_back(PendingOpen::Uni(reply)),
			DriverCommand::SendDatagram(b) => {
				self.out_datagrams.push_back(b);
				// Bound queued datagrams: drop the oldest past the cap (datagrams
				// are unreliable, and the command channel is unbounded).
				while self.out_datagrams.len() > MAX_OUT_DATAGRAMS {
					self.out_datagrams.pop_front();
				}
			}
			// The wake itself is the work: `process_writes` re-flushes every
			// stream's `pending_in` right after `wait_for_data` returns.
			DriverCommand::FlushInbound(_sid) => {}
			DriverCommand::Export {
				out_len,
				label,
				context,
				reply,
			} => self.pending_exports.push_back((out_len, label, context, reply)),
			DriverCommand::StreamShutdown { sid, write, code } => self.pending_shutdowns.push_back((sid, write, code)),
			DriverCommand::Close { code, reason } => self.pending_close = Some((code, reason)),
		}
	}
}

/// Recompute keying material (RFC 5705) from the live BoringSSL session.
///
/// quiche exposes the underlying `boring::ssl::SslRef` via `impl AsMut<SslRef>
/// for Connection` (tokio-quiche enables `boringssl-boring-crate`). We call the
/// BoringSSL FFI directly because the safe wrapper takes the label as `&str`,
/// while callers may use raw (non-UTF-8) label bytes.
fn export_keying_material(qconn: &mut QuicheConnection, out_len: usize, label: &[u8], context: &[u8]) -> Option<Vec<u8>> {
	use foreign_types_shared::ForeignTypeRef as _;

	let ssl: &mut boring::ssl::SslRef = qconn.as_mut();
	let mut out = vec![0u8; out_len];
	// SAFETY: `ssl.as_ptr()` yields a valid `SSL*` for the borrow; `out` /
	// `label` / `context` are passed as (ptr, len) of valid slices and
	// BoringSSL does not retain them past the call.
	let rc = unsafe {
		boring_sys::SSL_export_keying_material(
			ssl.as_ptr(),
			out.as_mut_ptr(),
			out.len(),
			label.as_ptr() as *const core::ffi::c_char,
			label.len(),
			context.as_ptr(),
			context.len(),
			1, // use_context = true
		)
	};
	(rc == 1).then_some(out)
}

impl ApplicationOverQuic for BridgeDriver {
	fn on_conn_established(&mut self, qconn: &mut QuicheConnection, _info: &HandshakeInfo) -> QuicResult<()> {
		let span = self.span.clone();
		let _enter = span.enter();
		self.established = true;
		self.shared
			.max_dgram
			.store(qconn.dgram_max_writable_len().unwrap_or(0), Ordering::Relaxed);
		debug!(trace = qconn.trace_id(), "wind-quic quiche connection established");
		if let (Some(tx), Some(handle)) = (self.established_tx.take(), self.handle.take()) {
			let _ = tx.send(handle);
		}
		Ok(())
	}

	fn should_act(&self) -> bool {
		self.established
	}

	fn buffer(&mut self) -> &mut [u8] {
		&mut self.buffer
	}

	async fn wait_for_data(&mut self, _qconn: &mut QuicheConnection) -> QuicResult<()> {
		enum Ev {
			Out((u64, Option<Bytes>, mpsc::Receiver<Bytes>)),
			Cmd(DriverCommand),
		}
		let ev = {
			let waiters = &mut self.waiters;
			let cmd_rx = &mut self.cmd_rx;
			tokio::select! {
				// `biased`: drain commands before out-channel events. A `reset()`
				// enqueues a `StreamShutdown` command and then (on drop) closes the
				// out back-channel; the channel-close would otherwise be observed
				// first and emit a clean FIN that races the RESET. Command-first
				// ordering guarantees the reset is applied before the close.
				biased;
				Some(c) = cmd_rx.recv() => Ev::Cmd(c),
				Some(w) = waiters.next() => Ev::Out(w),
				// Keep the future pending (never resolving) when there is no app
				// event, rather than busy-looping. The worker still processes
				// inbound packets and timers in parallel.
				_ = std::future::pending::<()>() => unreachable!(),
			}
		};
		let span = self.span.clone();
		let _enter = span.enter();
		match ev {
			Ev::Out((sid, data, rx)) => match data {
				Some(b) => {
					// Buffer the chunk; re-arm the back-channel only while under
					// the outbound cap. At/over the cap we park `rx` (stop
					// draining) so the local writer back-pressures; `write_stream`
					// re-arms once the queue drains. If the stream is gone or its
					// send side has failed, drop both `b` and `rx` — dropping `rx`
					// closes the channel so the writer's next `poll_write` fails.
					let rearm = match self.streams.get_mut(&sid) {
						Some(st) if !st.send_failed => {
							st.out_queue_len += b.len();
							st.out_queue.push_back(b);
							if st.out_queue_len >= MAX_PENDING_OUT {
								st.parked_out_rx = Some(rx);
								None
							} else {
								Some(rx)
							}
						}
						_ => None,
					};
					if let Some(rx) = rearm {
						self.waiters.push(WaitOut::new(sid, rx));
					}
				}
				None => {
					if let Some(st) = self.streams.get_mut(&sid) {
						st.out_done = true;
					}
				}
			},
			Ev::Cmd(cmd) => self.handle_cmd(cmd),
		}
		Ok(())
	}

	fn process_reads(&mut self, qconn: &mut QuicheConnection) -> QuicResult<()> {
		let span = self.span.clone();
		let _enter = span.enter();

		loop {
			match qconn.dgram_recv(&mut self.buffer) {
				Ok(n) => {
					let _ = self.dgram_in_tx.send(Bytes::copy_from_slice(&self.buffer[..n]));
				}
				Err(quiche::Error::Done) => break,
				Err(e) => {
					trace!("dgram_recv error: {e}");
					break;
				}
			}
		}
		self.shared
			.max_dgram
			.store(qconn.dgram_max_writable_len().unwrap_or(0), Ordering::Relaxed);

		let ids: Vec<u64> = qconn.readable().collect();
		for sid in ids {
			if self.is_peer_initiated(sid) {
				self.ensure_incoming(sid);
			}
			if self.streams.contains_key(&sid) {
				self.read_stream(qconn, sid);
			}
		}
		Ok(())
	}

	fn process_writes(&mut self, qconn: &mut QuicheConnection) -> QuicResult<()> {
		let span = self.span.clone();
		let _enter = span.enter();

		// Connection close (terminal — do it first).
		if let Some((code, reason)) = self.pending_close.take() {
			let _ = qconn.close(true, code as u64, &reason);
		}

		while let Some((out_len, label, context, reply)) = self.pending_exports.pop_front() {
			let res = export_keying_material(qconn, out_len, &label, &context);
			let _ = reply.send(res);
		}

		// Cache cumulative wire byte counters into `shared` so handles read them
		// without a driver round-trip — and so the final counts survive close.
		let stats = qconn.stats();
		self.shared.sent_bytes.store(stats.sent_bytes, Ordering::Relaxed);
		self.shared.recv_bytes.store(stats.recv_bytes, Ordering::Relaxed);

		while let Some(op) = self.pending_opens.pop_front() {
			match op {
				PendingOpen::Bi(reply) => {
					let pair = self.open_local_bi();
					let _ = reply.send(pair);
				}
				PendingOpen::Uni(reply) => {
					let send = self.open_local_uni();
					let _ = reply.send(send);
				}
			}
		}

		while let Some((sid, write, code)) = self.pending_shutdowns.pop_front() {
			let dir = if write { Shutdown::Write } else { Shutdown::Read };
			let _ = qconn.stream_shutdown(sid, dir, code);
			// A write-direction reset supersedes any pending FIN. Mark the send
			// side finished/failed so `write_stream` doesn't later emit a clean
			// FIN (which would race the RESET and could surface as a clean EOF at
			// the peer).
			if write && let Some(st) = self.streams.get_mut(&sid) {
				st.out_queue.clear();
				st.out_queue_len = 0;
				st.out_done = true;
				st.fin_sent = true;
				st.send_failed = true;
			}
		}

		while let Some(dg) = self.out_datagrams.front() {
			match qconn.dgram_send(dg.as_ref()) {
				Ok(()) => {
					self.out_datagrams.pop_front();
				}
				Err(quiche::Error::Done) => break,
				Err(e) => {
					trace!("dgram_send error: {e}");
					self.out_datagrams.pop_front();
				}
			}
		}

		let sids: Vec<u64> = self.streams.keys().copied().collect();
		for sid in &sids {
			self.write_stream(qconn, *sid);
		}
		// Re-flush inbound in case the handle drained its recv channels.
		for sid in &sids {
			self.flush_inbound(*sid);
		}
		Ok(())
	}

	fn on_conn_close<M: Metrics>(&mut self, qconn: &mut QuicheConnection, _metrics: &M, _result: &QuicResult<()>) {
		// Capture the final byte counts *before* waking `closed()` waiters: the
		// traffic sampler's close-path read runs after this worker loop has
		// exited, so this cached snapshot is its only source for the last window.
		let stats = qconn.stats();
		self.shared.sent_bytes.store(stats.sent_bytes, Ordering::Relaxed);
		self.shared.recv_bytes.store(stats.recv_bytes, Ordering::Relaxed);
		self.shared.closed.store(true, Ordering::SeqCst);
		self.shared.closed_notify.notify_waiters();
	}
}
