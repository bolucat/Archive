//! TUIC outbound client — quiche backend.
//!
//! The builder validates and stores connection parameters. A live client path
//! (handshake via [`wind_quic::quiche::connect`] driving the shared
//! [`crate::client`] / [`crate::proto`] code) is not yet wired up — this
//! mirrors the placeholder status carried over from `wind-tuiche`. The quiche
//! server is the production path; quiche clients are exercised via the quinn
//! client today.

use std::{net::SocketAddr, time::Duration};

use uuid::Uuid;

use crate::{Result, quiche::utils::ConnectionOpts};

/// TUIC client using the quiche backend (configuration-only placeholder).
#[allow(dead_code)]
pub struct TuicheOutbound {
	server_addr: SocketAddr,
	server_name: String,
	uuid: Uuid,
	password: Vec<u8>,
	opts: ConnectionOpts,
}

impl TuicheOutbound {
	/// Create a new TUIC client builder.
	pub fn builder() -> TuicheOutboundBuilder {
		TuicheOutboundBuilder::new()
	}
}

/// Builder for [`TuicheOutbound`].
pub struct TuicheOutboundBuilder {
	server_addr: Option<SocketAddr>,
	server_name: Option<String>,
	uuid: Option<Uuid>,
	password: Option<String>,
	max_idle_time: Duration,
	connect_timeout: Duration,
	verify_certificate: bool,
	opts: ConnectionOpts,
}

impl TuicheOutboundBuilder {
	/// Create a new builder.
	pub fn new() -> Self {
		Self {
			server_addr: None,
			server_name: None,
			uuid: None,
			password: None,
			max_idle_time: Duration::from_secs(30),
			connect_timeout: Duration::from_secs(5),
			verify_certificate: true,
			opts: ConnectionOpts::default(),
		}
	}

	/// Set the server address.
	pub fn server_addr(mut self, addr: SocketAddr) -> Self {
		self.server_addr = Some(addr);
		self
	}

	/// Set the server name (SNI).
	pub fn server_name(mut self, name: String) -> Self {
		self.server_name = Some(name);
		self
	}

	/// Set the user UUID.
	pub fn uuid(mut self, uuid: Uuid) -> Self {
		self.uuid = Some(uuid);
		self
	}

	/// Set the password.
	pub fn password(mut self, password: String) -> Self {
		self.password = Some(password);
		self
	}

	/// Set maximum idle time.
	pub fn max_idle_time(mut self, time: Duration) -> Self {
		self.max_idle_time = time;
		self
	}

	/// Set connection timeout.
	pub fn connect_timeout(mut self, timeout: Duration) -> Self {
		self.connect_timeout = timeout;
		self
	}

	/// Enable or disable certificate verification.
	pub fn verify_certificate(mut self, verify: bool) -> Self {
		self.verify_certificate = verify;
		self
	}

	/// Set connection options.
	pub fn connection_opts(mut self, opts: ConnectionOpts) -> Self {
		self.opts = opts;
		self
	}

	/// Build the client.
	pub fn build(self) -> Result<TuicheOutbound> {
		let server_addr = self.server_addr.ok_or_else(|| eyre::eyre!("Server address not set"))?;
		let server_name = self.server_name.ok_or_else(|| eyre::eyre!("Server name not set"))?;
		let uuid = self.uuid.ok_or_else(|| eyre::eyre!("UUID not set"))?;
		let password = self.password.ok_or_else(|| eyre::eyre!("Password not set"))?;

		Ok(TuicheOutbound {
			server_addr,
			server_name,
			uuid,
			password: password.into_bytes(),
			opts: self.opts,
		})
	}
}

impl Default for TuicheOutboundBuilder {
	fn default() -> Self {
		Self::new()
	}
}
