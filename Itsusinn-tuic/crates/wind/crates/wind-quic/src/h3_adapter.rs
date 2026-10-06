//! An [`h3::quic`] server surface implemented over [`QuicConnection`].
//!
//! The TUIC server poses as a real HTTP/3 web server for clients that speak
//! actual HTTP/3 instead of TUIC (see the masquerade in `wind-tuic`). Rather
//! than bind to a specific QUIC engine, this adapter implements the hyperium
//! [`h3`] crate's transport traits over our backend-neutral
//! [`QuicConnection`], so the same HTTP/3 server runs over either the quinn or
//! quiche backend.
//!
//! Only the **server** surface is implemented: accepting peer-initiated uni
//! (control / QPACK) and bidi (request) streams, and opening our own uni
//! streams (control / QPACK). The per-stream classifier in `wind-tuic` reads
//! each accepted stream's prefix, and feeds the ones it classified as h3 into
//! this adapter over two channels (`recv_rx` / `bidi_rx`) — already accepted
//! off the connection, with their peeked prefix replayed via
//! [`PrefixedRecv`](crate::PrefixedRecv). So the adapter just pulls streams
//! from the channels; there is no "first stream" special case. Every recv
//! stream is a `PrefixedRecv`, so the adapter is generic over the backend's
//! concrete stream types — no boxing or dynamic dispatch.
//!
//! The bridge is mechanical: our streams are `AsyncRead`/`AsyncWrite`, while
//! `h3::quic` is poll- and `Buf`-based. Recv streams read into a scratch buffer
//! and hand back `Bytes`; send streams buffer one `WriteBuf` and drain it
//! through `poll_write`. Our `QuicConnection` accept/open methods are `async
//! fn`, so each is driven as a boxed in-flight future stored on the
//! connection/opener.

use std::{
	future::Future,
	pin::Pin,
	task::{Context, Poll},
};

use bytes::{Buf, Bytes};
use h3::quic::{
	BidiStream, Connection, ConnectionErrorIncoming, OpenStreams, RecvStream, SendStream, StreamErrorIncoming, StreamId,
	WriteBuf,
};
use tokio::{
	io::{AsyncRead, AsyncWrite, ReadBuf},
	sync::mpsc,
};

use crate::{PrefixedRecv, QuicConnection, QuicError, QuicRecvStream, QuicSendStream};

/// Scratch buffer size for a single `poll_data` read.
const RECV_CHUNK: usize = 16 * 1024;

type BoxFut<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// A boxed in-flight `open_bi` future. Aliased because the bidi case returns a
/// `(SendStream, RecvStream)` tuple, which trips clippy's `type_complexity`
/// lint when written inline in every slot/signature.
type BoxBiFut<C> = BoxFut<Result<(<C as QuicConnection>::SendStream, <C as QuicConnection>::RecvStream), QuicError>>;

/// Channel of accepted (prefix-replayed) recv streams the `wind-tuic`
/// per-stream router feeds to the masquerade h3 server.
type RecvRx<C> = mpsc::UnboundedReceiver<PrefixedRecv<<C as QuicConnection>::RecvStream>>;
/// Channel of accepted bidi (request) streams — `(send half, prefix-replayed
/// recv)`.
type BidiRx<C> = mpsc::UnboundedReceiver<(
	<C as QuicConnection>::SendStream,
	PrefixedRecv<<C as QuicConnection>::RecvStream>,
)>;

/// Build an HTTP/3 server connection over `conn`.
///
/// Accepted streams are pulled from `recv_rx` / `bidi_rx`, which the
/// `wind-tuic` per-stream router fills with the streams it classified as h3
/// (already accepted off the connection, with their peeked prefix replayed via
/// [`PrefixedRecv`]). `conn` is kept only for *opening* the server's own
/// control / QPACK streams. There is no "first stream" special case — every
/// accepted stream arrives the same way.
pub fn server_connection<C: QuicConnection>(conn: C, recv_rx: RecvRx<C>, bidi_rx: BidiRx<C>) -> H3Conn<C> {
	H3Conn {
		conn,
		recv_rx,
		bidi_rx,
		open_uni_fut: None,
		open_bi_fut: None,
	}
}

fn conn_err(e: QuicError) -> ConnectionErrorIncoming {
	match e {
		QuicError::TimedOut => ConnectionErrorIncoming::Timeout,
		QuicError::ApplicationClosed { .. } | QuicError::LocallyClosed => {
			ConnectionErrorIncoming::ApplicationClose { error_code: 0 }
		}
		other => ConnectionErrorIncoming::InternalError(other.to_string()),
	}
}

fn stream_err(e: QuicError) -> StreamErrorIncoming {
	StreamErrorIncoming::ConnectionErrorIncoming {
		connection_error: conn_err(e),
	}
}

/// [`PrefixedRecv`] doubles as the adapter's `h3::quic::RecvStream`: it already
/// owns the (possibly empty) replayed prefix plus the backend recv stream, so
/// no separate wrapper type is needed. Fresh accepted streams are wrapped with
/// an empty prefix.
impl<R: QuicRecvStream> RecvStream for PrefixedRecv<R> {
	type Buf = Bytes;

	fn poll_data(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<Self::Buf>, StreamErrorIncoming>> {
		let mut scratch = [0u8; RECV_CHUNK];
		let mut rb = ReadBuf::new(&mut scratch);
		match Pin::new(&mut *self).poll_read(cx, &mut rb) {
			Poll::Ready(Ok(())) => {
				let filled = rb.filled();
				if filled.is_empty() {
					// Clean EOF (peer FIN).
					Poll::Ready(Ok(None))
				} else {
					Poll::Ready(Ok(Some(Bytes::copy_from_slice(filled))))
				}
			}
			Poll::Ready(Err(e)) => Poll::Ready(Err(StreamErrorIncoming::Unknown(Box::new(e)))),
			Poll::Pending => Poll::Pending,
		}
	}

	fn stop_sending(&mut self, error_code: u64) {
		self.stop(error_code);
	}

	fn recv_id(&self) -> StreamId {
		stream_id(self.id())
	}
}

/// `h3::quic::SendStream` over the backend's send stream.
pub struct H3Send<C: QuicConnection> {
	inner: C::SendStream,
	id: u64,
	/// At most one `WriteBuf` is buffered at a time; `poll_ready`/`poll_finish`
	/// drain it through the underlying `AsyncWrite`.
	pending: Option<WriteBuf<Bytes>>,
}

impl<C: QuicConnection> H3Send<C> {
	fn new(inner: C::SendStream) -> Self {
		let id = inner.id();
		Self {
			inner,
			id,
			pending: None,
		}
	}

	/// Flush the buffered `WriteBuf` (if any) to the underlying stream. Returns
	/// `Ready(Ok)` once nothing is buffered.
	fn poll_flush_pending(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
		if let Some(buf) = self.pending.as_mut() {
			while buf.has_remaining() {
				let chunk = buf.chunk();
				match Pin::new(&mut self.inner).poll_write(cx, chunk) {
					Poll::Ready(Ok(0)) => {
						return Poll::Ready(Err(StreamErrorIncoming::Unknown(Box::new(std::io::Error::new(
							std::io::ErrorKind::WriteZero,
							"h3 send stream wrote zero bytes",
						)))));
					}
					Poll::Ready(Ok(n)) => buf.advance(n),
					Poll::Ready(Err(e)) => return Poll::Ready(Err(StreamErrorIncoming::Unknown(Box::new(e)))),
					Poll::Pending => return Poll::Pending,
				}
			}
		}
		self.pending = None;
		Poll::Ready(Ok(()))
	}
}

impl<C: QuicConnection> SendStream<Bytes> for H3Send<C> {
	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
		self.poll_flush_pending(cx)
	}

	fn send_data<T: Into<WriteBuf<Bytes>>>(&mut self, data: T) -> Result<(), StreamErrorIncoming> {
		// h3 always polls `poll_ready` to readiness before `send_data`, so the
		// previous buffer has drained. Should it ever call this while a frame
		// is still buffered, overwriting `pending` would discard that
		// frame and hand h3 a response it believes was sent in full;
		// report the misuse instead, as `h3-quinn` does. h3 turns this
		// into H3_INTERNAL_ERROR.
		if self.pending.is_some() {
			return Err(StreamErrorIncoming::ConnectionErrorIncoming {
				connection_error: ConnectionErrorIncoming::InternalError(
					"send_data called while the h3 send stream is not ready".to_owned(),
				),
			});
		}
		self.pending = Some(data.into());
		Ok(())
	}

	fn poll_finish(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
		match self.poll_flush_pending(cx) {
			Poll::Ready(Ok(())) => match Pin::new(&mut self.inner).poll_flush(cx) {
				Poll::Ready(Ok(())) => {
					// The backend reports a FIN it could not queue (e.g. an
					// already-reset stream) through this `Result`. Discarding
					// it told h3 the stream ended cleanly; propagate it, as
					// `h3-quinn` does.
					Poll::Ready(self.inner.finish().map_err(|e| StreamErrorIncoming::Unknown(Box::new(e))))
				}
				Poll::Ready(Err(e)) => Poll::Ready(Err(StreamErrorIncoming::Unknown(Box::new(e)))),
				Poll::Pending => Poll::Pending,
			},
			other => other,
		}
	}

	fn reset(&mut self, reset_code: u64) {
		self.inner.reset(reset_code);
	}

	fn send_id(&self) -> StreamId {
		stream_id(self.id)
	}
}

/// `h3::quic::BidiStream` joining an [`H3Send`] and a [`PrefixedRecv`].
pub struct H3Bidi<C: QuicConnection> {
	send: H3Send<C>,
	recv: PrefixedRecv<C::RecvStream>,
}

impl<C: QuicConnection> SendStream<Bytes> for H3Bidi<C> {
	fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
		self.send.poll_ready(cx)
	}

	fn send_data<T: Into<WriteBuf<Bytes>>>(&mut self, data: T) -> Result<(), StreamErrorIncoming> {
		self.send.send_data(data)
	}

	fn poll_finish(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
		self.send.poll_finish(cx)
	}

	fn reset(&mut self, reset_code: u64) {
		self.send.reset(reset_code);
	}

	fn send_id(&self) -> StreamId {
		self.send.send_id()
	}
}

impl<C: QuicConnection> RecvStream for H3Bidi<C> {
	type Buf = Bytes;

	fn poll_data(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<Self::Buf>, StreamErrorIncoming>> {
		self.recv.poll_data(cx)
	}

	fn stop_sending(&mut self, error_code: u64) {
		self.recv.stop_sending(error_code);
	}

	fn recv_id(&self) -> StreamId {
		self.recv.recv_id()
	}
}

impl<C: QuicConnection> BidiStream<Bytes> for H3Bidi<C> {
	type RecvStream = PrefixedRecv<C::RecvStream>;
	type SendStream = H3Send<C>;

	fn split(self) -> (Self::SendStream, Self::RecvStream) {
		(self.send, self.recv)
	}
}

fn into_bidi<C: QuicConnection>((send, recv): (C::SendStream, C::RecvStream)) -> H3Bidi<C> {
	H3Bidi {
		send: H3Send::new(send),
		recv: PrefixedRecv::new(Bytes::new(), recv),
	}
}

/// Opens local uni/bidi streams (HTTP/3 control + QPACK streams). Produced by
/// [`Connection::opener`].
pub struct H3Opener<C: QuicConnection> {
	conn: C,
	open_uni_fut: Option<BoxFut<Result<C::SendStream, QuicError>>>,
	open_bi_fut: Option<BoxBiFut<C>>,
}

impl<C: QuicConnection> OpenStreams<Bytes> for H3Opener<C> {
	type BidiStream = H3Bidi<C>;
	type SendStream = H3Send<C>;

	fn poll_open_bidi(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::BidiStream, StreamErrorIncoming>> {
		poll_open_bidi(&self.conn, &mut self.open_bi_fut, cx)
	}

	fn poll_open_send(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::SendStream, StreamErrorIncoming>> {
		poll_open_send(&self.conn, &mut self.open_uni_fut, cx)
	}

	fn close(&mut self, code: h3::error::Code, reason: &[u8]) {
		self.conn.close(h3_code_to_u32(code), reason);
	}
}

/// `h3::quic::Connection` over a [`QuicConnection`] handle. Accepts streams
/// from the router's channels; opens streams directly on `conn`.
pub struct H3Conn<C: QuicConnection> {
	conn: C,
	recv_rx: RecvRx<C>,
	bidi_rx: BidiRx<C>,
	open_uni_fut: Option<BoxFut<Result<C::SendStream, QuicError>>>,
	open_bi_fut: Option<BoxBiFut<C>>,
}

impl<C: QuicConnection> OpenStreams<Bytes> for H3Conn<C> {
	type BidiStream = H3Bidi<C>;
	type SendStream = H3Send<C>;

	fn poll_open_bidi(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::BidiStream, StreamErrorIncoming>> {
		poll_open_bidi(&self.conn, &mut self.open_bi_fut, cx)
	}

	fn poll_open_send(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::SendStream, StreamErrorIncoming>> {
		poll_open_send(&self.conn, &mut self.open_uni_fut, cx)
	}

	fn close(&mut self, code: h3::error::Code, reason: &[u8]) {
		self.conn.close(h3_code_to_u32(code), reason);
	}
}

/// Map an h3 application error code onto the `u32` close code the QUIC handle
/// accepts, saturating rather than silently discarding the code as 0 (which
/// erased the protocol-level reason the peer saw).
fn h3_code_to_u32(code: h3::error::Code) -> u32 {
	u32::try_from(code.value()).unwrap_or(u32::MAX)
}

impl<C: QuicConnection> Connection<Bytes> for H3Conn<C> {
	type OpenStreams = H3Opener<C>;
	type RecvStream = PrefixedRecv<C::RecvStream>;

	fn poll_accept_recv(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::RecvStream, ConnectionErrorIncoming>> {
		match self.recv_rx.poll_recv(cx) {
			Poll::Ready(Some(recv)) => Poll::Ready(Ok(recv)),
			// Channel closed → the router (and the connection) is gone.
			Poll::Ready(None) => Poll::Ready(Err(ConnectionErrorIncoming::Timeout)),
			Poll::Pending => Poll::Pending,
		}
	}

	fn poll_accept_bidi(&mut self, cx: &mut Context<'_>) -> Poll<Result<Self::BidiStream, ConnectionErrorIncoming>> {
		match self.bidi_rx.poll_recv(cx) {
			Poll::Ready(Some((send, recv))) => Poll::Ready(Ok(H3Bidi {
				send: H3Send::new(send),
				recv,
			})),
			Poll::Ready(None) => Poll::Ready(Err(ConnectionErrorIncoming::Timeout)),
			Poll::Pending => Poll::Pending,
		}
	}

	fn opener(&self) -> Self::OpenStreams {
		H3Opener {
			conn: self.conn.clone(),
			open_uni_fut: None,
			open_bi_fut: None,
		}
	}
}

fn stream_id(id: u64) -> StreamId {
	// QUIC encodes stream ids as varints, so both backends only ever surface
	// ids that satisfy the h3 `StreamId` invariant (< 2^62) and this conversion
	// cannot fail. Should one ever report something larger, substituting 0
	// would hand h3 the id of the peer's *first bidi request stream* — a
	// plausible, valid id that h3 then records in its `ongoing_streams` set and
	// in the resolved request — so the wrong stream would be described silently
	// instead of loudly. Fail instead, as `h3-quinn` does.
	StreamId::try_from(id).unwrap_or_else(|_| panic!("backend stream id {id:#x} does not fit the h3 stream id range"))
}

fn poll_open_send<C: QuicConnection>(
	conn: &C,
	slot: &mut Option<BoxFut<Result<C::SendStream, QuicError>>>,
	cx: &mut Context<'_>,
) -> Poll<Result<H3Send<C>, StreamErrorIncoming>> {
	let poll = {
		let fut = slot.get_or_insert_with(|| {
			let conn = conn.clone();
			Box::pin(async move { conn.open_uni().await })
		});
		fut.as_mut().poll(cx)
	};
	match poll {
		Poll::Ready(res) => {
			*slot = None;
			Poll::Ready(res.map(H3Send::new).map_err(stream_err))
		}
		Poll::Pending => Poll::Pending,
	}
}

fn poll_open_bidi<C: QuicConnection>(
	conn: &C,
	slot: &mut Option<BoxBiFut<C>>,
	cx: &mut Context<'_>,
) -> Poll<Result<H3Bidi<C>, StreamErrorIncoming>> {
	let poll = {
		let fut = slot.get_or_insert_with(|| {
			let conn = conn.clone();
			Box::pin(async move { conn.open_bi().await })
		});
		fut.as_mut().poll(cx)
	};
	match poll {
		Poll::Ready(res) => {
			*slot = None;
			Poll::Ready(res.map(into_bidi).map_err(stream_err))
		}
		Poll::Pending => Poll::Pending,
	}
}

#[cfg(test)]
mod tests {
	use std::{
		future, io,
		sync::{
			Arc,
			atomic::{AtomicUsize, Ordering},
		},
		task::Waker,
	};

	use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

	use super::*;
	use crate::{QuicConnection, QuicError, QuicRecvStream, QuicSendStream};

	/// A send half the test drives directly: it counts the FINs handed to it
	/// and can be told to fail `finish()`, so the adapter's finish path can
	/// be exercised without a live QUIC connection. `h3-quinn` maps a
	/// failing `quinn::SendStream::finish()` (already finished or reset) to
	/// `StreamErrorIncoming::Unknown`; the same shape is reproduced here.
	struct FakeSend {
		id: u64,
		finishes: Arc<AtomicUsize>,
		fail_finish: bool,
	}

	impl FakeSend {
		fn new(fail_finish: bool) -> Self {
			Self {
				id: 4,
				finishes: Arc::new(AtomicUsize::new(0)),
				fail_finish,
			}
		}
	}

	impl AsyncWrite for FakeSend {
		fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
			Poll::Ready(Ok(buf.len()))
		}

		fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
			Poll::Ready(Ok(()))
		}

		fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
			Poll::Ready(Ok(()))
		}
	}

	impl QuicSendStream for FakeSend {
		fn finish(&mut self) -> Result<(), QuicError> {
			self.finishes.fetch_add(1, Ordering::SeqCst);
			if self.fail_finish {
				return Err(QuicError::ConnectionLost("the fake stream was already reset".to_owned()));
			}
			Ok(())
		}

		fn reset(&mut self, _code: u64) {}

		fn id(&self) -> u64 {
			self.id
		}
	}

	/// A recv half that is never read by these tests.
	struct FakeRecv;

	impl AsyncRead for FakeRecv {
		fn poll_read(self: Pin<&mut Self>, _cx: &mut Context<'_>, _buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
			Poll::Ready(Ok(()))
		}
	}

	impl QuicRecvStream for FakeRecv {
		fn stop(&mut self, _code: u64) {}

		fn id(&self) -> u64 {
			0
		}
	}

	/// The smallest `QuicConnection` that can hold a [`FakeSend`]; every method
	/// other than the ones under test is inert.
	#[derive(Clone)]
	struct FakeConn;

	impl QuicConnection for FakeConn {
		type RecvStream = FakeRecv;
		type SendStream = FakeSend;

		async fn open_bi(&self) -> Result<(Self::SendStream, Self::RecvStream), QuicError> {
			future::pending().await
		}

		async fn accept_bi(&self) -> Result<(Self::SendStream, Self::RecvStream), QuicError> {
			future::pending().await
		}

		async fn open_uni(&self) -> Result<Self::SendStream, QuicError> {
			future::pending().await
		}

		async fn accept_uni(&self) -> Result<Self::RecvStream, QuicError> {
			future::pending().await
		}

		fn send_datagram(&self, _data: Bytes) -> Result<(), QuicError> {
			Ok(())
		}

		async fn read_datagram(&self) -> Result<Bytes, QuicError> {
			future::pending().await
		}

		fn max_datagram_size(&self) -> Option<usize> {
			None
		}

		async fn export_keying_material(&self, _out: &mut [u8], _label: &[u8], _context: &[u8]) -> Result<(), QuicError> {
			Ok(())
		}

		fn close(&self, _code: u32, _reason: &[u8]) {}

		async fn closed(&self) {
			future::pending::<()>().await;
		}
	}

	fn poll_finish(send: &mut H3Send<FakeConn>) -> Poll<Result<(), StreamErrorIncoming>> {
		let mut cx = Context::from_waker(Waker::noop());
		SendStream::poll_finish(send, &mut cx)
	}

	/// A backend that refuses the FIN must not be reported to h3 as a clean
	/// finish: h3 only learns the stream did not end through this error, and
	/// otherwise records the response as fully delivered.
	#[test]
	fn a_failed_stream_finish_is_not_reported_as_a_clean_finish() {
		let mut send = H3Send::<FakeConn>::new(FakeSend::new(true));

		match poll_finish(&mut send) {
			Poll::Ready(Err(StreamErrorIncoming::Unknown(err))) => {
				// The backend's own error must survive, not a placeholder.
				assert!(
					err.to_string().contains("the fake stream was already reset"),
					"unexpected error text: {err}"
				);
			}
			other => panic!("a failed finish must surface as a stream error, got {other:?}"),
		}
	}

	/// The positive control for the test above: an accepted finish is still
	/// reported as clean, and the FIN is handed to the backend exactly once.
	#[test]
	fn an_accepted_stream_finish_is_reported_as_clean() {
		let stream = FakeSend::new(false);
		let finishes = Arc::clone(&stream.finishes);
		let mut send = H3Send::<FakeConn>::new(stream);

		assert!(matches!(poll_finish(&mut send), Poll::Ready(Ok(()))));
		assert_eq!(finishes.load(Ordering::SeqCst), 1, "the FIN must reach the backend");
	}

	/// The positive control for the test below: every id a QUIC varint can
	/// carry — including 0, the peer's first bidi request stream, and the
	/// largest encodable id — is reported to h3 unchanged.
	#[test]
	fn in_range_backend_stream_ids_are_reported_unchanged() {
		for id in [0, 4, 8, u32::MAX as u64, (1u64 << 62) - 1] {
			assert_eq!(stream_id(id).into_inner(), id, "stream id {id} must survive the conversion");
		}
	}

	/// An id a QUIC varint cannot encode is not a stream at all. Reporting it
	/// as 0 would hand h3 the id of the peer's *first bidi request stream* — an
	/// id it records in `ongoing_streams` and in the resolved request — so the
	/// adapter must fail loudly instead of describing the wrong stream.
	#[test]
	#[should_panic(expected = "does not fit the h3 stream id range")]
	fn an_out_of_range_backend_stream_id_is_not_reported_as_stream_zero() {
		let _ = stream_id(1u64 << 62);
	}
}
