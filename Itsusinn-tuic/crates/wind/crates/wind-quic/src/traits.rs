//! The handle-based async QUIC trait family.
//!
//! Modeled on the `h3::quic` traits but narrowed to what proxy protocols such
//! as TUIC actually use. A backend provides a `Clone` connection handle plus
//! send/recv stream types; everything above the trait (the TUIC state machine,
//! relays, …) is written once against these traits.

use std::{future::Future, net::SocketAddr};

use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::QuicError;

/// The send half of a QUIC stream.
///
/// `AsyncWrite` carries the payload; [`finish`](QuicSendStream::finish) marks a
/// clean end-of-stream (so the peer observes EOF rather than a reset), and
/// [`reset`](QuicSendStream::reset) abandons it with an error code.
pub trait QuicSendStream: AsyncWrite + Unpin + Send + 'static {
	/// Cleanly finish the stream: flush the FIN so the peer reads EOF.
	/// Idempotent.
	fn finish(&mut self) -> Result<(), QuicError>;

	/// Abruptly reset the stream with `code`, discarding unsent data.
	fn reset(&mut self, code: u64);

	/// The QUIC stream id of this send half.
	///
	/// Needed by protocol layers (e.g. the HTTP/3 masquerade's `h3::quic`
	/// adapter) that must report stream ids to the peer.
	fn id(&self) -> u64;
}

/// The receive half of a QUIC stream.
pub trait QuicRecvStream: AsyncRead + Unpin + Send + 'static {
	/// Ask the peer to stop sending, with error `code`.
	fn stop(&mut self, code: u64);

	/// The QUIC stream id of this recv half.
	fn id(&self) -> u64;
}

/// A cheaply-cloneable handle to an established QUIC connection.
///
/// All methods take `&self`; the handle is `Clone + Send + Sync` so it can be
/// shared across tasks. Dropping every clone does **not** necessarily close the
/// connection — use [`close`](QuicConnection::close) for a deterministic
/// shutdown.
pub trait QuicConnection: Clone + Send + Sync + 'static {
	/// The send-stream type produced by [`open_bi`](Self::open_bi) /
	/// [`open_uni`](Self::open_uni) / [`accept_bi`](Self::accept_bi).
	type SendStream: QuicSendStream;
	/// The recv-stream type produced by [`open_bi`](Self::open_bi) /
	/// [`accept_bi`](Self::accept_bi) / [`accept_uni`](Self::accept_uni).
	type RecvStream: QuicRecvStream;

	/// Open a new outbound bidirectional stream.
	fn open_bi(&self) -> impl Future<Output = Result<(Self::SendStream, Self::RecvStream), QuicError>> + Send;

	/// Accept the next inbound bidirectional stream.
	fn accept_bi(&self) -> impl Future<Output = Result<(Self::SendStream, Self::RecvStream), QuicError>> + Send;

	/// Open a new outbound unidirectional (send-only) stream.
	fn open_uni(&self) -> impl Future<Output = Result<Self::SendStream, QuicError>> + Send;

	/// Accept the next inbound unidirectional (recv-only) stream.
	fn accept_uni(&self) -> impl Future<Output = Result<Self::RecvStream, QuicError>> + Send;

	/// Send an unreliable datagram (RFC 9221).
	fn send_datagram(&self, data: Bytes) -> Result<(), QuicError>;

	/// Receive the next unreliable datagram.
	fn read_datagram(&self) -> impl Future<Output = Result<Bytes, QuicError>> + Send;

	/// The maximum datagram payload the peer will currently accept, or `None`
	/// if datagrams are not available.
	fn max_datagram_size(&self) -> Option<usize>;

	/// Export keying material (RFC 5705) into `out`.
	///
	/// Async to accommodate the quiche backend, where the export is routed
	/// through the connection's driver task; the quinn backend resolves
	/// immediately.
	fn export_keying_material<'a>(
		&'a self,
		out: &'a mut [u8],
		label: &'a [u8],
		context: &'a [u8],
	) -> impl Future<Output = Result<(), QuicError>> + Send + 'a;

	/// Close the connection with application error `code` and `reason`.
	fn close(&self, code: u32, reason: &[u8]);

	/// Resolve once the connection has closed (for any reason).
	fn closed(&self) -> impl Future<Output = ()> + Send;

	/// The peer's socket address, if known.
	fn peer_addr(&self) -> Option<SocketAddr> {
		None
	}

	/// Cumulative wire byte counters for this connection as `(sent, recv)`,
	/// or `None` if the backend doesn't expose them.
	///
	/// `sent` is bytes the local endpoint has put on the wire (server→client =
	/// download), `recv` is bytes received from the peer (client→server =
	/// upload). These are *wire* bytes (including QUIC framing / ACKs /
	/// retransmits), not relayed payload — used for lightweight per-connection
	/// traffic accounting. Async for trait uniformity; both backends resolve
	/// immediately — quiche reads counters its driver caches in shared atomics,
	/// quinn reads the connection handle directly.
	fn byte_stats(&self) -> impl Future<Output = Option<(u64, u64)>> + Send {
		async { None }
	}
}
