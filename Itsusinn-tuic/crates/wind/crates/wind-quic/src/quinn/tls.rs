//! rustls config construction for the quinn backend.

use std::sync::{Arc, OnceLock};

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::{
	config::{CertSource, ClientTlsConfig, ServerTlsConfig, TransportConfig},
	error::QuicError,
};

/// Install the process-wide rustls crypto provider exactly once.
///
/// `install_default` returns `Err` after the first call (the global is already
/// set), so a `OnceLock` is the race-free single-init primitive.
pub(super) fn ensure_provider() {
	static INSTALLED: OnceLock<()> = OnceLock::new();
	INSTALLED.get_or_init(|| {
		let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
	});
}

fn load_certs_and_key(src: &CertSource) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), QuicError> {
	let (cert_pem, key_pem): (Vec<u8>, Vec<u8>) = match src {
		CertSource::PemPaths { cert, key } => {
			let c = std::fs::read(cert).map_err(|e| QuicError::Tls(format!("reading certificate {cert}: {e}")))?;
			let k = std::fs::read(key).map_err(|e| QuicError::Tls(format!("reading private key {key}: {e}")))?;
			(c, k)
		}
		CertSource::PemBytes { cert, key } => (cert.clone(), key.clone()),
	};

	let certs = rustls_pemfile::certs(&mut cert_pem.as_slice())
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| QuicError::Tls(format!("parsing certificate chain: {e}")))?;
	if certs.is_empty() {
		return Err(QuicError::Tls("certificate PEM contained no certificates".into()));
	}
	let key = rustls_pemfile::private_key(&mut key_pem.as_slice())
		.map_err(|e| QuicError::Tls(format!("parsing private key: {e}")))?
		.ok_or_else(|| QuicError::Tls("private key PEM contained no key".into()))?;
	Ok((certs, key))
}

pub(super) fn server_crypto(cfg: &ServerTlsConfig, transport: &TransportConfig) -> Result<rustls::ServerConfig, QuicError> {
	let (certs, key) = load_certs_and_key(&cfg.cert)?;
	let mut crypto = rustls::ServerConfig::builder()
		.with_no_client_auth()
		.with_single_cert(certs, key)
		.map_err(|e| QuicError::Tls(format!("server with_single_cert: {e}")))?;
	crypto.alpn_protocols = transport.alpn.clone();
	crypto.max_early_data_size = if transport.enable_0rtt { u32::MAX } else { 0 };
	Ok(crypto)
}

pub(super) fn client_crypto(cfg: &ClientTlsConfig) -> Result<rustls::ClientConfig, QuicError> {
	use rustls_platform_verifier::BuilderVerifierExt as _;

	let provider = rustls::crypto::CryptoProvider::get_default()
		.ok_or_else(|| QuicError::Tls("no default crypto provider installed".into()))?
		.clone();

	let mut crypto = if cfg.verify_certificate {
		rustls::ClientConfig::builder_with_provider(provider)
			.with_protocol_versions(&[&rustls::version::TLS13])
			.map_err(|e| QuicError::Tls(format!("client protocol versions: {e}")))?
			.with_platform_verifier()
			.map_err(|e| QuicError::Tls(format!("client platform verifier: {e}")))?
			.with_no_client_auth()
	} else {
		tracing::warn!(
			target: "wind_quic::tls",
			"verify_certificate=false: server certificate verification is DISABLED (insecure, MITM-able)"
		);
		rustls::ClientConfig::builder()
			.dangerous()
			.with_custom_certificate_verifier(SkipServerVerification::new(provider))
			.with_no_client_auth()
	};
	crypto.alpn_protocols = cfg.alpn.clone();
	Ok(crypto)
}

/// A certificate verifier that accepts any server certificate. Used only when
/// `ClientTlsConfig::verify_certificate` is explicitly `false`.
#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
	fn new(provider: Arc<rustls::crypto::CryptoProvider>) -> Arc<Self> {
		Arc::new(Self(provider))
	}
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
	fn verify_server_cert(
		&self,
		_end_entity: &CertificateDer<'_>,
		_intermediates: &[CertificateDer<'_>],
		_server_name: &rustls::pki_types::ServerName<'_>,
		_ocsp: &[u8],
		_now: rustls::pki_types::UnixTime,
	) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
		Ok(rustls::client::danger::ServerCertVerified::assertion())
	}

	fn verify_tls12_signature(
		&self,
		message: &[u8],
		cert: &CertificateDer<'_>,
		dss: &rustls::DigitallySignedStruct,
	) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
		rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
	}

	fn verify_tls13_signature(
		&self,
		message: &[u8],
		cert: &CertificateDer<'_>,
		dss: &rustls::DigitallySignedStruct,
	) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
		rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
	}

	fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
		self.0.signature_verification_algorithms.supported_schemes()
	}
}
