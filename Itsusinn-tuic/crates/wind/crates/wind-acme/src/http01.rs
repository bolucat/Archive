//! One-shot HTTP-01 certificate provisioning via `instant-acme`.
//!
//! Unlike the resolver-based
//! [`start_acme_with_cert`](crate::start_acme_with_cert) flow (which
//! keeps a `rustls-acme` state machine running for background renewal), this
//! provisions or renews a certificate a single time and writes the PEM
//! certificate chain and private key to disk. Backends that load TLS material
//! from file paths (e.g. the quiche/tokio-quiche QUIC listeners) consume the
//! on-disk PEMs.

use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	sync::Arc,
};

use axum::{
	Router,
	extract::{Path as AxumPath, State},
	http::{HeaderValue, StatusCode, header},
	response::{IntoResponse, Response},
	routing::get,
};
use eyre::{Context, Result};
use instant_acme::{
	Account, AccountCredentials, AuthorizationStatus, ChallengeType, Identifier, LetsEncrypt, NewAccount, NewOrder, Order,
	OrderStatus, RetryPolicy,
};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{debug, info, instrument, warn};
use x509_parser::pem::parse_x509_pem;

type ChallengeMap = Arc<RwLock<HashMap<String, String>>>;

/// ACME account credentials as persisted next to the certificate key.
///
/// `instant_acme::AccountCredentials` is opaque but serializable; the directory
/// URL is stored alongside it because the credentials themselves do not expose
/// which ACME directory they belong to, and reusing a staging account against
/// production (or vice versa) would silently target the wrong server.
#[derive(Deserialize, Serialize)]
struct StoredAccount {
	directory_url: String,
	credentials: AccountCredentials,
}

/// File the account credentials are persisted to for a given key path.
///
/// The account is per certificate/key pair, so it lives next to the key file:
/// `/etc/wind/example.com.key` → `/etc/wind/example.com.account.json`.
fn account_credentials_path(key_path: &Path) -> PathBuf {
	key_path.with_extension("account.json")
}

/// Read persisted account credentials for `directory_url`.
///
/// Returns `None` when nothing is stored yet, when the stored data is
/// unreadable, or when the stored account belongs to a different ACME
/// directory; in all of those cases the caller must register a fresh account
/// (which then overwrites the file, so a damaged file heals on the next run).
/// Credentials are a private key, so the file is written owner-only on Unix
/// (see [`crate::write_key_file`]).
async fn load_stored_credentials(path: &Path, directory_url: &str) -> Option<AccountCredentials> {
	if !path.exists() {
		return None;
	}

	let raw = match tokio::fs::read_to_string(path).await {
		Ok(raw) => raw,
		Err(err) => {
			warn!(error = %err, path = %path.display(), "cannot read stored ACME account credentials, registering a new account");
			return None;
		}
	};
	let stored: StoredAccount = match serde_json::from_str(&raw) {
		Ok(stored) => stored,
		Err(err) => {
			warn!(error = %err, path = %path.display(), "stored ACME account credentials are unreadable, registering a new account");
			return None;
		}
	};

	if stored.directory_url != directory_url {
		warn!(
			stored = %stored.directory_url,
			requested = %directory_url,
			"stored ACME account belongs to a different directory, registering a new account"
		);
		return None;
	}

	Some(stored.credentials)
}

/// Persist account credentials so later runs (including renewals) reuse the
/// same account instead of registering a new one each time.
async fn store_credentials(path: &Path, directory_url: &str, credentials: AccountCredentials) -> Result<()> {
	if let Some(parent) = path.parent() {
		tokio::fs::create_dir_all(parent).await?;
	}

	let stored = StoredAccount {
		directory_url: directory_url.to_owned(),
		credentials,
	};
	let encoded = serde_json::to_vec_pretty(&stored).context("serialize ACME account credentials")?;
	crate::write_key_file(path, &encoded).await
}

/// Register a new ACME account and persist its credentials.
///
/// A failure to persist is reported but not fatal: the account already exists
/// on the server, and aborting here would only leave it orphaned.
async fn create_and_store_account(contact: &[String], directory_url: &str, credentials_path: &Path) -> Result<Account> {
	let (account, credentials) = Account::builder()?
		.create(
			&NewAccount {
				contact: &contact.iter().map(String::as_str).collect::<Vec<_>>(),
				terms_of_service_agreed: true,
				only_return_existing: false,
			},
			directory_url.to_owned(),
			None,
		)
		.await
		.context("Failed to create ACME account")?;

	if let Err(err) = store_credentials(credentials_path, directory_url, credentials).await {
		warn!(
			error = %err,
			path = %credentials_path.display(),
			"failed to persist ACME account credentials; a later run may register another account"
		);
	}

	Ok(account)
}

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

		// Register the response before telling Let's Encrypt the challenge is
		// ready.
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
///
/// The ACME account credentials are persisted next to `key_path` (see
/// [`account_credentials_path`]) and reused on subsequent calls, so renewals do
/// not register a new account every time. Credentials belonging to a different
/// ACME directory (staging vs. production) are ignored rather than reused.
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

	let credentials_path = account_credentials_path(key_path);
	let account = match load_stored_credentials(&credentials_path, &directory_url).await {
		Some(credentials) => match Account::builder()?.from_credentials(credentials).await {
			Ok(account) => {
				info!(path = %credentials_path.display(), "reusing stored ACME account credentials");
				account
			}
			Err(err) => {
				warn!(
					error = %err,
					path = %credentials_path.display(),
					"stored ACME account credentials are unusable, registering a new account"
				);
				create_and_store_account(&contact, &directory_url, &credentials_path).await?
			}
		},
		None => create_and_store_account(&contact, &directory_url, &credentials_path).await?,
	};

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

	/// Synthetic credentials shaped exactly like `AccountCredentials`
	/// serializes (base64url PKCS#8 key). Deserialization only base64-decodes
	/// the key, so the account-reuse path is covered without valid key material
	/// and without contacting a real CA.
	const SYNTHETIC_CREDENTIALS: &str =
		r#"{"id":"https://acme.test/acct/1","key_pkcs8":"AAECAwQFBgcICQ","directory":"https://acme.test/directory"}"#;

	const STAGING_DIRECTORY: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
	const PRODUCTION_DIRECTORY: &str = "https://acme-v02.api.letsencrypt.org/directory";

	fn synthetic_credentials() -> AccountCredentials {
		serde_json::from_str(SYNTHETIC_CREDENTIALS).expect("synthetic credentials deserialize")
	}

	fn unique_temp_dir(label: &str) -> PathBuf {
		let dir = std::env::temp_dir().join(format!("wind-acme-http01-{}-{label}", std::process::id()));
		let _ = std::fs::remove_dir_all(&dir);
		dir
	}

	#[test]
	fn account_credentials_path_sits_next_to_the_key_file() {
		assert_eq!(
			account_credentials_path(Path::new("/etc/wind/example.com.key")),
			PathBuf::from("/etc/wind/example.com.account.json")
		);
		assert_eq!(account_credentials_path(Path::new("key")), PathBuf::from("key.account.json"));
	}

	#[tokio::test]
	async fn stored_account_credentials_are_reused_for_the_same_directory() {
		let dir = unique_temp_dir("reuse");
		let path = account_credentials_path(&dir.join("example.com.key"));

		store_credentials(&path, PRODUCTION_DIRECTORY, synthetic_credentials())
			.await
			.unwrap();

		let loaded = load_stored_credentials(&path, PRODUCTION_DIRECTORY)
			.await
			.expect("a stored account for the same directory must be reused");
		assert_eq!(
			serde_json::to_value(&loaded).unwrap(),
			serde_json::to_value(synthetic_credentials()).unwrap()
		);

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[tokio::test]
	async fn stored_account_credentials_are_ignored_for_another_directory() {
		let dir = unique_temp_dir("directory-mismatch");
		let path = account_credentials_path(&dir.join("example.com.key"));

		store_credentials(&path, STAGING_DIRECTORY, synthetic_credentials())
			.await
			.unwrap();

		assert!(load_stored_credentials(&path, PRODUCTION_DIRECTORY).await.is_none());
		assert!(load_stored_credentials(&path, STAGING_DIRECTORY).await.is_some());

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[tokio::test]
	async fn missing_account_credentials_file_means_a_new_account() {
		let dir = unique_temp_dir("missing");
		let path = account_credentials_path(&dir.join("example.com.key"));

		assert!(load_stored_credentials(&path, PRODUCTION_DIRECTORY).await.is_none());
	}

	#[tokio::test]
	async fn damaged_account_credentials_file_means_a_new_account() {
		let dir = unique_temp_dir("damaged");
		let path = account_credentials_path(&dir.join("example.com.key"));
		tokio::fs::create_dir_all(&dir).await.unwrap();
		tokio::fs::write(&path, b"{ not json").await.unwrap();

		assert!(load_stored_credentials(&path, PRODUCTION_DIRECTORY).await.is_none());

		// The replacement account overwrites the damaged file, so the file
		// heals instead of forcing a new account on every later run.
		store_credentials(&path, PRODUCTION_DIRECTORY, synthetic_credentials())
			.await
			.unwrap();
		assert!(load_stored_credentials(&path, PRODUCTION_DIRECTORY).await.is_some());

		let _ = std::fs::remove_dir_all(&dir);
	}

	#[cfg(unix)]
	#[tokio::test]
	async fn stored_account_credentials_are_owner_only() {
		use std::os::unix::fs::PermissionsExt;

		let dir = unique_temp_dir("permissions");
		let path = account_credentials_path(&dir.join("example.com.key"));
		store_credentials(&path, PRODUCTION_DIRECTORY, synthetic_credentials())
			.await
			.unwrap();

		let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
		assert_eq!(mode, 0o600);

		let _ = std::fs::remove_dir_all(&dir);
	}
}
