//! A recv stream that replays a small buffered prefix before its inner stream.
//!
//! The TUIC server multiplexes the real TUIC protocol and an HTTP/3 masquerade
//! on the same QUIC port (both negotiate the `h3` ALPN). To classify a
//! connection it peeks the first byte(s) of the first stream; [`PrefixedRecv`]
//! then lets that consumed prefix be re-read transparently, whether the stream
//! is handed to the TUIC header parser or to the HTTP/3 adapter.

use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use bytes::Bytes;
use tokio::io::{AsyncRead, ReadBuf};

use crate::traits::QuicRecvStream;

/// Wraps a recv stream so a buffered `prefix` is yielded before the inner
/// stream's own data. Once the prefix is drained, reads delegate straight to
/// the inner stream.
pub struct PrefixedRecv<R> {
	prefix: Bytes,
	inner: R,
}

impl<R> PrefixedRecv<R> {
	/// Build a `PrefixedRecv` that replays `prefix`, then `inner`.
	pub fn new(prefix: impl Into<Bytes>, inner: R) -> Self {
		Self {
			prefix: prefix.into(),
			inner,
		}
	}

	/// Consume the wrapper, returning the inner stream. The (possibly partial)
	/// unread prefix is returned alongside so callers don't silently drop it.
	pub fn into_parts(self) -> (Bytes, R) {
		(self.prefix, self.inner)
	}
}

impl<R: AsyncRead + Unpin> AsyncRead for PrefixedRecv<R> {
	fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
		let this = self.get_mut();
		if !this.prefix.is_empty() {
			let n = this.prefix.len().min(buf.remaining());
			if n > 0 {
				let chunk = this.prefix.split_to(n);
				buf.put_slice(&chunk);
			}
			return Poll::Ready(Ok(()));
		}
		Pin::new(&mut this.inner).poll_read(cx, buf)
	}
}

impl<R: QuicRecvStream> QuicRecvStream for PrefixedRecv<R> {
	fn stop(&mut self, code: u64) {
		self.inner.stop(code);
	}

	fn id(&self) -> u64 {
		self.inner.id()
	}
}
