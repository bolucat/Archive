//! Byte-counting stream wrapper for SOCKS5 traffic accounting.
//!
//! TUIC samples the QUIC connection's own byte counters (see the TUIC server
//! stats sampler), but SOCKS5 is plain TCP with no connection-level counters,
//! so we count at the IO point: this wrapper increments the shared
//! [`StatsCollector`] on every read/write. `poll_flush`/`poll_shutdown` are
//! forwarded verbatim so half-close semantics (relied on by
//! `copy_bidirectional`) are preserved.

use std::{
	io,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::{StatsCollector, UserId};

pin_project_lite::pin_project! {
	/// Wraps a duplex stream, recording bytes read as upload (client→proxy) and
	/// bytes written as download (proxy→client) for `user`.
	pub struct CountingStream<S> {
		#[pin]
		inner: S,
		stats: Arc<StatsCollector>,
		user: UserId,
	}
}

impl<S> CountingStream<S> {
	pub fn new(inner: S, stats: Arc<StatsCollector>, user: UserId) -> Self {
		Self { inner, stats, user }
	}
}

impl<S: AsyncRead> AsyncRead for CountingStream<S> {
	fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
		let this = self.project();
		let before = buf.filled().len();
		let res = this.inner.poll_read(cx, buf);
		if let Poll::Ready(Ok(())) = &res {
			let n = buf.filled().len() - before;
			if n > 0 {
				this.stats.record_upload(this.user, n as u64);
			}
		}
		res
	}
}

impl<S: AsyncWrite> AsyncWrite for CountingStream<S> {
	fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
		let this = self.project();
		let res = this.inner.poll_write(cx, buf);
		if let Poll::Ready(Ok(n)) = &res
			&& *n > 0
		{
			this.stats.record_download(this.user, *n as u64);
		}
		res
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		self.project().inner.poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		self.project().inner.poll_shutdown(cx)
	}

	fn poll_write_vectored(self: Pin<&mut Self>, cx: &mut Context<'_>, bufs: &[io::IoSlice<'_>]) -> Poll<io::Result<usize>> {
		let this = self.project();
		let res = this.inner.poll_write_vectored(cx, bufs);
		if let Poll::Ready(Ok(n)) = &res
			&& *n > 0
		{
			this.stats.record_download(this.user, *n as u64);
		}
		res
	}

	fn is_write_vectored(&self) -> bool {
		self.inner.is_write_vectored()
	}
}

#[cfg(test)]
mod tests {
	use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

	use super::*;

	#[tokio::test]
	async fn counts_reads_as_upload_and_writes_as_download() {
		let (client, server) = tokio::io::duplex(1024);
		let stats = Arc::new(StatsCollector::new());
		let user = UserId::from("u1");
		let mut counted = CountingStream::new(server, stats.clone(), user.clone());

		// Peer writes 5 bytes → our wrapper reads them → counted as upload.
		let (mut c_rd, mut c_wr) = tokio::io::split(client);
		c_wr.write_all(b"hello").await.unwrap();
		let mut buf = [0u8; 5];
		counted.read_exact(&mut buf).await.unwrap();

		// Our wrapper writes 3 bytes → peer reads them → counted as download.
		counted.write_all(b"abc").await.unwrap();
		counted.flush().await.unwrap();
		let mut rbuf = [0u8; 3];
		c_rd.read_exact(&mut rbuf).await.unwrap();

		let s = stats.snapshot_user(&user).unwrap();
		assert_eq!(s.upload, 5);
		assert_eq!(s.download, 3);
	}
}
