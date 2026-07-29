use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};

/// A duplex byte stream relayed by the proxy.
///
/// `Sync` is intentionally **not** required: relay streams are always owned and
/// moved into a spawned task (which needs `Send`, not `Sync`). Requiring `Sync`
/// would exclude perfectly valid streams whose halves are joined from channel
/// senders/receivers — e.g. the quiche QUIC backend's `tokio_util::PollSender`,
/// which is `Send` but not `Sync`.
pub trait AbstractTcpStream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> AbstractTcpStream for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

/// TCP keepalive parameters for outbound connections.
///
/// When set on a `DirectOutboundOpts` or `Socks5ActionOpts`, `SO_KEEPALIVE` is
/// enabled and (on Linux) the keepalive timers are tuned accordingly.  `None`
/// means leave the socket at the OS default (typically keepalive off).
#[derive(Clone, Debug)]
pub struct TcpKeepalive {
	/// Idle time before the first keepalive probe (`TCP_KEEPIDLE` on Linux).
	pub idle: Duration,
	/// Interval between successive keepalive probes (`TCP_KEEPINTVL` on Linux).
	pub interval: Duration,
	/// Maximum number of unacknowledged probes before the connection is
	/// declared dead (`TCP_KEEPCNT` on Linux).
	pub retries: u32,
}

impl Default for TcpKeepalive {
	fn default() -> Self {
		Self {
			idle: Duration::from_secs(300),
			interval: Duration::from_secs(75),
			retries: 3,
		}
	}
}
