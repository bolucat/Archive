//! The unified QUIC error type.
//!
//! Both backends funnel their connection/stream errors into [`QuicError`]. The
//! variants are deliberately the ones TUIC (and similar protocols) actually
//! branch on — clean local/peer shutdown, idle timeout, stream reset — plus
//! catch-all variants for everything else.

use thiserror::Error;

/// Convenience alias for `Result<T, QuicError>`.
pub type Result<T> = std::result::Result<T, QuicError>;

/// A backend-agnostic QUIC error.
#[derive(Debug, Error)]
pub enum QuicError {
	/// The peer closed the connection with an application error code.
	#[error("connection closed by peer (application code {code})")]
	ApplicationClosed { code: u64 },

	/// The connection was closed locally (e.g. via [`QuicConnection::close`]).
	///
	/// [`QuicConnection::close`]: crate::traits::QuicConnection::close
	#[error("connection closed locally")]
	LocallyClosed,

	/// The connection timed out (idle timeout / handshake timeout).
	#[error("connection timed out")]
	TimedOut,

	/// A stream was reset by the peer with the given error code.
	#[error("stream reset by peer (code {0})")]
	Reset(u64),

	/// The connection was lost for some other reason.
	#[error("connection lost: {0}")]
	ConnectionLost(String),

	/// A datagram could not be sent (unsupported, or larger than the peer's
	/// advertised maximum).
	#[error("datagram error: {0}")]
	Datagram(String),

	/// A TLS / crypto configuration or handshake error.
	#[error("tls error: {0}")]
	Tls(String),

	/// Endpoint setup / socket bind failure.
	#[error("endpoint error: {0}")]
	Endpoint(String),

	/// Anything not covered above.
	#[error("{0}")]
	Other(String),
}

impl From<std::io::Error> for QuicError {
	fn from(e: std::io::Error) -> Self {
		QuicError::Other(e.to_string())
	}
}

impl From<eyre::Report> for QuicError {
	fn from(e: eyre::Report) -> Self {
		QuicError::Other(e.to_string())
	}
}

#[cfg(feature = "quinn")]
mod quinn_conv {
	use super::QuicError;

	impl From<quinn::ConnectionError> for QuicError {
		fn from(e: quinn::ConnectionError) -> Self {
			use quinn::ConnectionError::*;
			match e {
				ApplicationClosed(c) => QuicError::ApplicationClosed {
					code: c.error_code.into_inner(),
				},
				LocallyClosed => QuicError::LocallyClosed,
				TimedOut => QuicError::TimedOut,
				Reset => QuicError::Reset(0),
				other => QuicError::ConnectionLost(other.to_string()),
			}
		}
	}

	impl From<quinn::WriteError> for QuicError {
		fn from(e: quinn::WriteError) -> Self {
			match e {
				quinn::WriteError::ConnectionLost(c) => c.into(),
				quinn::WriteError::Stopped(code) => QuicError::Reset(code.into_inner()),
				other => QuicError::Other(other.to_string()),
			}
		}
	}

	impl From<quinn::ReadError> for QuicError {
		fn from(e: quinn::ReadError) -> Self {
			match e {
				quinn::ReadError::ConnectionLost(c) => c.into(),
				quinn::ReadError::Reset(code) => QuicError::Reset(code.into_inner()),
				other => QuicError::Other(other.to_string()),
			}
		}
	}

	impl From<quinn::SendDatagramError> for QuicError {
		fn from(e: quinn::SendDatagramError) -> Self {
			QuicError::Datagram(e.to_string())
		}
	}
}

#[cfg(feature = "quiche")]
impl From<tokio_quiche::quiche::Error> for QuicError {
	fn from(e: tokio_quiche::quiche::Error) -> Self {
		QuicError::Other(format!("quiche: {e}"))
	}
}
