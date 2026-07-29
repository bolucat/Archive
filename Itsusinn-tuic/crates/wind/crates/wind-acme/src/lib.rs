//! ACME certificate management for the wind proxy framework.
//!
//! Two strategies are provided behind features:
//!
//! - `resolver` (default): a [`rustls_acme`]-backed background renewal state
//!   machine returning a `rustls` cert resolver — see [`resolver`]. Used by the
//!   rustls-based (quinn) backend.
//! - `http01`: one-shot HTTP-01 provisioning that writes the PEM chain/key to
//!   disk ([`http01`]) plus self-signed generation ([`selfsigned`]). Used by
//!   backends that load TLS material from files (quiche/tokio-quiche).
//!
//! [`is_valid_domain`] is always available.

/// One-shot HTTP-01 provisioning that writes PEM cert/key files to disk.
#[cfg(feature = "http01")]
pub mod http01;
/// `rustls-acme`-backed resolver flow with background renewal.
#[cfg(feature = "resolver")]
pub mod resolver;
/// Self-signed certificate generation to disk.
#[cfg(feature = "http01")]
pub mod selfsigned;

#[cfg(feature = "resolver")]
pub use resolver::{CertPem, start_acme, start_acme_with_cert};

/// Write a TLS private key to disk with owner-only permissions.
///
/// On Unix the file is restricted to mode `0600` so other local users cannot
/// read the key (a plain `fs::write` leaves it at the process umask, typically
/// `0644`). On other platforms this is a normal write.
#[cfg(feature = "http01")]
pub(crate) async fn write_key_file(path: &std::path::Path, contents: &[u8]) -> eyre::Result<()> {
	tokio::fs::write(path, contents).await?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;

		tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
	}
	Ok(())
}

/// Check if a domain name is valid for ACME certificate issuance.
pub fn is_valid_domain(hostname: &str) -> bool {
	if hostname.is_empty() || hostname.len() > 253 {
		return false;
	}

	hostname.split('.').all(|label| {
		!label.is_empty()
			&& label.len() <= 63
			&& label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
			&& !label.starts_with('-')
			&& !label.ends_with('-')
	}) && hostname.contains('.')
		&& !hostname.starts_with('.')
		&& !hostname.ends_with('.')
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_domain_validation() {
		assert!(is_valid_domain("example.com"));
		assert!(is_valid_domain("sub.domain.co.uk"));
		assert!(is_valid_domain("a-b.c-d.com"));
		assert!(is_valid_domain("xn--eckwd4c7c.xn--zckzah.jp"));

		assert!(!is_valid_domain(".leading.dot"));
		assert!(!is_valid_domain("trailing.dot."));
		assert!(!is_valid_domain("double..dot"));
		assert!(!is_valid_domain("-leading-hyphen.com"));
		assert!(!is_valid_domain("trailing-hyphen-.com"));
		assert!(!is_valid_domain("space in.domain"));
		assert!(!is_valid_domain(""));
		assert!(!is_valid_domain(&"a".repeat(254)));
		assert!(!is_valid_domain("no-tld"));
	}
}
