//! Channel-backed stream halves bridging the synchronous quiche worker to the
//! async [`QuicSendStream`] / [`QuicRecvStream`] surface.
//!
//! The [`BridgeDriver`](crate::quiche::driver::BridgeDriver) worker owns the
//! `quiche::Connection` and can only touch streams synchronously from inside
//! its `ApplicationOverQuic` callbacks. These handles let application code hold
//! an owned async stream: the recv half is fed by the worker (data the peer
//! sent), and the send half drains to the worker (data to send to the peer).
//!
//! Generalized from `wind-tuiche`'s `QuicheStream`, split into independent send
//! and recv halves so it fits the `(SendStream, RecvStream)` shape of the
//! [`QuicConnection`](crate::traits::QuicConnection) trait.

use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use bytes::Bytes;
use tokio::{
	io::{AsyncRead, AsyncWrite, ReadBuf},
	sync::mpsc,
};
use tokio_util::sync::PollSender;

use crate::{
	error::QuicError,
	quiche::driver::{CmdTx, DriverCommand, InboundItem},
	traits::{QuicRecvStream, QuicSendStream},
};

/// Largest chunk handed to the worker per `poll_write`.
const WRITE_CHUNK: usize = 16 * 1024;

/// Send half of a quiche stream.
pub struct QuicheSend {
	sid: u64,
	cmd_tx: CmdTx,
	/// Target → peer payload, drained by the worker for `stream_send`.
	tx: PollSender<Bytes>,
	finished: bool,
}

impl QuicheSend {
	pub(crate) fn new(sid: u64, cmd_tx: CmdTx, tx: mpsc::Sender<Bytes>) -> Self {
		Self {
			sid,
			cmd_tx,
			tx: PollSender::new(tx),
			finished: false,
		}
	}
}

impl AsyncWrite for QuicheSend {
	fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, data: &[u8]) -> Poll<io::Result<usize>> {
		match self.tx.poll_reserve(cx) {
			Poll::Ready(Ok(())) => {
				let n = data.len().min(WRITE_CHUNK);
				match self.tx.send_item(Bytes::copy_from_slice(&data[..n])) {
					Ok(()) => Poll::Ready(Ok(n)),
					Err(_) => Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "quic stream closed"))),
				}
			}
			Poll::Ready(Err(_)) => Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "quic stream closed"))),
			Poll::Pending => Poll::Pending,
		}
	}

	fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Poll::Ready(Ok(()))
	}

	fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		// Closing the sender makes the worker observe end-of-data on this
		// stream's back-channel and emit a FIN on the QUIC stream.
		self.tx.close();
		self.finished = true;
		Poll::Ready(Ok(()))
	}
}

impl QuicSendStream for QuicheSend {
	fn finish(&mut self) -> Result<(), QuicError> {
		// Closing the channel signals the worker to flush a FIN. Idempotent.
		self.tx.close();
		self.finished = true;
		Ok(())
	}

	fn reset(&mut self, code: u64) {
		// Best-effort: ask the worker to reset the stream's send side, then drop
		// the back-channel so no further data is queued.
		let _ = self.cmd_tx.send(DriverCommand::StreamShutdown {
			sid: self.sid,
			write: true,
			code,
		});
		self.tx.close();
		self.finished = true;
	}

	fn id(&self) -> u64 {
		self.sid
	}
}

/// Recv half of a quiche stream.
pub struct QuicheRecv {
	sid: u64,
	cmd_tx: CmdTx,
	/// Peer → application payload, fed by the worker. An `Err(code)` item is
	/// the peer's RESET_STREAM (surfaced as an I/O error); `None` from the
	/// channel (sender dropped) signals the peer's FIN → clean EOF.
	rx: mpsc::Receiver<InboundItem>,
	leftover: Bytes,
}

impl QuicheRecv {
	pub(crate) fn new(sid: u64, cmd_tx: CmdTx, rx: mpsc::Receiver<InboundItem>) -> Self {
		Self {
			sid,
			cmd_tx,
			rx,
			leftover: Bytes::new(),
		}
	}
}

impl AsyncRead for QuicheRecv {
	fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
		if self.leftover.is_empty() {
			// If the channel was full, the driver may be holding overflow in its
			// `pending_in` (and possibly a queued FIN). Draining it here frees a
			// slot, so nudge the driver to re-flush — otherwise that data could
			// stall until some other event happened to wake the worker (e.g. a
			// pure-upload stream with no reverse traffic).
			let was_full = self.rx.capacity() == 0;
			match self.rx.poll_recv(cx) {
				Poll::Ready(Some(Ok(b))) => {
					self.leftover = b;
					if was_full {
						let _ = self.cmd_tx.send(DriverCommand::FlushInbound(self.sid));
					}
				}
				// Peer reset the stream → surface an error, not a truncated-but-clean
				// EOF, so callers can tell a complete stream from an aborted one.
				Poll::Ready(Some(Err(code))) => {
					return Poll::Ready(Err(io::Error::new(
						io::ErrorKind::ConnectionReset,
						format!("quic stream reset by peer (code {code})"),
					)));
				}
				// Sender dropped → peer finished sending → clean EOF.
				Poll::Ready(None) => return Poll::Ready(Ok(())),
				Poll::Pending => return Poll::Pending,
			}
		}
		let n = self.leftover.len().min(buf.remaining());
		let chunk = self.leftover.split_to(n);
		buf.put_slice(&chunk);
		Poll::Ready(Ok(()))
	}
}

impl QuicRecvStream for QuicheRecv {
	fn stop(&mut self, code: u64) {
		let _ = self.cmd_tx.send(DriverCommand::StreamShutdown {
			sid: self.sid,
			write: false,
			code,
		});
	}

	fn id(&self) -> u64 {
		self.sid
	}
}
