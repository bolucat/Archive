use crate::acceptor::AcmeAcceptor;
use crate::acme::{Account, AcmeError, Auth, AuthStatus, Directory, Order, OrderStatus, ACME_TLS_ALPN_NAME};
use crate::{any_ecdsa_type, crypto_provider, AcmeConfig, Incoming, ResolvesServerCertAcme, UseChallenge};
use async_io::Timer;
use chrono::{DateTime, TimeZone, Utc};
use core::fmt;
use futures::prelude::*;
use futures::ready;
use futures_rustls::pki_types::{CertificateDer as RustlsCertificate, PrivateKeyDer, PrivatePkcs8KeyDer};
use futures_rustls::rustls::crypto::CryptoProvider;
use futures_rustls::rustls::sign::CertifiedKey;
use futures_rustls::rustls::ServerConfig;
use rcgen::{CertificateParams, DistinguishedName, KeyPair, PKCS_ECDSA_P256_SHA256};
use std::convert::Infallible;
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use thiserror::Error;
use x509_parser::parse_x509_certificate;

#[cfg(doc)]
use crate::is_tls_alpn_challenge;
#[cfg(doc)]
use crate::rustls;

#[allow(clippy::type_complexity)]
pub struct AcmeState<EC: Debug = Infallible, EA: Debug = EC> {
    config: Arc<AcmeConfig<EC, EA>>,
    resolver: Arc<ResolvesServerCertAcme>,
    account_key: Option<Vec<u8>>,

    early_action: Option<Pin<Box<dyn Future<Output = Event<EC, EA>> + Send>>>,
    load_cert: Option<Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, EC>> + Send>>>,
    load_account: Option<Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>, EA>> + Send>>>,
    order: Option<Pin<Box<dyn Future<Output = Result<Vec<u8>, OrderError>> + Send>>>,
    order_events: Option<futures::channel::mpsc::UnboundedReceiver<EventOk>>,
    backoff_cnt: usize,
    wait: Option<Timer>,
    current_cert: Option<Certificate>,
}

impl<EC: 'static + Debug, EA: 'static + Debug> fmt::Debug for AcmeState<EC, EA> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AcmeState").field("config", &self.config).finish_non_exhaustive()
    }
}

pub type Event<EC, EA> = Result<EventOk, EventError<EC, EA>>;

#[derive(Debug, Clone)]
pub struct Certificate {
    pem: String,
}

impl Certificate {
    /// Returns the full PEM formatted string of the certificate (including the private key, if present).
    pub fn pem(&self) -> &str {
        &self.pem
    }

    /// Returns the private key in PEM format.
    pub fn private_key_pem(&self) -> Option<&str> {
        let marker = "-----BEGIN CERTIFICATE-----";
        self.pem.find(marker).map(|idx| self.pem[..idx].trim())
    }

    /// Returns the full certificate chain in PEM format.
    pub fn fullchain_pem(&self) -> Option<&str> {
        let marker = "-----BEGIN CERTIFICATE-----";
        self.pem.find(marker).map(|idx| self.pem[idx..].trim())
    }

    /// Returns only the leaf certificate in PEM format.
    pub fn leaf_cert_pem(&self) -> Option<&str> {
        let start_marker = "-----BEGIN CERTIFICATE-----";
        let end_marker = "-----END CERTIFICATE-----";
        let start_idx = self.pem.find(start_marker)?;
        let end_idx = self.pem[start_idx..].find(end_marker)?;
        Some(self.pem[start_idx..start_idx + end_idx + end_marker.len()].trim())
    }
}

#[derive(Debug, Clone)]
pub enum EventOk {
    DeployedCachedCert(Certificate),
    DeployedNewCert(Certificate),
    CertCacheStore,
    AccountCacheStore,
    ValidationChallenge(crate::acme::Challenge),
}

#[derive(Error, Debug)]
pub enum EventError<EC: Debug, EA: Debug> {
    #[error("cert cache load: {0}")]
    CertCacheLoad(EC),
    #[error("account cache load: {0}")]
    AccountCacheLoad(EA),
    #[error("cert cache store: {0}")]
    CertCacheStore(EC),
    #[error("account cache store: {0}")]
    AccountCacheStore(EA),
    #[error("cached cert parse: {0}")]
    CachedCertParse(CertParseError),
    #[error("order: {0}")]
    Order(OrderError),
    #[error("new cert parse: {0}")]
    NewCertParse(CertParseError),
}

#[derive(Error, Debug)]
pub enum OrderError {
    #[error("acme error: {0}")]
    Acme(#[from] AcmeError),
    #[error("certificate generation error: {0}")]
    Rcgen(#[from] rcgen::Error),
    #[error("bad order object: {0:?}")]
    BadOrder(Order),
    #[error("bad auth object: {0:?}")]
    BadAuth(Auth),
    #[error("authorization for {0} failed too many times")]
    TooManyAttemptsAuth(String),
    #[error("order status stayed on processing too long")]
    ProcessingTimeout(Order),
}

#[derive(Error, Debug)]
pub enum CertParseError {
    #[error("X509 parsing error: {0}")]
    X509(#[from] x509_parser::nom::Err<x509_parser::error::X509Error>),
    #[error("expected 2 or more pem, got: {0}")]
    Pem(#[from] pem::PemError),
    #[error("expected 2 or more pem, got: {0}")]
    TooFewPem(usize),
    #[error("unsupported private key type")]
    InvalidPrivateKey,
}

impl<EC: 'static + Debug, EA: 'static + Debug> AcmeState<EC, EA> {
    pub fn incoming<TCP: AsyncRead + AsyncWrite + Unpin, ETCP, ITCP: Stream<Item = Result<TCP, ETCP>> + Unpin>(
        self,
        tcp_incoming: ITCP,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> Incoming<TCP, ETCP, ITCP, EC, EA> {
        #[allow(deprecated)]
        let acceptor = self.acceptor();
        Incoming::new(tcp_incoming, self, acceptor, alpn_protocols)
    }
    #[deprecated(note = "please use high-level API via `AcmeState::incoming()` instead or refer to updated low-level API examples")]
    #[allow(deprecated)]
    pub fn acceptor(&self) -> AcmeAcceptor {
        AcmeAcceptor::new(self.resolver())
    }
    #[cfg(feature = "tokio")]
    pub fn tokio_incoming<
        TokioTCP: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
        ETCP,
        TokioITCP: Stream<Item = Result<TokioTCP, ETCP>> + Unpin,
    >(
        self,
        tcp_incoming: TokioITCP,
        alpn_protocols: Vec<Vec<u8>>,
    ) -> crate::tokio::TokioIncoming<
        tokio_util::compat::Compat<TokioTCP>,
        ETCP,
        crate::tokio::TokioIncomingTcpWrapper<TokioTCP, ETCP, TokioITCP>,
        EC,
        EA,
    > {
        let tcp_incoming = crate::tokio::TokioIncomingTcpWrapper::from(tcp_incoming);
        crate::tokio::TokioIncoming::from(self.incoming(tcp_incoming, alpn_protocols))
    }
    #[cfg(feature = "axum")]
    pub fn axum_acceptor(&self, rustls_config: Arc<ServerConfig>) -> crate::axum::AxumAcceptor {
        #[allow(deprecated)]
        crate::axum::AxumAcceptor::new(self.acceptor(), rustls_config)
    }

    #[cfg(feature = "tower")]
    pub fn http01_challenge_tower_service(&self) -> crate::tower::TowerHttp01ChallengeService {
        crate::tower::TowerHttp01ChallengeService(self.resolver.clone())
    }

    pub fn resolver(&self) -> Arc<ResolvesServerCertAcme> {
        self.resolver.clone()
    }

    /// Creates a [rustls::ServerConfig] for TLS-ALPN-01 challenge connections. Use this if [is_tls_alpn_challenge] returns `true`.
    #[cfg(any(feature = "ring", feature = "aws-lc-rs"))]
    pub fn challenge_rustls_config(&self) -> Arc<ServerConfig> {
        self.challenge_rustls_config_with_provider(crypto_provider().into())
    }
    /// Same as [AcmeState::challenge_rustls_config], with a specific [CryptoProvider].
    pub fn challenge_rustls_config_with_provider(&self, provider: Arc<CryptoProvider>) -> Arc<ServerConfig> {
        let mut rustls_config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_cert_resolver(self.resolver());
        rustls_config.alpn_protocols.push(ACME_TLS_ALPN_NAME.to_vec());
        Arc::new(rustls_config)
    }
    /// Creates a default [rustls::ServerConfig] for accepting regular tls connections. Use this if [is_tls_alpn_challenge] returns `false`.
    /// If you need a [rustls::ServerConfig], which uses the certificates acquired by this [AcmeState],
    /// you may build your own using the output of [AcmeState::resolver].
    #[cfg(any(feature = "ring", feature = "aws-lc-rs"))]
    pub fn default_rustls_config(&self) -> Arc<ServerConfig> {
        self.default_rustls_config_with_provider(crypto_provider().into())
    }
    /// Same as [AcmeState::default_rustls_config], with a specific [CryptoProvider].
    pub fn default_rustls_config_with_provider(&self, provider: Arc<CryptoProvider>) -> Arc<ServerConfig> {
        let rustls_config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_cert_resolver(self.resolver());
        Arc::new(rustls_config)
    }

    /// Returns the current certificate.
    pub fn current_cert(&self) -> Option<&Certificate> {
        self.current_cert.as_ref()
    }

    pub fn new(config: AcmeConfig<EC, EA>) -> Self {
        let config = Arc::new(config);
        Self {
            config: config.clone(),
            resolver: ResolvesServerCertAcme::new(),
            account_key: None,
            early_action: None,
            load_cert: Some(Box::pin({
                let config = config.clone();
                async move { config.cache.load_cert(&config.all_domains(), &config.directory_url).await }
            })),
            load_account: Some(Box::pin({
                let config = config.clone();
                async move { config.cache.load_account(&config.contact, &config.directory_url).await }
            })),
            order: None,
            order_events: None,
            backoff_cnt: 0,
            wait: None,
            current_cert: None,
        }
    }
    fn parse_cert(pem: &[u8]) -> Result<(CertifiedKey, [DateTime<Utc>; 2]), CertParseError> {
        let mut pems = pem::parse_many(pem)?;
        if pems.len() < 2 {
            return Err(CertParseError::TooFewPem(pems.len()));
        }
        let pk = match any_ecdsa_type(&PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pems.remove(0).contents()))) {
            Ok(pk) => pk,
            Err(_) => return Err(CertParseError::InvalidPrivateKey),
        };
        let cert_chain: Vec<RustlsCertificate> = pems.into_iter().map(|p| RustlsCertificate::from(p.into_contents())).collect();
        let validity = match parse_x509_certificate(&cert_chain[0]) {
            Ok((_, cert)) => {
                let validity = cert.validity();
                [validity.not_before, validity.not_after].map(|t| Utc.timestamp_opt(t.timestamp(), 0).earliest().unwrap())
            }
            Err(err) => return Err(CertParseError::X509(err)),
        };
        let cert = CertifiedKey::new(cert_chain, pk);
        Ok((cert, validity))
    }

    #[allow(clippy::result_large_err)]
    fn process_cert(&mut self, pem: Vec<u8>, cached: bool) -> Event<EC, EA> {
        let (cert, validity) = match (Self::parse_cert(&pem), cached) {
            (Ok(r), _) => r,
            (Err(err), cached) => {
                return match cached {
                    true => Err(EventError::CachedCertParse(err)),
                    false => Err(EventError::NewCertParse(err)),
                }
            }
        };
        self.resolver.set_cert(Arc::new(cert));
        let wait_duration = (validity[1] - (validity[1] - validity[0]) / 3 - Utc::now())
            .max(chrono::Duration::zero())
            .to_std()
            .unwrap_or_default();
        self.wait = Some(Timer::after(wait_duration));
        let pem_str = String::from_utf8(pem.clone()).unwrap_or_default();
        let cert_obj = Certificate { pem: pem_str };
        self.current_cert = Some(cert_obj.clone());
        if cached {
            return Ok(EventOk::DeployedCachedCert(cert_obj));
        }
        let config = self.config.clone();
        self.early_action = Some(Box::pin(async move {
            match config.cache.store_cert(&config.all_domains(), &config.directory_url, &pem).await {
                Ok(()) => Ok(EventOk::CertCacheStore),
                Err(err) => Err(EventError::CertCacheStore(err)),
            }
        }));
        Event::Ok(EventOk::DeployedNewCert(cert_obj))
    }
    async fn order(
        config: Arc<AcmeConfig<EC, EA>>,
        resolver: Arc<ResolvesServerCertAcme>,
        key_pair: Vec<u8>,
        event_tx: futures::channel::mpsc::UnboundedSender<EventOk>,
    ) -> Result<Vec<u8>, OrderError> {
        let directory = Directory::discover(&config.client_config, &config.directory_url).await?;
        let account = Account::create_with_keypair(&config.client_config, directory, &config.contact, &key_pair).await?;

        let mut params = CertificateParams::new(config.all_domains())?;
        params.distinguished_name = DistinguishedName::new();
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
        let csr = params.serialize_request(&key_pair)?;

        let (order_url, mut order) = account.new_order(&config.client_config, config.all_domains()).await?;
        loop {
            match order.status {
                OrderStatus::Pending => {
                    // Force in order authorizations to allow single global challenge data state
                    for url in order.authorizations.iter() {
                        Self::authorize(&config, &resolver, &account, url, &event_tx).await?
                    }
                    log::info!("completed all authorizations");
                    order = account.order(&config.client_config, &order_url).await?;
                }
                OrderStatus::Processing => {
                    for i in 0u64..10 {
                        log::info!("order processing");
                        Timer::after(Duration::from_secs(1u64 << i)).await;
                        order = account.order(&config.client_config, &order_url).await?;
                        if order.status != OrderStatus::Processing {
                            break;
                        }
                    }
                    if order.status == OrderStatus::Processing {
                        return Err(OrderError::ProcessingTimeout(order));
                    }
                }
                OrderStatus::Ready => {
                    log::info!("sending csr");
                    order = account.finalize(&config.client_config, order.finalize, csr.der()).await?
                }
                OrderStatus::Valid { certificate } => {
                    log::info!("download certificate");
                    let pem = [
                        &key_pair.serialize_pem(),
                        "\n",
                        &account.certificate(&config.client_config, certificate).await?,
                    ]
                    .concat();
                    return Ok(pem.into_bytes());
                }
                OrderStatus::Invalid => return Err(OrderError::BadOrder(order)),
            }
        }
    }
    async fn authorize(
        config: &AcmeConfig<EC, EA>,
        resolver: &ResolvesServerCertAcme,
        account: &Account,
        url: &String,
        event_tx: &futures::channel::mpsc::UnboundedSender<EventOk>,
    ) -> Result<(), OrderError> {
        let auth = account.auth(&config.client_config, url).await?;
        let (domain, challenge_url) = match auth.status {
            AuthStatus::Pending => {
                let domain = auth.identifier.clone().into_inner();
                log::info!("trigger challenge for {}", &domain);
                let challenge = match config.challenge_type {
                    UseChallenge::Http01 => {
                        let (challenge, key_auth) = account.http_01(&auth.challenges)?;
                        resolver.set_http_01_challenge_data(challenge.token.clone(), key_auth);
                        challenge
                    }
                    UseChallenge::TlsAlpn01 => {
                        let (challenge, auth_key) = account.tls_alpn_01(&auth.challenges, domain.clone())?;
                        // For IP identifiers, use reverse-DNS (ARPA) format as the SNI,
                        // because RFC 8738 §6 requires the CA to send SNI in ARPA format
                        // (RFC 6066 does not permit IP addresses in SNI).
                        let sni = match &auth.identifier {
                            crate::acme::Identifier::Ip(addr) => crate::acme::ip_to_arpa(*addr),
                            crate::acme::Identifier::Dns(_) => domain.clone(),
                        };
                        resolver.set_tls_alpn_01_challenge_data(sni, Arc::new(auth_key));
                        challenge
                    }
                };
                let _ = event_tx.unbounded_send(EventOk::ValidationChallenge(challenge.clone()));
                account.challenge(&config.client_config, &challenge.url).await?;
                (domain, challenge.url.clone())
            }
            AuthStatus::Valid => {
                // clear challenge data when auth validated
                resolver.clear_challenge_data();
                return Ok(());
            }
            _ => {
                // clear challenge data when auth invalidated
                resolver.clear_challenge_data();
                return Err(OrderError::BadAuth(auth));
            }
        };
        for i in 0u64..5 {
            Timer::after(Duration::from_secs(1u64 << i)).await;
            let auth = account.auth(&config.client_config, url).await?;
            match auth.status {
                AuthStatus::Pending => {
                    log::info!("authorization for {} still pending", &domain);
                    account.challenge(&config.client_config, &challenge_url).await?
                }
                AuthStatus::Valid => {
                    // clear challenge data when auth validated
                    resolver.clear_challenge_data();
                    return Ok(());
                }
                _ => {
                    // clear challenge data when auth invalidated
                    resolver.clear_challenge_data();
                    return Err(OrderError::BadAuth(auth));
                }
            }
        }
        Err(OrderError::TooManyAttemptsAuth(domain))
    }
    fn poll_next_infinite(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Event<EC, EA>> {
        loop {
            // queued early action
            if let Some(early_action) = &mut self.early_action {
                let result = ready!(early_action.poll_unpin(cx));
                self.early_action.take();
                return Poll::Ready(result);
            }

            // sleep
            if let Some(timer) = &mut self.wait {
                ready!(timer.poll_unpin(cx));
                self.wait.take();
            }

            // load from cert cache
            if let Some(load_cert) = &mut self.load_cert {
                let result = ready!(load_cert.poll_unpin(cx));
                self.load_cert.take();
                match result {
                    Ok(Some(pem)) => {
                        return Poll::Ready(Self::process_cert(self.get_mut(), pem, true));
                    }
                    Ok(None) => {}
                    Err(err) => return Poll::Ready(Err(EventError::CertCacheLoad(err))),
                }
            }

            // load from account cache
            if let Some(load_account) = &mut self.load_account {
                let result = ready!(load_account.poll_unpin(cx));
                self.load_account.take();
                match result {
                    Ok(Some(key_pair)) => self.account_key = Some(key_pair),
                    Ok(None) => {}
                    Err(err) => return Poll::Ready(Err(EventError::AccountCacheLoad(err))),
                }
            }

            if let Some(events) = &mut self.order_events {
                match events.poll_next_unpin(cx) {
                    Poll::Ready(Some(event)) => return Poll::Ready(Ok(event)),
                    Poll::Ready(None) => self.order_events = None,
                    Poll::Pending => {}
                }
            }

            // execute order
            if let Some(order) = &mut self.order {
                let result = ready!(order.poll_unpin(cx));
                self.order.take();
                self.order_events = None; // clear events channel as well
                match result {
                    Ok(pem) => {
                        self.backoff_cnt = 0;
                        return Poll::Ready(Self::process_cert(self.get_mut(), pem, false));
                    }
                    Err(err) => {
                        // TODO: replace key on some errors or high backoff_cnt?
                        self.wait = Some(Timer::after(Duration::from_secs(1 << self.backoff_cnt)));
                        self.backoff_cnt = (self.backoff_cnt + 1).min(16);
                        return Poll::Ready(Err(EventError::Order(err)));
                    }
                }
            }

            // schedule order
            let account_key = match &self.account_key {
                None => {
                    let account_key = Account::generate_key_pair();
                    self.account_key = Some(account_key.clone());
                    let config = self.config.clone();
                    let account_key_clone = account_key.clone();
                    self.early_action = Some(Box::pin(async move {
                        match config
                            .cache
                            .store_account(&config.contact, &config.directory_url, &account_key_clone)
                            .await
                        {
                            Ok(()) => Ok(EventOk::AccountCacheStore),
                            Err(err) => Err(EventError::AccountCacheStore(err)),
                        }
                    }));
                    account_key
                }
                Some(account_key) => account_key.clone(),
            };
            let config = self.config.clone();
            let resolver = self.resolver.clone();
            let (tx, rx) = futures::channel::mpsc::unbounded();
            self.order_events = Some(rx);
            self.order = Some(Box::pin(Self::order(config.clone(), resolver.clone(), account_key, tx)));
        }
    }
}

impl<EC: 'static + Debug, EA: 'static + Debug> Stream for AcmeState<EC, EA> {
    type Item = Event<EC, EA>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(ready!(self.poll_next_infinite(cx))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMMY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIE...dummy_private_key...\n-----END PRIVATE KEY-----\n-----BEGIN CERTIFICATE-----\nMIID...dummy_cert_1...\n-----END CERTIFICATE-----\n-----BEGIN CERTIFICATE-----\nMIID...dummy_cert_2...\n-----END CERTIFICATE-----\n";

    #[test]
    fn test_event_ok_pem_extraction() {
        let cert = Certificate { pem: DUMMY_PEM.to_string() };
        let _event = EventOk::DeployedNewCert(cert.clone());

        let priv_key = cert.private_key_pem().unwrap();
        assert!(priv_key.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(priv_key.ends_with("-----END PRIVATE KEY-----"));
        assert!(!priv_key.contains("BEGIN CERTIFICATE"));

        let fullchain = cert.fullchain_pem().unwrap();
        assert!(fullchain.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(fullchain.ends_with("-----END CERTIFICATE-----"));
        assert!(!fullchain.contains("BEGIN PRIVATE KEY"));
        assert!(fullchain.contains("dummy_cert_2"));

        let leaf = cert.leaf_cert_pem().unwrap();
        assert!(leaf.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(leaf.ends_with("-----END CERTIFICATE-----"));
        assert!(!leaf.contains("BEGIN PRIVATE KEY"));
        assert!(leaf.contains("dummy_cert_1"));
        assert!(!leaf.contains("dummy_cert_2"));
    }

    #[test]
    fn test_acme_state_pem_extraction() {
        use crate::AcmeConfig;

        let config = AcmeConfig::new(["example.com"]);
        let mut state = config.state();
        state.current_cert = Some(Certificate { pem: DUMMY_PEM.to_string() });

        let cert = state.current_cert().unwrap();

        let priv_key = cert.private_key_pem().unwrap();
        assert!(priv_key.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(priv_key.ends_with("-----END PRIVATE KEY-----"));
        assert!(!priv_key.contains("BEGIN CERTIFICATE"));

        let fullchain = cert.fullchain_pem().unwrap();
        assert!(fullchain.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(fullchain.ends_with("-----END CERTIFICATE-----"));
        assert!(!fullchain.contains("BEGIN PRIVATE KEY"));
        assert!(fullchain.contains("dummy_cert_2"));

        let leaf = cert.leaf_cert_pem().unwrap();
        assert!(leaf.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(leaf.ends_with("-----END CERTIFICATE-----"));
        assert!(!leaf.contains("BEGIN PRIVATE KEY"));
        assert!(leaf.contains("dummy_cert_1"));
        assert!(!leaf.contains("dummy_cert_2"));
    }
}
