//! Resolver-based ACME via `rustls-acme` (background renewal state machine).
//!
//! This is the entry point used by the rustls-based (quinn) backend. Backends
//! that need the certificate as on-disk files should use [`crate::http01`].

use std::{
	path::{Path, PathBuf},
	sync::Arc,
};

use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use axum::Router;
use eyre::{Context, Result};
use rustls::server::ResolvesServerCert;
use rustls_acme::{AccountCache, AcmeConfig, CertCache, UseChallenge::Http01, caches::DirCache};
use tokio::sync::watch;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::is_valid_domain;

/// The PEM blob rustls-acme persists for a certificate: the PKCS#8 private key
/// followed by the certificate chain, all PEM-encoded. Published by
/// [`start_acme_with_cert`] so non-rustls consumers (e.g. the tokio-quiche
/// backend, which needs on-disk cert/key files) can materialise it.
pub type CertPem = Arc<Vec<u8>>;

/// A [`rustls_acme`] cache that wraps [`DirCache`] and, in addition to the
/// normal disk persistence, publishes every loaded/stored certificate PEM blob
/// to a [`watch`] channel.
struct CapturingCache {
	inner: DirCache<PathBuf>,
	tx: watch::Sender<Option<CertPem>>,
}

#[async_trait]
impl CertCache for CapturingCache {
	type EC = std::io::Error;

	async fn load_cert(&self, domains: &[String], directory_url: &str) -> Result<Option<Vec<u8>>, Self::EC> {
		let cert = self.inner.load_cert(domains, directory_url).await?;
		if let Some(pem) = &cert {
			let _ = self.tx.send(Some(Arc::new(pem.clone())));
		}
		Ok(cert)
	}

	async fn store_cert(&self, domains: &[String], directory_url: &str, cert: &[u8]) -> Result<(), Self::EC> {
		// Publish before persisting so a waiter is unblocked as early as possible.
		let _ = self.tx.send(Some(Arc::new(cert.to_vec())));
		self.inner.store_cert(domains, directory_url, cert).await
	}
}

#[async_trait]
impl AccountCache for CapturingCache {
	type EA = std::io::Error;

	async fn load_account(&self, contact: &[String], directory_url: &str) -> Result<Option<Vec<u8>>, Self::EA> {
		self.inner.load_account(contact, directory_url).await
	}

	async fn store_account(&self, contact: &[String], directory_url: &str, account: &[u8]) -> Result<(), Self::EA> {
		self.inner.store_account(contact, directory_url, account).await
	}
}

/// Start automatic ACME certificate management.
///
/// Uses `rustls-acme` to automatically provision and renew certificates from
/// Let's Encrypt via HTTP-01 challenges. An HTTP challenge server on port 80 is
/// started **only** when the ACME state machine actually needs to answer a
/// challenge, and is shut down once the new certificate has been deployed.
///
/// Returns a certificate resolver that can be used with a
/// `rustls::ServerConfig`.
///
/// This is the resolver-only entry point used by the rustls-based (quinn)
/// backend. Backends that need the certificate as on-disk files (e.g. the
/// tokio-quiche backend) should use [`start_acme_with_cert`] instead.
pub async fn start_acme(
	cancel: CancellationToken,
	hostname: &str,
	acme_email: &str,
	cache_dir: &Path,
	production: bool,
) -> Result<Arc<dyn ResolvesServerCert>> {
	let (resolver, _cert_rx) = start_acme_with_cert(cancel, hostname, acme_email, cache_dir, production).await?;
	Ok(resolver)
}

/// Like [`start_acme`], but additionally returns a [`watch::Receiver`] that is
/// updated with the certificate PEM blob (PKCS#8 private key + certificate
/// chain) whenever a certificate is loaded from cache or freshly
/// issued/renewed.
///
/// The initial value is `None`; it becomes `Some` once a cached certificate is
/// found or the first issuance completes. Consumers that need cert/key files
/// (the tokio-quiche backend) can wait on this and materialise the blob to
/// disk.
pub async fn start_acme_with_cert(
	cancel: CancellationToken,
	hostname: &str,
	acme_email: &str,
	cache_dir: &Path,
	production: bool,
) -> Result<(Arc<dyn ResolvesServerCert>, watch::Receiver<Option<CertPem>>)> {
	if !is_valid_domain(hostname) {
		return Err(eyre::eyre!("Invalid domain name: {hostname}"));
	}

	let contact = if !acme_email.is_empty() {
		format!("mailto:{acme_email}")
	} else {
		format!("mailto:admin@{hostname}")
	};

	info!("Starting ACME certificate management for domain: {hostname}");

	tokio::fs::create_dir_all(cache_dir)
		.await
		.context("Failed to create ACME cache directory")?;

	let (cert_tx, cert_rx) = watch::channel(None);
	let cache = CapturingCache {
		inner: DirCache::new(cache_dir.to_path_buf()),
		tx: cert_tx,
	};

	let mut state = AcmeConfig::new(vec![hostname.to_string()])
		.contact(vec![contact])
		.cache(cache)
		.directory_lets_encrypt(production)
		.challenge_type(Http01)
		.state();

	let default_config = state.default_rustls_config();
	let resolver = default_config.cert_resolver.clone();

	let axum_cancel: ArcSwapOption<CancellationToken> = None.into();
	let hostname = hostname.to_string();

	// Drive the ACME state machine in background. Previously this task was
	// fire-and-forget: a stream that ended (the rustls-acme state machine
	// returning None) or an error storm would simply spin down with no signal
	// to the rest of the server, leaving TLS pinned to whatever cached cert
	// was last loaded until process restart. We now:
	//   * propagate `cancel` so shutdown is graceful,
	//   * track consecutive errors and escalate to error! when they cluster,
	//   * log a loud message when the loop exits so operators see it.
	tokio::spawn(async move {
		let mut consecutive_errors: u32 = 0;
		let mut total_errors: u64 = 0;
		loop {
			tokio::select! {
				biased;
				_ = cancel.cancelled() => {
					info!("ACME: cancellation requested for domain {hostname}, shutting down state machine");
					axum_cancel.swap(None).inspect(|v| v.cancel());
					break;
				}
				event = state.next() => match event {
					Some(Ok(event)) => {
						consecutive_errors = 0;
						match event {
							// Requesting certificate for the first time or renewing
							rustls_acme::EventOk::AccountCacheStore => {
								info!("ACME event: AccountCacheStore");
							}
							rustls_acme::EventOk::ValidationChallenge(challenge) => {
								info!("ACME event: ValidationChallenge for {}", challenge.url);
								let child = Arc::new(cancel.child_token());
								axum_cancel.swap(Some(child.clone())).inspect(|v| v.cancel());
								let http01_service = state.http01_challenge_tower_service();
								let axum_app =
									Router::new().route_service("/.well-known/acme-challenge/{challenge_token}", http01_service);
								if let Err(e) = spawn_axum(child.child_token(), axum_app).await {
									error!("Failed to start ACME HTTP-01 challenge server: {:?}", e);
								}
							}
							rustls_acme::EventOk::DeployedNewCert(_) => {
								info!("ACME event: DeployedNewCert");
								axum_cancel.swap(None).inspect(|v| v.cancel());
							}
							rustls_acme::EventOk::DeployedCachedCert(_) => {
								info!("ACME event: DeployedCachedCert");
							}
							_ => info!("ACME event: {:?}", event),
						}
					}
					Some(Err(e)) => {
						consecutive_errors = consecutive_errors.saturating_add(1);
						total_errors = total_errors.saturating_add(1);
						if consecutive_errors >= 3 {
							error!(
								"ACME error (#{consecutive_errors} in a row, {total_errors} total) for {hostname}: {e:?} \
								 — cached certificate (if any) will continue to be served"
							);
						} else {
							warn!("ACME error for {hostname}: {e:?}");
						}
					}
					None => {
						error!(
							"ACME state machine stream ended for {hostname} ({total_errors} errors total). \
							 No further renewals will be attempted until the process is restarted."
						);
						axum_cancel.swap(None).inspect(|v| v.cancel());
						break;
					}
				}
			}
		}
		info!("ACME background task for {hostname} exited");
	});

	Ok((resolver, cert_rx))
}

async fn spawn_axum(cancel: CancellationToken, router: Router) -> eyre::Result<()> {
	let listener = tokio::net::TcpListener::bind("[::]:80")
		.await
		.context("Failed to bind to port 80 for ACME HTTP-01 challenges")?;
	info!("Started ACME HTTP-01 challenge server on port 80");
	tokio::spawn(async move {
		tokio::select! {
			Err(e) = axum::serve(listener, router) => {
				error!("ACME HTTP-01 challenge server error: {:?}", e);
			}
			_ = cancel.cancelled() => {
				info!("ACME HTTP-01 challenge server cancellation requested");
			}
		}
		info!("ACME certificate deployed, shutting down HTTP-01 challenge server");
	});
	Ok(())
}
