use std::{future::pending, time::Duration};

use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Per-direction copy buffer size. 64 KiB keeps enough bytes in flight over
/// high bandwidth-delay-product links (a QUIC tunnel to a distant peer).
const BUFFER_SIZE: usize = 64 * 1024;

/// Bidirectionally relay bytes between two duplex streams until BOTH sides
/// have closed.
pub async fn copy_io<A, B>(a: &mut A, b: &mut B) -> (usize, usize, Option<std::io::Error>)
where
	A: AsyncRead + AsyncWrite + Unpin + ?Sized,
	B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
	copy_bidirectional(a, b, Duration::ZERO).await
}

/// Bidirectional relay with a half-close idle reaper.
pub async fn copy_bidirectional<A, B>(
	a: &mut A,
	b: &mut B,
	half_close_timeout: Duration,
) -> (usize, usize, Option<std::io::Error>)
where
	A: AsyncRead + AsyncWrite + Unpin + ?Sized,
	B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
	let mut a2b = BytesMut::with_capacity(BUFFER_SIZE);
	let mut b2a = BytesMut::with_capacity(BUFFER_SIZE);

	let mut a2b_num = 0;
	let mut b2a_num = 0;

	let mut a_eof = false;
	let mut b_eof = false;

	let mut last_err = None;

	let reaper_enabled = !half_close_timeout.is_zero();

	loop {
		let half_closed = a_eof ^ b_eof;
		let reaper = async {
			if reaper_enabled && half_closed {
				tokio::time::sleep(half_close_timeout).await;
			} else {
				pending::<()>().await;
			}
		};

		tokio::select! {
			a2b_res = a.read_buf(&mut a2b), if !a_eof => match a2b_res {
				Ok(0) => {
					a_eof = true;
					if let Err(err) = b.shutdown().await {
						last_err = Some(err);
					}
					if b_eof {
						break;
					}
				}
				Ok(num) => {
					a2b_num += num;
					if let Err(err) = b.write_all(&a2b[..num]).await {
						last_err = Some(err);
						break;
					}
					a2b.clear();
				}
				Err(err) => {
					last_err = Some(err);
					break;
				}
			},
			b2a_res = b.read_buf(&mut b2a), if !b_eof => match b2a_res {
				Ok(0) => {
					b_eof = true;
					if let Err(err) = a.shutdown().await {
						last_err = Some(err);
					}
					if a_eof {
						break;
					}
				}
				Ok(num) => {
					b2a_num += num;
					if let Err(err) = a.write_all(&b2a[..num]).await {
						last_err = Some(err);
						break;
					}
					b2a.clear();
				}
				Err(err) => {
					last_err = Some(err);
					break;
				}
			},
			// Half-open and idle past the timeout: stop waiting for the peer
			// that will never close so the still-open socket is released
			// instead of lingering in CLOSE_WAIT for the lifetime of the
			// parent connection.
			() = reaper => break,
		}
	}

	(a2b_num, b2a_num, last_err)
}

#[cfg(feature = "quic")]
pub mod quinn {
	use std::{
		io,
		pin::Pin,
		task::{Context, Poll},
	};

	use quinn::{RecvStream, SendStream};
	use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

	pub struct QuinnCompat {
		send: SendStream,
		recv: RecvStream,
	}

	impl QuinnCompat {
		pub fn new(send_stream: SendStream, recv_stream: RecvStream) -> Self {
			QuinnCompat {
				send: send_stream,
				recv: recv_stream,
			}
		}

		pub fn send_stream(&self) -> &SendStream {
			&self.send
		}

		pub fn recv_stream(&self) -> &RecvStream {
			&self.recv
		}

		pub fn send_stream_mut(&mut self) -> &mut SendStream {
			&mut self.send
		}

		pub fn recv_stream_mut(&mut self) -> &mut RecvStream {
			&mut self.recv
		}

		pub fn into_inner(self) -> (SendStream, RecvStream) {
			(self.send, self.recv)
		}
	}

	impl AsyncWrite for QuinnCompat {
		fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
			Pin::new(&mut self.send).poll_write(cx, buf).map_err(io::Error::other)
		}

		fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
			Pin::new(&mut self.send).poll_flush(cx)
		}

		fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
			Pin::new(&mut self.send).poll_shutdown(cx)
		}
	}

	impl AsyncRead for QuinnCompat {
		fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
			Pin::new(&mut self.recv).poll_read(cx, buf)
		}
	}
}
