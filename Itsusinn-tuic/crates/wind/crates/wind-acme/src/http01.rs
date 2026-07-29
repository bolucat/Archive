//! One-shot HTTP-01 certificate provisioning via `instant-acme`.
//!
//! Unlike the resolver-based [`start_acme`](crate::start_acme) flow (which
//! keeps a `rustls-acme` state machine running for background renewal), this
//! provisions or renews a certificate a single time and writes the PEM
//! certificate chain and private key to disk. Backends that load TLS material
//! from file paths (e.g. the quiche/tokio-quiche QUIC listeners) consume the
//! on-disk PEMs.

use std::{collections::HashMap, path::Path, sync::Arc};

use axum::{
	Router,
	extract::{Path as AxumPath, State},
	http::{HeaderValue, StatusCode, header},
	response::{IntoResponse, Response},
	routing::get,
};
use eyre::{Context, Result};
use instant_acme::{
	Account, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewAccount, NewOrder, Order, OrderStatus, RetryPolicy,
};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{debug, info, instrument};
use x509_parser::pem::parse_x509_pem;

type ChallengeMap = Arc<RwLock<HashMap<String, String>>>;

async fn handle_challenge(State(challenges): State<ChallengeMap>, AxumPath(token): AxumPath<String>) -> Response {
	let Some(key_auth) = challenges.read().await.get(&token).cloned() else {
		return StatusCode::NOT_FOUND.into_response();
	};
	debug!(%token, "serving challenge");
	(
		StatusCode::OK,
		[(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"))],
		key_auth,
	)
		.into_response()
}

/// Completes all pending HTTP-01 challenges for an order using a single
/// short-lived axum server on port 80.
async fn complete_http01_challenges(order: &mut Order) -> Result<()> {
	let challenges: ChallengeMap = Arc::new(RwLock::new(HashMap::new()));
	let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

	// Bind `[::]` (dual-stack on Linux, the ACME deployment target) rather than
	// IPv4-only `0.0.0.0`: Let's Encrypt validates AAAA records over IPv6 when
	// present, and this matches the resolver flow's bind. Falls back to IPv4 if
	// the host has no IPv6 stack.
	let listener = match TcpListener::bind("[::]:80").await {
		Ok(l) => l,
		Err(_) => TcpListener::bind("0.0.0.0:80").await.context(
			"Failed to bind port 80 for ACME challenge. Ensure port 80 is open and you are running as root (or use authbind).",
		)?,
	};

	let app = Router::new()
		.route("/.well-known/acme-challenge/{token}", get(handle_challenge))
		.with_state(challenges.clone());

	info!("HTTP-01 challenge server listening on :80");

	let server_handle = tokio::spawn(async move {
		axum::serve(listener, app)
			.with_graceful_shutdown(async move {
				let _ = shutdown_rx.await;
			})
			.await
	});

	let mut authorizations = order.authorizations();
	while let Some(result) = authorizations.next().await {
		let mut authz = result?;
		if authz.status == AuthorizationStatus::Valid {
			continue;
		}

		let mut challenge = authz
			.challenge(ChallengeType::Http01)
			.ok_or_else(|| eyre::eyre!("No HTTP-01 challenge found"))?;

		let token = challenge.token.to_string();
		let key_auth = challenge.key_authorization().as_str().to_string();

		// Register the response before telling Let's Encrypt the challenge is ready.
		challenges.write().await.insert(token, key_auth);
		challenge.set_ready().await?;
	}

	info!("polling for order ready...");
	let status = order.poll_ready(&RetryPolicy::default()).await?;

	let _ = shutdown_tx.send(());
	let _ = server_handle.await;

	if status != OrderStatus::Ready {
		eyre::bail!("ACME order invalid or failed: {:?}", status);
	}

	Ok(())
}

pub(crate) fn cert_not_after(cert_pem: &[u8]) -> Result<time::OffsetDateTime> {
	let (_, pem) = parse_x509_pem(cert_pem).map_err(|e| eyre::eyre!("parsing certificate PEM: {e}"))?;
	let cert = pem.parse_x509().map_err(|e| eyre::eyre!("parsing certificate DER: {e}"))?;
	Ok(cert.validity().not_after.to_datetime())
}

pub(crate) fn should_renew(not_after: time::OffsetDateTime, now: time::OffsetDateTime) -> bool {
	const RENEW_BEFORE_DAYS: i64 = 30;
	not_after <= now + time::Duration::days(RENEW_BEFORE_DAYS)
}

/// Provision (or renew) an ACME certificate via HTTP-01, writing the PEM cert
/// chain and private key to disk. If a fresh certificate already exists on disk
/// (more than 30 days from expiry) this is a no-op.
#[instrument(name = "acme", skip_all, fields(hostname = %hostname))]
pub async fn ensure_acme_cert(
	hostname: &str,
	email: Option<&str>,
	cert_path: &Path,
	key_path: &Path,
	staging: bool,
) -> Result<()> {
	if cert_path.exists() && key_path.exists() {
		let cert_pem = tokio::fs::read(cert_path).await.context("read cert file")?;
		let not_after = cert_not_after(&cert_pem)?;
		let now = time::OffsetDateTime::now_utc();
		let days_left = (not_after - now).whole_days();

		if !should_renew(not_after, now) {
			info!(days_left, not_after = %not_after, "cert fresh, skipping renewal");
			return Ok(());
		}
		info!(days_left, not_after = %not_after, "cert expiring soon or expired, renewing");
	} else {
		info!("no cert found, provisioning");
	}

	let contact: Vec<String> = email.into_iter().map(|e| format!("mailto:{e}")).collect();

	let directory_url = if staging {
		info!("using Let's Encrypt STAGING directory");
		LetsEncrypt::Staging.url().to_owned()
	} else {
		LetsEncrypt::Production.url().to_owned()
	};

	let (account, _credentials) = Account::builder()?
		.create(
			&NewAccount {
				contact: &contact.iter().map(String::as_str).collect::<Vec<_>>(),
				terms_of_service_agreed: true,
				only_return_existing: false,
			},
			directory_url,
			None,
		)
		.await
		.context("Failed to create ACME account")?;

	let identifiers = vec![Identifier::Dns(hostname.to_string())];
	let mut order = account
		.new_order(&NewOrder::new(&identifiers))
		.await
		.context("Failed to create ACME order")?;

	let state = order.state();
	if !matches!(state.status, OrderStatus::Pending | OrderStatus::Ready) {
		eyre::bail!("Unexpected order state: {:?}", state.status);
	}

	if matches!(state.status, OrderStatus::Pending) {
		complete_http01_challenges(&mut order).await?;
	}

	info!("finalizing order...");
	let private_key_pem = order.finalize().await?;
	// `poll_certificate` returns the PEM-encoded certificate chain (leaf +
	// intermediates), which is required by quiche/tokio-quiche for H3.
	let cert_chain_pem = order.poll_certificate(&RetryPolicy::default()).await?;

	if let Some(parent) = cert_path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}
	if let Some(parent) = key_path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}

	tokio::fs::write(cert_path, cert_chain_pem).await?;
	crate::write_key_file(key_path, private_key_pem.as_bytes()).await?;

	info!("cert issued and saved");

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn renews_when_certificate_expires_within_thirty_days() {
		let now = time::OffsetDateTime::now_utc();
		assert!(should_renew(now + time::Duration::days(10), now));
		assert!(should_renew(now - time::Duration::days(1), now));
		assert!(!should_renew(now + time::Duration::days(45), now));
	}

	#[test]
	fn parses_certificate_not_after_from_pem() {
		let not_after = time::OffsetDateTime::now_utc() + time::Duration::days(42);
		let mut params = rcgen::CertificateParams::new(vec!["example.com".to_string()]).unwrap();
		params.not_after = not_after;
		let key_pair = rcgen::KeyPair::generate().unwrap();
		let cert = params.self_signed(&key_pair).unwrap();

		let parsed = cert_not_after(cert.pem().as_bytes()).unwrap();
		assert_eq!(parsed.unix_timestamp(), not_after.unix_timestamp());
	}
}
