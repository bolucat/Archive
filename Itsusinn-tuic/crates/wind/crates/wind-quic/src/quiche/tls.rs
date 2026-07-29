//! Hot-reloadable TLS for the quiche backend.
//!
//! tokio-quiche loads the certificate into the BoringSSL `SSL_CTX` **once** at
//! `listen()` time and reuses that context for every connection, so a renewed
//! certificate on disk is never picked up without a restart.
//!
//! We work around this through the only TLS customization seam tokio-quiche
//! exposes — [`ConnectionHook::create_custom_ssl_context_builder`]. The hook
//! installs a per-handshake `select_certificate` callback that serves the
//! *current* certificate from a hot-swappable [`CertStore`]. Calling
//! [`CertStore::update`] changes the certificate used by all **subsequent**
//! handshakes with no listener restart; in-flight connections keep the
//! certificate they negotiated.
//!
//! This is pure QUIC-TLS infrastructure (boring-based) — independent of any
//! application protocol — so it lives in `wind-quic` and is shared by every
//! quiche-backed server. `wind-quic` exposes it so quiche servers can opt into
//! live certificate rotation (e.g. ACME renewal); quinn/rustls has no
//! equivalent seam.

use std::sync::Arc;

use arc_swap::ArcSwap;
use boring::{
	error::ErrorStack,
	pkey::{PKey, Private},
	ssl::{SelectCertError, SslContextBuilder, SslMethod, SslRef, SslVersion},
	x509::X509,
};
use eyre::{WrapErr as _, eyre};
use tokio_quiche::{quic::ConnectionHook, settings::TlsCertificatePaths};

/// A parsed leaf certificate, its intermediate chain, and the private key.
struct CurrentCert {
	leaf: X509,
	chain: Vec<X509>,
	key: PKey<Private>,
}

fn parse(cert_pem: &[u8], key_pem: &[u8]) -> eyre::Result<CurrentCert> {
	let mut stack = X509::stack_from_pem(cert_pem).wrap_err("parsing certificate chain PEM")?;
	if stack.is_empty() {
		return Err(eyre!("certificate PEM contained no certificates"));
	}
	let leaf = stack.remove(0);
	let key = PKey::private_key_from_pem(key_pem).wrap_err("parsing private key PEM")?;
	Ok(CurrentCert { leaf, chain: stack, key })
}

/// A hot-swappable certificate handle shared between the QUIC listener and any
/// renewal source (e.g. ACME). Cheap to clone (`Arc`).
#[derive(Clone)]
pub struct CertStore {
	inner: Arc<ArcSwap<CurrentCert>>,
}

impl CertStore {
	/// Build a store from a leaf+chain PEM and a private-key PEM.
	pub fn from_pem(cert_pem: &[u8], key_pem: &[u8]) -> eyre::Result<Self> {
		Ok(Self {
			inner: Arc::new(ArcSwap::from_pointee(parse(cert_pem, key_pem)?)),
		})
	}

	/// Replace the served certificate. Subsequent handshakes use the new
	/// certificate; existing connections keep the one they negotiated.
	pub fn update(&self, cert_pem: &[u8], key_pem: &[u8]) -> eyre::Result<()> {
		self.inner.store(Arc::new(parse(cert_pem, key_pem)?));
		Ok(())
	}

	/// Install the current certificate onto a per-connection `SslRef`.
	fn install(&self, ssl: &mut SslRef) -> Result<(), ErrorStack> {
		let cur = self.inner.load();
		ssl.set_certificate(&cur.leaf)?;
		ssl.set_private_key(&cur.key)?;
		for cert in &cur.chain {
			ssl.add_chain_cert(cert)?;
		}
		Ok(())
	}
}

/// A [`ConnectionHook`] that serves the current certificate from a
/// [`CertStore`] via a per-handshake `select_certificate` callback, enabling
/// live cert rotation on the quiche backend.
pub(crate) struct CertReloadHook {
	store: CertStore,
}

impl CertReloadHook {
	pub(crate) fn new(store: CertStore) -> Self {
		Self { store }
	}
}

impl ConnectionHook for CertReloadHook {
	fn create_custom_ssl_context_builder(&self, _settings: TlsCertificatePaths<'_>) -> Option<SslContextBuilder> {
		let mut builder = SslContextBuilder::new(SslMethod::tls()).ok()?;
		// QUIC mandates TLS 1.3.
		builder.set_min_proto_version(Some(SslVersion::TLS1_3)).ok()?;
		builder.set_max_proto_version(Some(SslVersion::TLS1_3)).ok()?;

		// Seed the context with an initial certificate so it is valid even
		// before the per-handshake callback runs.
		{
			let cur = self.store.inner.load();
			builder.set_certificate(&cur.leaf).ok()?;
			builder.set_private_key(&cur.key).ok()?;
			for cert in &cur.chain {
				builder.add_extra_chain_cert(cert.clone()).ok()?;
			}
		}

		// Per-handshake: install the *current* certificate so renewals are
		// picked up without rebuilding the context.
		let store = self.store.clone();
		builder.set_select_certificate_callback(move |mut hello| {
			store.install(hello.ssl_mut()).map_err(|_| SelectCertError::ERROR)
		});

		Some(builder)
	}
}
