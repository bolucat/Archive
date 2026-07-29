//! Self-signed certificate generation for backends that load TLS material from
//! file paths (e.g. the QUIC listeners) and for local development.

use std::path::Path;

use eyre::Result;
use tokio::fs;

/// Generate a self-signed certificate and write it to disk if the files don't
/// already exist. The QUIC listener loads TLS material from file paths, so both
/// backends consume the same on-disk PEMs.
pub async fn ensure_self_signed_cert_files(hostname: &str, cert_path: &Path, key_path: &Path) -> Result<()> {
	// Reuse the existing pair only if the cert is still fresh. These certs have a
	// ~45-day validity, so returning early merely because the files exist would
	// serve an expired cert forever after the first 45 days.
	if cert_path.exists()
		&& key_path.exists()
		&& let Ok(cert_pem) = fs::read(cert_path).await
		&& let Ok(not_after) = crate::http01::cert_not_after(&cert_pem)
		&& !crate::http01::should_renew(not_after, time::OffsetDateTime::now_utc())
	{
		return Ok(());
	}
	let (cert, key_pair) = generate_short_lived_self_signed(hostname)?;
	let cert_pem = cert.pem();
	let key_pem = key_pair.serialize_pem();

	if let Some(parent) = cert_path.parent() {
		fs::create_dir_all(parent).await?;
	}
	if let Some(parent) = key_path.parent() {
		fs::create_dir_all(parent).await?;
	}
	fs::write(cert_path, cert_pem.as_bytes()).await?;
	crate::write_key_file(key_path, key_pem.as_bytes()).await?;
	Ok(())
}

/// Generate a self-signed leaf certificate (ECDSA P-256, ~45-day validity).
///
/// Uses `is_ca = false` so the leaf is a plain end-entity TLS server cert, and
/// a short validity period (≤398 days) to satisfy Chromium's certificate
/// validation requirements.
fn generate_short_lived_self_signed(hostname: &str) -> Result<(rcgen::Certificate, rcgen::KeyPair)> {
	let mut params = rcgen::CertificateParams::new(vec![hostname.to_owned()])
		.map_err(|e| eyre::eyre!("creating certificate params: {e}"))?;
	params.is_ca = rcgen::IsCa::NoCa;
	params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(45);
	let key_pair =
		rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|e| eyre::eyre!("generating key pair: {e}"))?;
	let cert = params
		.self_signed(&key_pair)
		.map_err(|e| eyre::eyre!("generating self-signed certificate: {e}"))?;
	Ok((cert, key_pair))
}
