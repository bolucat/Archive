use std::sync::Arc;

use rustls::{
	crypto::CryptoProvider,
	pki_types::{CertificateDer, ServerName, UnixTime},
};
use tracing::warn;

use crate::{Error, quinn::outbound::TuicOutboundOpts};

#[allow(clippy::result_large_err)]
pub(crate) fn tls_config(_servername: &str, opts: &TuicOutboundOpts) -> Result<rustls::ClientConfig, Error> {
	use rustls::ClientConfig;
	use rustls_platform_verifier::BuilderVerifierExt;

	let arc_crypto_provider = CryptoProvider::get_default().expect("Unable to find default crypto provider");
	let mut config = if opts.skip_cert_verify {
		warn!(
			target: "tls",
			"skip_cert_verify=true: server certificate verification is DISABLED. \
			 This is insecure and allows trivial MITM of the upstream relay."
		);
		ClientConfig::builder()
			.dangerous()
			.with_custom_certificate_verifier(SkipServerVerification::new())
			.with_no_client_auth()
	} else {
		ClientConfig::builder_with_provider(arc_crypto_provider.clone())
			.with_protocol_versions(&[&rustls::version::TLS13])?
			.with_platform_verifier()?
			.with_no_client_auth()
	};

	// Honour caller-supplied ALPN list. Empty list falls back to "h3" for backward
	// compatibility with existing deployments; previously this was hardcoded and
	// silently ignored `opts.alpn`.
	let mut alpn: Vec<Vec<u8>> = opts.alpn.iter().map(|a| a.as_bytes().to_vec()).collect();
	if alpn.is_empty() {
		alpn.push(b"h3".to_vec());
	}
	config.alpn_protocols = alpn;

	// 0-RTT: without `enable_early_data` on the rustls client config, quinn never
	// attempts to send early data even when the server accepts it (see the quinn
	// `QuicClientConfig` docs). Wire the caller's `zero_rtt_handshake` flag through
	// so a resumed handshake can actually replay early data.
	config.enable_early_data = opts.zero_rtt_handshake;

	Ok(config)
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
	fn new() -> Arc<Self> {
		Arc::new(Self(
			rustls::crypto::CryptoProvider::get_default()
				.expect("CryptoProvider not found")
				.clone(),
		))
	}
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
	fn verify_server_cert(
		&self,
		_end_entity: &CertificateDer<'_>,
		_intermediates: &[CertificateDer<'_>],
		_server_name: &ServerName<'_>,
		_ocsp: &[u8],
		_now: UnixTime,
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

#[cfg(test)]
mod tests {
	use std::{net::SocketAddr, sync::Arc, time::Duration};

	use uuid::Uuid;

	use super::*;
	use crate::quinn::outbound::TuicOutboundOpts;

	fn install_provider() {
		// Idempotent — install_default returns Err once the global is set.
		#[cfg(feature = "aws-lc-rs")]
		let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
		#[cfg(feature = "ring")]
		let _ = rustls::crypto::ring::default_provider().install_default();
	}

	fn opts_with(alpn: Vec<String>) -> TuicOutboundOpts {
		TuicOutboundOpts {
			peer_addr: "127.0.0.1:9443".parse::<SocketAddr>().unwrap(),
			sni: "localhost".into(),
			auth: (Uuid::nil(), Arc::<[u8]>::from(&[][..])),
			zero_rtt_handshake: false,
			heartbeat: Duration::from_secs(10),
			gc_interval: Duration::from_secs(10),
			gc_lifetime: Duration::from_secs(10),
			// Use the skip-verify path so the test does not depend on a working
			// platform-verifier (which may not have access to the system trust
			// store in restricted CI environments). The ALPN logic is shared
			// between both branches.
			skip_cert_verify: true,
			alpn,
			reconnect: crate::quinn::outbound::ReconnectConfig::default(),
		}
	}

	#[test]
	fn alpn_honours_caller_supplied_list() {
		install_provider();
		let opts = opts_with(vec!["tuic".into(), "h3".into()]);
		let cfg = tls_config("localhost", &opts).expect("tls_config must succeed");
		assert_eq!(
			cfg.alpn_protocols,
			vec![b"tuic".to_vec(), b"h3".to_vec()],
			"ALPN list must be taken from opts.alpn verbatim"
		);
	}

	#[test]
	fn alpn_falls_back_to_h3_when_empty() {
		install_provider();
		let opts = opts_with(Vec::new());
		let cfg = tls_config("localhost", &opts).expect("tls_config must succeed");
		assert_eq!(
			cfg.alpn_protocols,
			vec![b"h3".to_vec()],
			"empty opts.alpn must fall back to a single h3 entry"
		);
	}

	#[test]
	fn alpn_does_not_silently_inject_h3_when_caller_specified_something_else() {
		install_provider();
		let opts = opts_with(vec!["my-protocol".into()]);
		let cfg = tls_config("localhost", &opts).expect("tls_config must succeed");
		assert_eq!(cfg.alpn_protocols, vec![b"my-protocol".to_vec()]);
		assert!(
			!cfg.alpn_protocols.contains(&b"h3".to_vec()),
			"hardcoded \"h3\" must no longer override the caller's ALPN choice"
		);
	}

	#[test]
	fn enable_early_data_tracks_zero_rtt_handshake() {
		install_provider();

		let mut off = opts_with(vec!["h3".into()]);
		off.zero_rtt_handshake = false;
		let cfg_off = tls_config("localhost", &off).expect("tls_config must succeed");
		assert!(
			!cfg_off.enable_early_data,
			"0-RTT must stay disabled on the rustls client config when zero_rtt_handshake=false"
		);

		let mut on = opts_with(vec!["h3".into()]);
		on.zero_rtt_handshake = true;
		let cfg_on = tls_config("localhost", &on).expect("tls_config must succeed");
		assert!(
			cfg_on.enable_early_data,
			"zero_rtt_handshake=true must enable enable_early_data on the rustls client config"
		);
	}
}
