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
		// previous buffer has drained.
		self.pending = Some(data.into());
		Ok(())
	}

	fn poll_finish(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), StreamErrorIncoming>> {
		match self.poll_flush_pending(cx) {
			Poll::Ready(Ok(())) => match Pin::new(&mut self.inner).poll_flush(cx) {
				Poll::Ready(Ok(())) => {
					let _ = self.inner.finish();
					Poll::Ready(Ok(()))
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
	// QUIC stream ids fit the h3 `StreamId` invariant (< 2^62); fall back to 0
	// only if a backend ever surfaces something out of range.
	StreamId::try_from(id).unwrap_or_else(|_| StreamId::try_from(0).expect("0 is a valid stream id"))
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
