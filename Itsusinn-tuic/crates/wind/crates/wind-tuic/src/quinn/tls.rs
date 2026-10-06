use std::sync::Arc;

use rustls::{
	crypto::CryptoProvider,
	pki_types::{CertificateDer, ServerName, UnixTime},
};
use tracing::warn;

use crate::{Error, quinn::outbound::TuicOutboundOpts};

/// Resolve the process-wide rustls crypto provider, returning an error instead
/// of panicking when none can be obtained.
///
/// The global provider is normally installed by the caller
/// ([`TuicOutbound::new`](crate::quinn::outbound::TuicOutbound::new)) or by any
/// other rustls user in the process, so [`CryptoProvider::get_default`] usually
/// succeeds. It must nevertheless stay a recoverable error rather than an
/// `expect`: a consumer that drives [`tls_config`] directly (or links a rustls
/// user that never installs a provider) used to abort the process here. When
/// nothing is installed globally we install the feature-selected default
/// exactly once and fall back to it.
fn resolve_crypto_provider() -> Result<Arc<CryptoProvider>, Error> {
	// A provider already installed by this crate, another rustls user, or an
	// earlier call is authoritative; `install_default` can only ever succeed
	// once per process, so never try to replace it.
	if let Some(installed) = CryptoProvider::get_default() {
		return Ok(installed.clone());
	}

	// `build` mirrors the fallback `ClientConfig::builder()` would pick on its
	// own. It yields the bare provider so `install_default`, which consumes
	// `self`, can take ownership without racing on the shared `Arc`. When both
	// provider features are enabled the `aws-lc-rs` arm wins, keeping the same
	// precedence as the provider installation in `TuicOutbound::new`.
	#[cfg(any(feature = "aws-lc-rs", feature = "ring"))]
	let build = || -> CryptoProvider {
		#[cfg(feature = "aws-lc-rs")]
		{
			rustls::crypto::aws_lc_rs::default_provider()
		}
		#[cfg(all(feature = "ring", not(feature = "aws-lc-rs")))]
		{
			rustls::crypto::ring::default_provider()
		}
	};

	#[cfg(any(feature = "aws-lc-rs", feature = "ring"))]
	{
		// Installing is only a race-safe way to make sure *some* provider
		// exists; read the global back either way, because
		// `install_default` returns the losing implementation when
		// another thread installed first and only the global one is
		// then shared by the config builder and the cert verifier.
		match build().install_default() {
			Ok(()) => Ok(CryptoProvider::get_default()
				.cloned()
				.ok_or_else(|| eyre::eyre!("no default rustls CryptoProvider is available"))?),
			Err(_) => CryptoProvider::get_default()
				.cloned()
				.ok_or_else(|| eyre::eyre!("no default rustls CryptoProvider is available")),
		}
	}

	// Neither provider feature is enabled, so there is nothing to install and
	// no way to construct one: report a recoverable error instead of
	// panicking.
	#[cfg(not(any(feature = "aws-lc-rs", feature = "ring")))]
	{
		Err(eyre::eyre!("no default rustls CryptoProvider is available"))
	}
}

#[allow(clippy::result_large_err)]
pub(crate) fn tls_config(_servername: &str, opts: &TuicOutboundOpts) -> Result<rustls::ClientConfig, Error> {
	use rustls::ClientConfig;
	use rustls_platform_verifier::BuilderVerifierExt;

	let arc_crypto_provider = resolve_crypto_provider()?;
	let mut config = if opts.skip_cert_verify {
		warn!(
			target: "tls",
			"skip_cert_verify=true: server certificate verification is DISABLED. \
			 This is insecure and allows trivial MITM of the upstream relay."
		);
		// `ClientConfig::builder()` resolves the global provider itself and
		// panics when there is none; build the provider-explicit
		// variant instead so the dangerous path shares the resolved
		// provider and cannot abort.
		ClientConfig::builder_with_provider(arc_crypto_provider.clone())
			.with_protocol_versions(&[&rustls::version::TLS13])?
			.dangerous()
			.with_custom_certificate_verifier(SkipServerVerification::new(arc_crypto_provider))
			.with_no_client_auth()
	} else {
		ClientConfig::builder_with_provider(arc_crypto_provider.clone())
			.with_protocol_versions(&[&rustls::version::TLS13])?
			.with_platform_verifier()?
			.with_no_client_auth()
	};

	// Honour caller-supplied ALPN list. Empty list falls back to "h3" for
	// backward compatibility with existing deployments; previously this was
	// hardcoded and silently ignored `opts.alpn`.
	let mut alpn: Vec<Vec<u8>> = opts.alpn.iter().map(|a| a.as_bytes().to_vec()).collect();
	if alpn.is_empty() {
		alpn.push(b"h3".to_vec());
	}
	config.alpn_protocols = alpn;

	// 0-RTT: without `enable_early_data` on the rustls client config, quinn
	// never attempts to send early data even when the server accepts it (see
	// the quinn `QuicClientConfig` docs). Wire the caller's
	// `zero_rtt_handshake` flag through so a resumed handshake can actually
	// replay early data.
	config.enable_early_data = opts.zero_rtt_handshake;

	Ok(config)
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
	/// The provider supplies the signature-verification algorithms used by
	/// `verify_tls12_signature`/`verify_tls13_signature`; it is passed in
	/// rather than looked up globally so this path cannot panic on a
	/// missing default.
	fn new(provider: Arc<rustls::crypto::CryptoProvider>) -> Arc<Self> {
		Arc::new(Self(provider))
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
			peer_resolver: None,
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
			client_config: None,
			congestion_control: crate::quinn::CongestionControl::Bbr,
			max_concurrent_bi_streams: None,
			max_concurrent_uni_streams: None,
			send_window: None,
			stream_receive_window: None,
			max_idle_time: None,
			udp_relay_mode: crate::quinn::UdpRelayMode::Native,
			socket_factory: None,
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

	/// Marks the child process spawned by
	/// [`tls_config_without_a_preinstalled_crypto_provider_does_not_panic`].
	const CHILD_ENV: &str = "WIND_TUIC_TLS_NO_PROVIDER_CHILD";

	/// A process-wide rustls global cannot be uninstalled, so the "no default
	/// provider is installed" state W38 describes can only be built in a fresh
	/// process. The parent runs this test worker with `--test-threads=1`, which
	/// confines `CHILD_ENV` to this thread. Both conditions are required: the
	/// cargo/libtest environment is free to have `CHILD_ENV` set for unrelated
	/// reasons, and that must not silently turn the parent assertion into a
	/// no-op.
	fn running_as_no_provider_child() -> bool {
		std::env::var(CHILD_ENV).is_ok()
			&& std::thread::current()
				.name()
				.is_some_and(|n| n.ends_with("no_provider_child_worker"))
	}

	/// Child half of
	/// [`tls_config_without_a_preinstalled_crypto_provider_does_not_panic`]:
	/// builds the real client config with no default crypto provider installed.
	/// Before the fix this panicked inside
	/// `CryptoProvider::get_default().expect(..)`.
	#[test]
	fn no_provider_child_worker() {
		if !running_as_no_provider_child() {
			return;
		}

		let rt = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.expect("current-thread runtime");
		let cfg = rt.block_on(async { tls_config("localhost", &opts_with(vec!["h3".into()])) });
		// The worker must fail the child (non-zero exit) instead of aborting.
		let cfg = cfg.expect("tls_config must succeed without a preinstalled default provider");
		assert_eq!(cfg.alpn_protocols, vec![b"h3".to_vec()]);
	}

	#[test]
	fn tls_config_without_a_preinstalled_crypto_provider_does_not_panic() {
		let exe = std::env::current_exe().expect("test executable path");
		let output = std::process::Command::new(exe)
			.args(["no_provider_child_worker", "--test-threads=1", "--nocapture"])
			.env(CHILD_ENV, "1")
			.output()
			.expect("spawn the child test worker");

		let stdout = String::from_utf8_lossy(&output.stdout);
		let stderr = String::from_utf8_lossy(&output.stderr);
		assert!(
			output.status.success(),
			"building a TUIC client config without a default rustls CryptoProvider must not panic; \
			 status={:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
			output.status
		);
		assert!(
			stdout.contains("1 passed"),
			"child worker did not report a passing test:\nstdout:\n{stdout}"
		);
	}

	/// The config the skip-verify branch returns must be built from the same
	/// provider its cert verifier holds; building the dangerous path with a
	/// provider-less builder would panic, and a mismatch between the two would
	/// make signature verification inconsistent.
	#[test]
	fn skip_verify_config_uses_the_resolved_crypto_provider() {
		install_provider();
		let opts = opts_with(vec!["h3".into()]);
		let cfg = tls_config("localhost", &opts).expect("tls_config must succeed");

		let global = CryptoProvider::get_default().expect("a default provider must be installed after tls_config");
		let expected = global.signature_verification_algorithms.supported_schemes();
		assert_eq!(
			cfg.crypto_provider().signature_verification_algorithms.supported_schemes(),
			expected,
			"the returned config must be built from the resolved process-wide provider"
		);
	}
}
