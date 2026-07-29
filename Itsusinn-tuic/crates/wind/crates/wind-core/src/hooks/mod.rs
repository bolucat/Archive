//! Downstream-extensibility hooks for inbound proxy behavior.
//!
//! When `wind` is used as a library these traits let downstream code customize
//! the three inbound concerns that are otherwise hard-wired:
//!
//! * **Authentication** — [`TuicAuthenticator`] (supplies password material;
//!   the core keeps the constant-time token compare) and
//!   [`UserPassAuthenticator`] (SOCKS5 username/password).
//! * **Traffic statistics** — a central, lock-free [`StatsCollector`] written
//!   as bytes flow, drained periodically to a [`TrafficSink`].
//! * **Connection management** — [`ConnectionHooks`] lifecycle callbacks with a
//!   veto ([`ConnectDecision`]) for per-user limits.
//!
//! All traits are object-safe (`#[async_trait]` + `Arc<dyn _>`), mirroring
//! [`OutboundAction`](crate::OutboundAction). They are bundled in
//! [`InboundHooks`], which is threaded into each inbound via its opts struct
//! and defaults to all-`None` (no behavior change).

use std::{
	collections::HashMap,
	fmt,
	net::SocketAddr,
	sync::{
		Arc,
		atomic::{AtomicU64, Ordering},
	},
	time::Duration,
};

use async_trait::async_trait;
use uuid::Uuid;

mod counting;
mod stats;

pub use counting::CountingStream;
pub use stats::{StatsCollector, UserTraffic};

/// Opaque per-user identity that ties auth → stats → connection-management
/// together. Backed by raw bytes (`Arc<[u8]>`) so identities need not be valid
/// UTF-8 (e.g. binary tokens or raw UUID bytes); cheap to clone.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UserId(Arc<[u8]>);

impl UserId {
	pub fn new(bytes: impl Into<Arc<[u8]>>) -> Self {
		Self(bytes.into())
	}

	pub fn as_bytes(&self) -> &[u8] {
		&self.0
	}
}

impl From<Uuid> for UserId {
	fn from(u: Uuid) -> Self {
		Self(Arc::from(u.into_bytes().as_slice()))
	}
}

impl From<String> for UserId {
	fn from(s: String) -> Self {
		Self(Arc::from(s.into_bytes().as_slice()))
	}
}

impl From<&str> for UserId {
	fn from(s: &str) -> Self {
		Self(Arc::from(s.as_bytes()))
	}
}

impl From<Vec<u8>> for UserId {
	fn from(v: Vec<u8>) -> Self {
		Self(Arc::from(v.as_slice()))
	}
}

impl From<&[u8]> for UserId {
	fn from(b: &[u8]) -> Self {
		Self(Arc::from(b))
	}
}

impl fmt::Display for UserId {
	/// Render as text when the bytes are valid UTF-8 *and* free of control
	/// characters (string-derived ids like usernames or `static:<uuid>` print
	/// verbatim), otherwise as lowercase hex.
	///
	/// The control-character guard matters: binary identities such as the
	/// panel-id encoding (`b'P'` + big-endian `i64`) or raw 16-byte UUIDs are
	/// frequently *valid* UTF-8 — small ids are mostly NUL bytes — so a plain
	/// `from_utf8` check would emit raw control bytes and garble the log panel.
	/// Falling back to hex keeps those ids readable and copy-pasteable.
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match std::str::from_utf8(&self.0) {
			Ok(s) if !s.chars().any(char::is_control) => f.write_str(s),
			_ => {
				for b in self.0.iter() {
					write!(f, "{b:02x}")?;
				}
				Ok(())
			}
		}
	}
}

/// Which inbound protocol a connection arrived on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
	Tuic,
	Socks5,
	Naive,
}

/// Per-connection context handed to [`ConnectionHooks`].
#[derive(Clone, Debug)]
pub struct ConnInfo {
	pub remote_addr: SocketAddr,
	pub protocol: Protocol,
	/// Process-unique id from [`next_conn_id`].
	pub conn_id: u64,
}

/// A [`ConnectionHooks`] veto.
#[derive(Clone, Debug)]
pub enum ConnectDecision {
	Accept,
	Reject(String),
}

impl ConnectDecision {
	pub fn is_accept(&self) -> bool {
		matches!(self, ConnectDecision::Accept)
	}
}

/// Supplies TUIC password material for a UUID.
///
/// The hook does **not** validate the token — it only returns the identity to
/// bind plus the raw password bytes. The TUIC core derives the expected token
/// via `export_keying_material` and compares it in constant time, and a `None`
/// from this hook still runs the constant-time dummy comparison, so this must
/// **never** be used to short-circuit unknown users.
#[async_trait]
pub trait TuicAuthenticator: Send + Sync + 'static {
	async fn lookup(&self, uuid: &Uuid) -> Option<(UserId, Arc<[u8]>)>;
}

/// Validates a SOCKS5 username/password and returns the bound identity.
#[async_trait]
pub trait UserPassAuthenticator: Send + Sync + 'static {
	async fn authenticate(&self, username: &str, password: &str) -> Option<UserId>;
}

/// Connection lifecycle callbacks. All default to no-op / accept.
#[async_trait]
pub trait ConnectionHooks: Send + Sync + 'static {
	/// A new connection was accepted (post-handshake, pre-auth — no `UserId`
	/// yet). Reject to drop it.
	async fn on_connect(&self, _info: &ConnInfo) -> ConnectDecision {
		ConnectDecision::Accept
	}

	/// The connection authenticated as `user`. Reject to drop it (e.g. a
	/// per-user concurrent-connection limit).
	async fn on_authenticated(&self, _info: &ConnInfo, _user: &UserId) -> ConnectDecision {
		ConnectDecision::Accept
	}

	/// The connection closed. `user` is `None` if it never authenticated.
	async fn on_disconnect(&self, _info: &ConnInfo, _user: Option<&UserId>) {}
}

/// Receives periodic batches of per-user traffic from the flush task.
#[async_trait]
pub trait TrafficSink: Send + Sync + 'static {
	/// Submit one flush cycle's batch. On `Err`, the collector `restore`s the
	/// batch so it rolls into the next cycle (zero loss).
	async fn submit(&self, batch: Vec<UserTraffic>) -> eyre::Result<()>;
}

/// The bundle of hooks threaded into an inbound. Cloneable handles; defaults to
/// all-`None` (current behavior).
#[derive(Clone)]
pub struct InboundHooks {
	pub tuic_auth: Option<Arc<dyn TuicAuthenticator>>,
	pub userpass_auth: Option<Arc<dyn UserPassAuthenticator>>,
	pub connection: Option<Arc<dyn ConnectionHooks>>,
	/// Central collector handle (set when traffic stats are enabled).
	pub stats: Option<Arc<StatsCollector>>,
	/// Cadence for the TUIC QUIC-stats sampler.
	pub sample_interval: Duration,
}

impl Default for InboundHooks {
	fn default() -> Self {
		Self {
			tuic_auth: None,
			userpass_auth: None,
			connection: None,
			stats: None,
			sample_interval: Duration::from_secs(60),
		}
	}
}

/// A process-unique, monotonic connection id for [`ConnInfo::conn_id`].
pub fn next_conn_id() -> u64 {
	static NEXT: AtomicU64 = AtomicU64::new(1);
	NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Default [`TuicAuthenticator`] backed by a static UUID→password map,
/// reproducing the pre-hooks behavior.
pub struct StaticTuicAuth {
	users: HashMap<Uuid, Arc<[u8]>>,
}

impl StaticTuicAuth {
	pub fn new(users: HashMap<Uuid, Arc<[u8]>>) -> Self {
		Self { users }
	}

	/// Build from the existing `HashMap<Uuid, String>` password config.
	pub fn from_passwords(users: &HashMap<Uuid, String>) -> Self {
		Self {
			users: users.iter().map(|(uuid, pw)| (*uuid, Arc::from(pw.as_bytes()))).collect(),
		}
	}
}

#[async_trait]
impl TuicAuthenticator for StaticTuicAuth {
	async fn lookup(&self, uuid: &Uuid) -> Option<(UserId, Arc<[u8]>)> {
		self.users.get(uuid).map(|pw| (UserId::from(*uuid), pw.clone()))
	}
}

/// Default [`UserPassAuthenticator`] backed by a single static credential,
/// reproducing the pre-hooks SOCKS5 behavior.
pub struct StaticUserPass {
	pub username: String,
	pub password: String,
	/// Identity to bind on success (defaults to the username if built via
	/// [`StaticUserPass::new`]).
	pub user_id: UserId,
}

impl StaticUserPass {
	pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
		let username = username.into();
		let user_id = UserId::from(username.as_str());
		Self {
			username,
			password: password.into(),
			user_id,
		}
	}
}

#[async_trait]
impl UserPassAuthenticator for StaticUserPass {
	async fn authenticate(&self, username: &str, password: &str) -> Option<UserId> {
		(username == self.username && password == self.password).then(|| self.user_id.clone())
	}
}

/// Combines several [`ConnectionHooks`] — connect/auth reject if ANY rejects;
/// disconnect fans out to all.
pub struct FanOutConnectionHooks(pub Vec<Arc<dyn ConnectionHooks>>);

#[async_trait]
impl ConnectionHooks for FanOutConnectionHooks {
	async fn on_connect(&self, info: &ConnInfo) -> ConnectDecision {
		for h in &self.0 {
			if let ConnectDecision::Reject(r) = h.on_connect(info).await {
				return ConnectDecision::Reject(r);
			}
		}
		ConnectDecision::Accept
	}

	async fn on_authenticated(&self, info: &ConnInfo, user: &UserId) -> ConnectDecision {
		for h in &self.0 {
			if let ConnectDecision::Reject(r) = h.on_authenticated(info, user).await {
				return ConnectDecision::Reject(r);
			}
		}
		ConnectDecision::Accept
	}

	async fn on_disconnect(&self, info: &ConnInfo, user: Option<&UserId>) {
		for h in &self.0 {
			h.on_disconnect(info, user).await;
		}
	}
}

#[cfg(test)]
mod tests {
	use std::sync::atomic::{AtomicUsize, Ordering};

	use super::*;

	struct Rejecter;
	#[async_trait]
	impl ConnectionHooks for Rejecter {
		async fn on_connect(&self, _i: &ConnInfo) -> ConnectDecision {
			ConnectDecision::Reject("nope".into())
		}
	}

	struct Counter(Arc<AtomicUsize>);
	#[async_trait]
	impl ConnectionHooks for Counter {
		async fn on_disconnect(&self, _i: &ConnInfo, _u: Option<&UserId>) {
			self.0.fetch_add(1, Ordering::Relaxed);
		}
	}

	fn info() -> ConnInfo {
		ConnInfo {
			remote_addr: "127.0.0.1:1000".parse().unwrap(),
			protocol: Protocol::Tuic,
			conn_id: next_conn_id(),
		}
	}

	#[tokio::test]
	async fn fanout_rejects_if_any_rejects() {
		let hooks = FanOutConnectionHooks(vec![Arc::new(Counter(Arc::new(AtomicUsize::new(0)))), Arc::new(Rejecter)]);
		assert!(!hooks.on_connect(&info()).await.is_accept());
	}

	#[tokio::test]
	async fn fanout_disconnect_calls_all() {
		let n = Arc::new(AtomicUsize::new(0));
		let hooks = FanOutConnectionHooks(vec![Arc::new(Counter(n.clone())), Arc::new(Counter(n.clone()))]);
		hooks.on_disconnect(&info(), None).await;
		assert_eq!(n.load(Ordering::Relaxed), 2);
	}

	#[tokio::test]
	async fn static_tuic_auth_lookup() {
		let uuid = Uuid::nil();
		let mut map = HashMap::new();
		map.insert(uuid, "secret".to_string());
		let auth = StaticTuicAuth::from_passwords(&map);

		let (user, pw) = auth.lookup(&uuid).await.unwrap();
		assert_eq!(user, UserId::from(uuid));
		assert_eq!(&*pw, b"secret");
		// Unknown UUID returns None (core must still run the dummy compare).
		assert!(auth.lookup(&Uuid::from_u128(1)).await.is_none());
	}

	#[tokio::test]
	async fn static_userpass_auth() {
		let auth = StaticUserPass::new("alice", "pw");
		assert_eq!(auth.authenticate("alice", "pw").await, Some(UserId::from("alice")));
		assert!(auth.authenticate("alice", "bad").await.is_none());
	}

	#[test]
	fn display_renders_printable_ids_verbatim() {
		assert_eq!(UserId::from("alice").to_string(), "alice");
		assert_eq!(UserId::from("static:abc-123").to_string(), "static:abc-123");
		// Non-ASCII but printable text stays verbatim.
		assert_eq!(UserId::from("用户").to_string(), "用户");
	}

	#[test]
	fn display_hexes_binary_ids_without_emitting_control_bytes() {
		// The panel-id encoding (`b'P'` + big-endian i64) is valid UTF-8 for
		// small ids (mostly NUL bytes) — it must NOT print as raw control bytes.
		let mut panel = vec![b'P'];
		panel.extend_from_slice(&42i64.to_be_bytes());
		let shown = UserId::from(panel).to_string();
		assert_eq!(shown, "50000000000000002a");
		assert!(!shown.chars().any(char::is_control), "log output must be control-char free");

		// Raw UUID bytes likewise fall back to hex rather than garbling.
		let uuid = Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);
		assert_eq!(UserId::from(uuid).to_string(), "0123456789abcdef0123456789abcdef");
	}
}
