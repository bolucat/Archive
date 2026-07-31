use std::{future::Future, path::PathBuf, sync::Arc};

use futures_util::StreamExt;
use reqwest::{
    Method, RequestBuilder, Response, Url,
    header::{AUTHORIZATION, HeaderValue},
};
use reqwest_websocket::Upgrade;

use crate::{
    Error, Result,
    retry::{NoRetry, RequestMetadata, RetryPolicy, SharedRetryPolicy},
};

const LOCAL_TRANSPORT_BASE_URL: &str = "http://localhost/";

/// The transport used to connect to the Mihomo external controller.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Host {
    NamedPipe(PathBuf),
    UnixSocket(PathBuf),
    Http(Url),
}

/// More descriptive alias for [`Host`].
pub type ControllerEndpoint = Host;

impl Host {
    pub fn named_pipe(path: impl Into<PathBuf>) -> Self {
        Self::NamedPipe(path.into())
    }

    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket(path.into())
    }

    /// Construct an HTTP endpoint from either `host:port` or a complete URL.
    pub fn http(base_url: impl AsRef<str>) -> Result<Self> {
        parse_controller_url(base_url.as_ref(), "http")
    }

    /// Construct an HTTPS endpoint from either `host:port` or a complete URL.
    pub fn https(base_url: impl AsRef<str>) -> Result<Self> {
        parse_controller_url(base_url.as_ref(), "https")
    }

    /// Construct an endpoint from a complete HTTP(S) URL.
    pub fn url(base_url: impl AsRef<str>) -> Result<Self> {
        parse_complete_url(base_url.as_ref())
    }
}

/// Controller secret with redacted debug output.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Secret([REDACTED])")
    }
}

/// A Clash API client that shares one connection pool across all operations.
#[derive(Clone, Debug)]
pub struct Client {
    client: reqwest::Client,
    host: Host,
    base_url: Url,
    authorization: Option<HeaderValue>,
    retry_policy: SharedRetryPolicy,
}

impl Client {
    pub fn builder(host: Host) -> ClientBuilder {
        ClientBuilder::new(host)
    }

    /// Create a client with no secret and no high-level retries.
    pub fn new(host: Host) -> Result<Self> {
        Self::builder(host).build()
    }

    pub fn new_named_pipe(path: impl Into<PathBuf>) -> Result<Self> {
        Self::new(Host::named_pipe(path))
    }

    pub fn new_unix_socket(path: impl Into<PathBuf>) -> Result<Self> {
        Self::new(Host::unix_socket(path))
    }

    #[cfg(unix)]
    pub fn unix_socket(path: impl Into<PathBuf>) -> Result<Self> {
        Self::new_unix_socket(path)
    }

    pub fn new_http(base_url: impl AsRef<str>) -> Result<Self> {
        Self::new(Host::http(base_url)?)
    }

    pub fn host(&self) -> &Host {
        &self.host
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// Access the shared reqwest client for advanced integrations.
    pub fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Resolve a relative API endpoint against the configured controller URL.
    pub fn endpoint(&self, endpoint: &str) -> Result<Url> {
        if Url::parse(endpoint).is_ok() {
            return Err(Error::AbsoluteEndpoint {
                endpoint: endpoint.to_owned(),
            });
        }

        self.base_url
            .join(endpoint.trim_start_matches('/'))
            .map_err(|source| Error::InvalidEndpoint {
                endpoint: endpoint.to_owned(),
                source,
            })
    }

    /// Resolve an endpoint and safely append dynamic path segments.
    pub fn endpoint_with_segments<I, S>(&self, endpoint: &str, segments: I) -> Result<Url>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut url = self.endpoint(endpoint)?;
        {
            let error_url = url.clone();
            let mut path = url
                .path_segments_mut()
                .map_err(|()| Error::CannotAppendPathSegment { url: error_url })?;
            for segment in segments {
                path.push(segment.as_ref());
            }
        }
        Ok(url)
    }

    /// Start building a low-level request. Typed endpoint methods are preferred.
    pub fn request(&self, method: Method, endpoint: &str) -> Result<RequestBuilder> {
        Ok(self.request_url(method, self.endpoint(endpoint)?))
    }

    pub fn get(&self, endpoint: &str) -> Result<RequestBuilder> {
        self.request(Method::GET, endpoint)
    }

    pub fn post(&self, endpoint: &str) -> Result<RequestBuilder> {
        self.request(Method::POST, endpoint)
    }

    pub fn put(&self, endpoint: &str) -> Result<RequestBuilder> {
        self.request(Method::PUT, endpoint)
    }

    pub fn patch(&self, endpoint: &str) -> Result<RequestBuilder> {
        self.request(Method::PATCH, endpoint)
    }

    pub fn delete(&self, endpoint: &str) -> Result<RequestBuilder> {
        self.request(Method::DELETE, endpoint)
    }

    pub(crate) fn request_url(&self, method: Method, url: Url) -> RequestBuilder {
        let request = self.client.request(method, url);
        match &self.authorization {
            Some(authorization) => request.header(AUTHORIZATION, authorization.clone()),
            None => request,
        }
    }

    pub(crate) async fn send<F>(
        &self,
        metadata: RequestMetadata,
        make_request: F,
    ) -> Result<Response>
    where
        F: Fn() -> Result<RequestBuilder>,
    {
        self.execute(&metadata, || async {
            let response = make_request()?
                .send()
                .await
                .map_err(|source| Error::Request {
                    operation: metadata.operation(),
                    source,
                })?;

            self.ensure_success(response, metadata.operation()).await
        })
        .await
    }

    pub(crate) async fn decode_json<T>(
        &self,
        response: Response,
        operation: &'static str,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let bytes = response
            .bytes()
            .await
            .map_err(|source| Error::Request { operation, source })?;
        serde_json::from_slice(&bytes).map_err(|source| Error::Decode { operation, source })
    }

    pub(crate) async fn send_json<T, F>(
        &self,
        metadata: RequestMetadata,
        make_request: F,
    ) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
        F: Fn() -> Result<RequestBuilder>,
    {
        let operation = metadata.operation();
        let response = self.send(metadata, make_request).await?;
        self.decode_json(response, operation).await
    }

    pub(crate) async fn send_empty<F>(
        &self,
        metadata: RequestMetadata,
        make_request: F,
    ) -> Result<()>
    where
        F: Fn() -> Result<RequestBuilder>,
    {
        self.send(metadata, make_request).await?;
        Ok(())
    }

    pub(crate) async fn websocket<F>(
        &self,
        metadata: RequestMetadata,
        make_request: F,
    ) -> Result<reqwest_websocket::WebSocket>
    where
        F: Fn() -> Result<RequestBuilder>,
    {
        let operation = metadata.operation();
        self.execute(&metadata, || async {
            let response = make_request()?
                .upgrade()
                .send()
                .await
                .map_err(|source| Error::WebSocket { operation, source })?;
            response
                .into_websocket()
                .await
                .map_err(|source| Error::WebSocket { operation, source })
        })
        .await
    }

    pub(crate) async fn execute<T, F, Fut>(
        &self,
        metadata: &RequestMetadata,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut delays = self.retry_policy.delays(metadata);

        loop {
            match operation().await {
                Ok(value) => return Ok(value),
                Err(error) if self.retry_policy.is_retryable(metadata, &error) => {
                    let Some(delay) = delays.next() else {
                        return Err(error);
                    };
                    tokio::time::sleep(delay).await;
                }
                Err(error) => return Err(error),
            }
        }
    }

    async fn ensure_success(
        &self,
        response: Response,
        operation: &'static str,
    ) -> Result<Response> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while bytes.len() < 16 * 1024 {
            let Some(chunk) = stream.next().await else {
                break;
            };
            let Ok(chunk) = chunk else {
                break;
            };
            let remaining = 16 * 1024 - bytes.len();
            bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }
        let body = (!bytes.is_empty()).then(|| crate::ErrorBody::from_bytes(&bytes));

        Err(Error::HttpStatus {
            operation,
            status,
            body,
        })
    }
}

pub struct ClientBuilder {
    host: Host,
    reqwest: reqwest::ClientBuilder,
    secret: Option<Secret>,
    retry_policy: SharedRetryPolicy,
}

impl ClientBuilder {
    pub fn new(host: Host) -> Self {
        Self {
            host,
            reqwest: reqwest::Client::builder()
                .no_proxy()
                .http1_only()
                .retry(reqwest::retry::never()),
            secret: None,
            retry_policy: Arc::new(NoRetry),
        }
    }

    pub fn secret(mut self, secret: impl Into<Secret>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    pub fn retry_policy(mut self, retry_policy: impl RetryPolicy) -> Self {
        self.retry_policy = Arc::new(retry_policy);
        self
    }

    /// Inject a policy that is already shared or selected dynamically.
    pub fn shared_retry_policy(mut self, retry_policy: Arc<dyn RetryPolicy>) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    /// Apply timeout, TLS, certificate, or other reqwest configuration.
    pub fn configure_reqwest(
        mut self,
        configure: impl FnOnce(reqwest::ClientBuilder) -> reqwest::ClientBuilder,
    ) -> Self {
        self.reqwest = configure(self.reqwest);
        self
    }

    pub fn build(self) -> Result<Client> {
        let Self {
            host,
            mut reqwest,
            secret,
            retry_policy,
        } = self;

        let (host, base_url) = match host {
            Host::Http(base_url) => {
                let base_url = normalize_base_url(base_url)?;
                (Host::Http(base_url.clone()), base_url)
            }
            Host::NamedPipe(path) => {
                #[cfg(windows)]
                {
                    reqwest = reqwest.windows_named_pipe(path.as_path());
                    (
                        Host::NamedPipe(path),
                        Url::parse(LOCAL_TRANSPORT_BASE_URL)
                            .expect("the local transport base URL must be valid"),
                    )
                }
                #[cfg(not(windows))]
                {
                    let _ = (path, reqwest);
                    return Err(Error::UnsupportedTransport {
                        transport: "Windows named pipe",
                        platform: std::env::consts::OS,
                    });
                }
            }
            Host::UnixSocket(path) => {
                #[cfg(unix)]
                {
                    reqwest = reqwest.unix_socket(path.as_path());
                    (
                        Host::UnixSocket(path),
                        Url::parse(LOCAL_TRANSPORT_BASE_URL)
                            .expect("the local transport base URL must be valid"),
                    )
                }
                #[cfg(not(unix))]
                {
                    let _ = (path, reqwest);
                    return Err(Error::UnsupportedTransport {
                        transport: "Unix domain socket",
                        platform: std::env::consts::OS,
                    });
                }
            }
        };

        let authorization = if matches!(host, Host::Http(_)) {
            secret
                .filter(|secret| !secret.is_empty())
                .map(|secret| {
                    let mut value = HeaderValue::from_str(&format!("Bearer {}", secret.0))
                        .map_err(Error::InvalidSecret)?;
                    value.set_sensitive(true);
                    Ok(value)
                })
                .transpose()?
        } else {
            None
        };

        let client = reqwest.build().map_err(Error::BuildClient)?;

        Ok(Client {
            client,
            host,
            base_url,
            authorization,
            retry_policy,
        })
    }
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientBuilder")
            .field("host", &self.host)
            .field("secret", &self.secret)
            .field("retry_policy", &self.retry_policy)
            .finish_non_exhaustive()
    }
}

fn parse_controller_url(value: &str, default_scheme: &str) -> Result<Host> {
    if value.contains("://") {
        return parse_complete_url(value);
    }

    parse_complete_url(&format!("{default_scheme}://{value}"))
}

fn parse_complete_url(value: &str) -> Result<Host> {
    let url = Url::parse(value).map_err(|source| Error::InvalidBaseUrl {
        value: value.to_owned(),
        source,
    })?;
    Ok(Host::Http(normalize_base_url(url)?))
}

fn normalize_base_url(mut base_url: Url) -> Result<Url> {
    if !matches!(base_url.scheme(), "http" | "https") {
        return Err(Error::UnsupportedUrlScheme {
            scheme: base_url.scheme().to_owned(),
        });
    }

    if base_url.cannot_be_a_base() {
        return Err(Error::UrlCannotBeABase { url: base_url });
    }

    if base_url.query().is_some() || base_url.fragment().is_some() {
        return Err(Error::BaseUrlHasQueryOrFragment { url: base_url });
    }

    if !base_url.path().ends_with('/') {
        let mut path = base_url.path().to_owned();
        path.push('/');
        base_url.set_path(&path);
    }

    Ok(base_url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_port_is_ergonomic_and_base_url_is_normalized() {
        let client = Client::new_http("127.0.0.1:9090/api").unwrap();

        assert_eq!(client.base_url().as_str(), "http://127.0.0.1:9090/api/");
        assert_eq!(
            client
                .get("/version")
                .unwrap()
                .build()
                .unwrap()
                .url()
                .as_str(),
            "http://127.0.0.1:9090/api/version"
        );
    }

    #[test]
    fn dynamic_path_segments_are_percent_encoded() {
        let client = Client::new_http("127.0.0.1:9090/api").unwrap();
        let url = client
            .endpoint_with_segments("/proxies", ["a/b ?#%", "日本語"])
            .unwrap();

        assert_eq!(
            url.as_str(),
            "http://127.0.0.1:9090/api/proxies/a%2Fb%20%3F%23%25/%E6%97%A5%E6%9C%AC%E8%AA%9E"
        );

        let detail = client
            .endpoint_with_segments("/proxies", ["a/b", ""])
            .unwrap();
        assert_eq!(detail.as_str(), "http://127.0.0.1:9090/api/proxies/a%2Fb/");
    }

    #[test]
    fn secret_is_redacted_in_debug_output_and_marked_sensitive() {
        let client = Client::builder(Host::http("127.0.0.1:9090").unwrap())
            .secret("do-not-log-me")
            .build()
            .unwrap();
        let request = client.get("/version").unwrap().build().unwrap();

        assert!(!format!("{client:?}").contains("do-not-log-me"));
        assert!(request.headers()[AUTHORIZATION].is_sensitive());
    }

    #[test]
    fn invalid_controller_urls_are_rejected() {
        assert!(matches!(
            Host::url("not a URL"),
            Err(Error::InvalidBaseUrl { .. })
        ));
        assert!(matches!(
            Host::url("ftp://127.0.0.1/api"),
            Err(Error::UnsupportedUrlScheme { .. })
        ));
        assert!(matches!(
            Host::url("http://127.0.0.1/api?secret=value"),
            Err(Error::BaseUrlHasQueryOrFragment { .. })
        ));
    }

    #[test]
    fn absolute_endpoints_are_rejected() {
        let client = Client::new_http("127.0.0.1:9090").unwrap();
        assert!(matches!(
            client.get("https://example.com/version"),
            Err(Error::AbsoluteEndpoint { .. })
        ));
    }

    #[cfg(windows)]
    #[test]
    fn named_pipe_client_uses_a_synthetic_local_base_url_without_auth() {
        let path = PathBuf::from(r"\\.\pipe\clash-api-test");
        let client = Client::builder(Host::NamedPipe(path.clone()))
            .secret("ignored-for-local-transports")
            .build()
            .unwrap();

        assert_eq!(client.host(), &Host::NamedPipe(path));
        let request = client.get("/version").unwrap().build().unwrap();
        assert_eq!(request.url().as_str(), "http://localhost/version");
        assert!(!request.headers().contains_key(AUTHORIZATION));
    }

    #[cfg(not(windows))]
    #[test]
    fn named_pipe_transport_is_rejected_off_windows() {
        assert!(matches!(
            Client::new_named_pipe("clash-api-test"),
            Err(Error::UnsupportedTransport { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn unix_socket_client_uses_a_synthetic_local_base_url_without_auth() {
        let path = PathBuf::from("/tmp/clash-api-test.sock");
        let client = Client::builder(Host::UnixSocket(path.clone()))
            .secret("ignored-for-local-transports")
            .build()
            .unwrap();

        assert_eq!(client.host(), &Host::UnixSocket(path));
        let request = client.get("/version").unwrap().build().unwrap();
        assert_eq!(request.url().as_str(), "http://localhost/version");
        assert!(!request.headers().contains_key(AUTHORIZATION));
    }

    #[cfg(not(unix))]
    #[test]
    fn unix_socket_transport_is_rejected_off_unix() {
        assert!(matches!(
            Client::new_unix_socket("clash-api-test.sock"),
            Err(Error::UnsupportedTransport { .. })
        ));
    }
}
