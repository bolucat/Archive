//! Isolate peer-reported UDP errors from a shared server endpoint.

use std::{
	future::Future,
	io::{self, IoSliceMut},
	net::SocketAddr,
	pin::Pin,
	task::{Context, Poll, ready},
	time::Duration,
};

use quinn::{AsyncUdpSocket, Runtime, TokioRuntime, UdpSender, udp::RecvMeta};
use tokio::time::Sleep;

/// Wrap a server socket without letting a remote peer's ICMP error terminate
/// every connection sharing the endpoint. Local socket failures still
/// propagate.
pub fn wrap_server_socket(socket: std::net::UdpSocket) -> io::Result<Box<dyn AsyncUdpSocket>> {
	Ok(Box::new(ServerUdpSocket::new(TokioRuntime.wrap_udp_socket(socket)?)))
}

#[derive(Debug)]
struct ServerUdpSocket {
	inner: Box<dyn AsyncUdpSocket>,
	retry: Option<Pin<Box<Sleep>>>,
}

impl ServerUdpSocket {
	fn new(inner: Box<dyn AsyncUdpSocket>) -> Self {
		Self { inner, retry: None }
	}
}

impl AsyncUdpSocket for ServerUdpSocket {
	fn create_sender(&self) -> Pin<Box<dyn UdpSender>> {
		self.inner.create_sender()
	}

	fn poll_recv(
		&mut self,
		cx: &mut Context<'_>,
		bufs: &mut [IoSliceMut<'_>],
		meta: &mut [RecvMeta],
	) -> Poll<io::Result<usize>> {
		loop {
			if let Some(retry) = &mut self.retry {
				ready!(retry.as_mut().poll(cx));
				self.retry = None;
			}
			match self.inner.poll_recv(cx, bufs, meta) {
				Poll::Ready(Err(err))
					if matches!(
						err.kind(),
						io::ErrorKind::ConnectionRefused
							| io::ErrorKind::ConnectionReset
							| io::ErrorKind::HostUnreachable
							| io::ErrorKind::NetworkUnreachable
					) =>
				{
					// Linux can deliver asynchronous errors for a previous send
					// on an unconnected UDP socket. They say nothing about
					// other peers. Yield with a timer: persistent errors
					// must neither spin inside one poll nor keep
					// rescheduling a hot task without a delay.
					tracing::debug!(error = %err, "ignoring peer UDP error on shared QUIC server socket");
					self.retry = Some(Box::pin(tokio::time::sleep(Duration::from_millis(1))));
				}
				result => return result,
			}
		}
	}

	fn local_addr(&self) -> io::Result<SocketAddr> {
		self.inner.local_addr()
	}

	fn max_receive_segments(&self) -> usize {
		self.inner.max_receive_segments()
	}

	fn may_fragment(&self) -> bool {
		self.inner.may_fragment()
	}
}

#[cfg(test)]
mod tests;
