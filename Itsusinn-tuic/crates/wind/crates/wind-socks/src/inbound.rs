use std::{
	net::{IpAddr, Ipv4Addr, SocketAddr},
	sync::Arc,
};

use fast_socks5::{ReplyError, Socks5Command, server::Socks5ServerProtocol, util::target_addr::TargetAddr as SocksTargetAddr};
use snafu::ResultExt;
use tokio::{
	net::{TcpListener, TcpStream},
	sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{Instrument as _, error, info, warn};
use wind_core::{
	AbstractInbound, ConnInfo, ConnectDecision, InboundCallback, InboundHooks, Protocol, StatsCollector, UserId,
	hooks::{CountingStream, next_conn_id},
	types::TargetAddr,
	udp::{UdpPacket, UdpStream},
};

use crate::{CallbackSnafu, Error, IoSnafu, SocksSnafu};

pub struct SocksInboundOpt {
	/// Bind on address address. eg. `127.0.0.1:1080`
	pub listen_addr: SocketAddr,

	/// Our external IP address to be sent in reply packets (required for UDP)
	pub public_addr: Option<std::net::IpAddr>,

	/// Choose authentication type
	pub auth: AuthMode,

	/// Don't perform the auth handshake, send directly the command request
	pub skip_auth: bool,

	/// Allow UDP proxying, requires public-addr to be set
	pub allow_udp: bool,

	/// Downstream extensibility hooks (auth / traffic stats / connection
	/// management). Defaults to all-`None` (no behavior change).
	pub hooks: InboundHooks,
}

pub enum AuthMode {
	NoAuth,
	Password { username: String, password: String },
}

pub struct SocksInbound {
	opts: Arc<SocksInboundOpt>,
	cancel: CancellationToken,
}

impl AbstractInbound for SocksInbound {
	async fn listen(&self, cb: &impl InboundCallback) -> eyre::Result<()> {
		let listener = TcpListener::bind(self.opts.listen_addr).await?;
		// Track per-connection tasks so shutdown can wait for them instead of
		// leaving in-flight sessions to be killed by runtime teardown. Each task
		// also gets a child token so cancellation aborts the session promptly.
		let conn_tasks = tokio_util::task::TaskTracker::new();
		loop {
			tokio::select! {
				_ = self.cancel.cancelled() => {
					info!(target: "socks_in_reactor", "Cancellation received, shutting down");
					break;
				}
				res = listener.accept() => {
					let (stream, client_addr) = match res {
						Err(err) => {
							error!(target:"[IN] REACTOR", "{:}", err);
							continue;
						}
						Ok(conn) => conn,
					};

					let opts = self.opts.clone();
					let cb = cb.clone();
					let conn_cancel = self.cancel.child_token();
					conn_tasks.spawn(
						async move {
							let handler_cancel = conn_cancel.clone();
							tokio::select! {
								_ = conn_cancel.cancelled() => {
									info!(target: "socks_in_handler", "session aborted by shutdown");
								}
								res = handle_income(opts, stream, client_addr, cb, handler_cancel) => {
									if let Err(err) = res {
										error!(target: "socks_in_handler" , "{:}", err);
									}
								}
							}
						}
						.in_current_span(),
					);
				}
			};
		}
		conn_tasks.close();
		conn_tasks.wait().await;
		Ok(())
	}
}

impl SocksInbound {
	// Pure plumbing — no `await` was reached inside. Drop the redundant
	// `async`. Callers update from `.await` to plain call.
	pub fn new(opts: SocksInboundOpt, cancel: CancellationToken) -> Self {
		Self {
			opts: Arc::new(opts),
			cancel,
		}
	}
}

async fn handle_income(
	opts: Arc<SocksInboundOpt>,
	stream: TcpStream,
	client_addr: SocketAddr,
	cb: impl InboundCallback,
	cancel: CancellationToken,
) -> Result<(), Error> {
	let conn_info = ConnInfo {
		remote_addr: client_addr,
		protocol: Protocol::Socks5,
		conn_id: next_conn_id(),
	};

	// Connection-level veto (pre-auth — no UserId yet).
	if let Some(ch) = &opts.hooks.connection
		&& let ConnectDecision::Reject(reason) = ch.on_connect(&conn_info).await
	{
		info!(target: "socks_in_handler", "connection from {} rejected by hook: {}", client_addr, reason);
		return Ok(());
	}

	// Track the authenticated identity so the disconnect hook can report it,
	// regardless of how the session exits.
	let mut user: Option<UserId> = None;
	let result = serve_socks(&opts, stream, client_addr, &cb, &cancel, &mut user).await;
	if let Some(ch) = &opts.hooks.connection {
		ch.on_disconnect(&conn_info, user.as_ref()).await;
	}
	result
}

async fn serve_socks(
	opts: &Arc<SocksInboundOpt>,
	stream: TcpStream,
	client_addr: SocketAddr,
	cb: &impl InboundCallback,
	cancel: &CancellationToken,
	user: &mut Option<UserId>,
) -> Result<(), Error> {
	// Authenticate. If a `UserPassAuthenticator` hook is set it takes precedence:
	// the client's credentials are captured during the SOCKS5 password sub-
	// negotiation and validated via the hook (NB: fast_socks5 has already replied
	// "auth success" by then, so a rejected user simply has its session dropped).
	// Otherwise the static `AuthMode` is used and the username becomes the
	// identity.
	let proto = if let Some(auth) = &opts.hooks.userpass_auth {
		let captured: Arc<std::sync::Mutex<Option<(String, String)>>> = Arc::new(std::sync::Mutex::new(None));
		let cap = captured.clone();
		let proto = Socks5ServerProtocol::accept_password_auth(stream, move |u, p| {
			*cap.lock().unwrap() = Some((u.to_string(), p.to_string()));
			true
		})
		.await
		.context(SocksSnafu)?
		.0;
		let creds = captured.lock().unwrap().take();
		match creds {
			Some((u, p)) => match auth.authenticate(&u, &p).await {
				Some(id) => *user = Some(id),
				None => {
					warn!(target: "socks_in_handler", "SOCKS5 auth rejected for user {:?} from {}", u, client_addr);
					return Ok(());
				}
			},
			None => return Ok(()),
		}
		proto
	} else {
		match &opts.auth {
			AuthMode::NoAuth => Socks5ServerProtocol::accept_no_auth(stream).await.context(SocksSnafu)?,
			AuthMode::Password { username, password } => {
				let proto = Socks5ServerProtocol::accept_password_auth(stream, |u, p| u == *username && p == *password)
					.await
					.context(SocksSnafu)?
					.0;
				*user = Some(UserId::from(username.as_str()));
				proto
			}
		}
	};

	let (proto, cmd, target_addr) = proto.read_command().await?;

	match cmd {
		Socks5Command::TCPConnect => {
			let inner = proto
				.reply_success(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0))
				.await?;
			let target_addr = match target_addr {
				SocksTargetAddr::Ip(socket_addr) => match socket_addr {
					SocketAddr::V4(socket_addr) => TargetAddr::IPv4(*socket_addr.ip(), socket_addr.port()),
					SocketAddr::V6(socket_addr) => TargetAddr::IPv6(*socket_addr.ip(), socket_addr.port()),
				},
				SocksTargetAddr::Domain(domain, port) => TargetAddr::Domain(domain, port),
			};

			// Count per-user TCP traffic when stats are enabled and the client is
			// identified (anonymous NoAuth sessions are not metered).
			match (&opts.hooks.stats, user.as_ref()) {
				(Some(stats), Some(uid)) => {
					stats.record_request(uid);
					let counted = CountingStream::new(inner, stats.clone(), uid.clone());
					cb.handle_tcpstream(target_addr, counted).await.context(CallbackSnafu)?;
				}
				_ => {
					cb.handle_tcpstream(target_addr, inner).await.context(CallbackSnafu)?;
				}
			}
		}
		Socks5Command::UDPAssociate if opts.allow_udp => {
			// RFC 1928 §6: the reply IP in BND.ADDR must be reachable by the
			// client. Previously hardcoded to 127.0.0.1, which broke any
			// non-loopback client. Prefer the operator-supplied `public_addr`;
			// otherwise fall back to the local TCP listen address (still
			// reachable for same-host clients) and log a warning.
			let reply_ip = match opts.public_addr {
				Some(ip) => ip,
				None => {
					warn!(
						target: "socks_in_handler",
						"SOCKS5 UDPAssociate from {} without `public_addr` configured; \
						 falling back to listen-IP {}. Remote clients will not be able \
						 to reach the UDP relay.",
						client_addr,
						opts.listen_addr.ip(),
					);
					opts.listen_addr.ip()
				}
			};
			let expected_client_ip = client_addr.ip();

			// Per-user UDP accounting (only when stats are enabled and the client
			// is identified). One associate ≈ one request.
			let stats = opts.hooks.stats.clone();
			let stats_user = user.clone();
			if let (Some(s), Some(u)) = (&stats, &stats_user) {
				s.record_request(u);
			}

			crate::ext::run_udp_proxy(proto, &target_addr, None, reply_ip, move |inbound| async move {
				let (tx_to_out, rx_from_in) = mpsc::channel(100);
				let (tx_to_in, rx_from_out) = mpsc::channel(100);

				let udp_stream = UdpStream {
					tx: tx_to_out,
					rx: rx_from_out,
				};

				// Wrap the callback-facing stream so UDP payloads are metered.
				let udp_stream = match (stats, stats_user) {
					(Some(s), Some(u)) => count_udp(udp_stream, s, u),
					_ => udp_stream,
				};

				let serve_stream = UdpStream {
					tx: tx_to_in,
					rx: rx_from_in,
				};

				let cb = cb.clone();
				// Detached from the session task, so it needs its own cancel
				// guard — otherwise it would outlive shutdown until runtime drop.
				let udp_cancel = cancel.clone();
				tokio::spawn(
					async move {
						tokio::select! {
							_ = udp_cancel.cancelled() => {}
							res = cb.handle_udpstream(udp_stream) => {
								if let Err(e) = res {
									error!(target: "socks_in_handler", "UDP association error: {}", e);
								}
							}
						}
					}
					.in_current_span(),
				);

				crate::udp::serve_udp_with_client(inbound.into(), serve_stream, Some(expected_client_ip))
					.await
					.context(IoSnafu)
			})
			.await?;
		}
		_ => {
			proto.reply_error(&ReplyError::CommandNotSupported).await?;
			return Err(ReplyError::CommandNotSupported.into());
		}
	};
	Ok(())
}

/// Interpose counting forwarders on a callback-facing [`UdpStream`]: payloads
/// the callback reads (client→proxy) are recorded as upload, payloads it writes
/// (proxy→client) as download. Used only when SOCKS5 stats are enabled — the
/// default path keeps the raw channels with zero overhead. The forwarder tasks
/// exit when either side closes its channel.
fn count_udp(stream: UdpStream, stats: Arc<StatsCollector>, user: UserId) -> UdpStream {
	let UdpStream {
		tx: inner_tx,
		rx: mut inner_rx,
	} = stream;
	let (cb_tx, mut cb_tx_rx) = mpsc::channel::<UdpPacket>(100); // callback writes here (download)
	let (up_tx, cb_rx) = mpsc::channel::<UdpPacket>(100); // callback reads here (upload)

	// download: callback → client.
	let dl_stats = stats.clone();
	let dl_user = user.clone();
	tokio::spawn(async move {
		while let Some(pkt) = cb_tx_rx.recv().await {
			dl_stats.record_download(&dl_user, pkt.payload.len() as u64);
			if inner_tx.send(pkt).await.is_err() {
				break;
			}
		}
	});

	// upload: client → callback.
	tokio::spawn(async move {
		while let Some(pkt) = inner_rx.recv().await {
			stats.record_upload(&user, pkt.payload.len() as u64);
			if up_tx.send(pkt).await.is_err() {
				break;
			}
		}
	});

	UdpStream { tx: cb_tx, rx: cb_rx }
}
