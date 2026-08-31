use std::sync::OnceLock;

use reqwest::{Method, RequestBuilder, StatusCode, Url};

use crate::{
    SERVICE_PLACEHOLDER,
    api::{
        CoreErrorKind, R, ResponseCode,
        contract::{IpcOperation, OpResponse},
    },
};

pub mod shortcuts;

pub type Result<T> = std::result::Result<T, ClientError>;

/// Synthetic base URL for requests over the local IPC transport.
///
/// Requests travel over a named pipe or unix socket; the HTTP authority is
/// only there to satisfy the protocol, nothing is routed by it.
const LOCAL_TRANSPORT_BASE_URL: &str = "http://localhost/";

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ClientError {
    #[error("failed to build the IPC client: {0}")]
    BuildClient(#[source] reqwest::Error),
    #[error("IPC request `{operation}` failed: {source}")]
    Request {
        operation: &'static str,
        #[source]
        source: reqwest::Error,
    },
    #[error("IPC request `{operation}` returned HTTP {status}")]
    HttpStatus {
        operation: &'static str,
        status: StatusCode,
        body: Option<String>,
    },
    #[error("failed to decode the IPC response for `{operation}`: {source}")]
    Decode {
        operation: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("IPC request `{operation}` failed with {code:?}: {msg}")]
    Server {
        operation: &'static str,
        code: ResponseCode,
        msg: String,
        /// The envelope's `error_kind`, when the service classified the
        /// failure. See [`crate::api::CoreErrorKind`].
        error_kind: Option<String>,
        /// The envelope's `retryable`, when the service answered. `None` also
        /// covers every service too old to carry the field, so it must not be
        /// read as "do not retry"; see [`ClientError::retryable`].
        retryable: Option<bool>,
    },
    #[error("IPC request `{operation}` succeeded but carried no data")]
    EmptyData { operation: &'static str },
    #[error("IPC WebSocket `{operation}` failed: {source}")]
    WebSocket {
        operation: &'static str,
        #[source]
        source: reqwest_websocket::Error,
    },
}

impl ClientError {
    /// The typed classification of a server-side failure, when there is one
    /// this build knows.
    ///
    /// `None` covers three different things and deliberately does not
    /// distinguish them: a transport failure with no envelope, an envelope the
    /// service did not classify, and a kind a newer service named that this
    /// build has no variant for. The raw string is still on
    /// [`Self::Server::error_kind`] for the last case.
    pub fn core_error_kind(&self) -> Option<CoreErrorKind> {
        match self {
            Self::Server { error_kind, .. } => {
                error_kind.as_deref().and_then(CoreErrorKind::from_wire)
            }
            _ => None,
        }
    }

    /// Whether retrying this request could succeed.
    ///
    /// The service's own answer wins; when it did not give one — an older
    /// build, or an unclassified failure — the kind's default stands in, and a
    /// failure with neither is not retryable.
    pub fn retryable(&self) -> bool {
        match self {
            Self::Server { retryable, .. } => retryable.unwrap_or_else(|| {
                self.core_error_kind()
                    .is_some_and(|kind| kind.default_retryable())
            }),
            _ => false,
        }
    }
}

/// An IPC client sharing one underlying HTTP client across all operations.
#[derive(Clone, Debug)]
pub struct Client {
    client: reqwest::Client,
    base_url: Url,
}

impl Client {
    /// Create a client for the IPC endpoint named by `placeholder`.
    ///
    /// The placeholder maps to `\\.\pipe\{placeholder}` on Windows and
    /// `/var/run/{placeholder}.sock` on Unix.
    pub fn new(placeholder: &str) -> Result<Self> {
        let path = crate::utils::get_name_string(placeholder);
        let builder = reqwest::Client::builder().no_proxy().http1_only();
        #[cfg(windows)]
        let builder = builder.windows_named_pipe(std::path::Path::new(&path));
        #[cfg(unix)]
        let builder = builder.unix_socket(std::path::Path::new(&path));
        let client = builder.build().map_err(ClientError::BuildClient)?;
        Ok(Self {
            client,
            base_url: Url::parse(LOCAL_TRANSPORT_BASE_URL)
                .expect("the local transport base URL must be valid"),
        })
    }

    /// The client for the nyanpasu service's default IPC endpoint.
    pub fn service_default() -> &'static Self {
        static CLIENT: OnceLock<Client> = OnceLock::new();
        CLIENT.get_or_init(|| {
            Self::new(SERVICE_PLACEHOLDER).expect("failed to build the default IPC client")
        })
    }

    /// Access the shared reqwest client for advanced integrations.
    pub fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    pub(crate) fn request(&self, method: Method, endpoint: &str) -> RequestBuilder {
        let url = self
            .base_url
            .join(endpoint.trim_start_matches('/'))
            .expect("IPC endpoint must be a valid relative URL");
        self.client.request(method, url)
    }

    pub(crate) fn get(&self, endpoint: &str) -> RequestBuilder {
        self.request(Method::GET, endpoint)
    }

    pub(crate) async fn send(
        &self,
        operation: &'static str,
        request: RequestBuilder,
    ) -> Result<reqwest::Response> {
        let response = request
            .send()
            .await
            .map_err(|source| ClientError::Request { operation, source })?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|source| ClientError::Request { operation, source })?;
        // The service reports failures with the usual response envelope.
        if let Ok(envelope) = serde_json::from_slice::<R<'_, Option<()>>>(&bytes)
            && envelope.code != ResponseCode::Ok
        {
            return Err(ClientError::Server {
                operation,
                code: envelope.code,
                msg: envelope.msg.into_owned(),
                error_kind: envelope.error_kind.map(|kind| kind.into_owned()),
                retryable: envelope.retryable,
            });
        }
        let body =
            Some(String::from_utf8_lossy(&bytes).into_owned()).filter(|body| !body.is_empty());
        Err(ClientError::HttpStatus {
            operation,
            status,
            body,
        })
    }

    /// Send `Op` and return its response envelope.
    ///
    /// `body` is `None` for the operations whose contract declares
    /// `Req<'a> = ()`: nothing is written to the wire and no `Content-Type` is
    /// set, matching what the hand-written shortcuts sent before the contract
    /// existed.
    pub async fn call<Op>(&self, body: Option<&Op::Req<'_>>) -> Result<OpResponse<Op>>
    where
        Op: IpcOperation,
    {
        let mut request = self.request(Op::METHOD, Op::PATH);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = self.send(Op::PATH, request).await?;
        let bytes = response
            .bytes()
            .await
            .map_err(|source| ClientError::Request {
                operation: Op::PATH,
                source,
            })?;
        let envelope = serde_json::from_slice::<OpResponse<Op>>(&bytes).map_err(|source| {
            ClientError::Decode {
                operation: Op::PATH,
                source,
            }
        })?;
        if envelope.code != ResponseCode::Ok {
            return Err(ClientError::Server {
                operation: Op::PATH,
                code: envelope.code,
                msg: envelope.msg.into_owned(),
                error_kind: envelope.error_kind.map(|kind| kind.into_owned()),
                retryable: envelope.retryable,
            });
        }
        Ok(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_kind_is_typed() {
        let error = ClientError::Server {
            operation: "/core/apply",
            code: ResponseCode::OtherError,
            msg: "boom".into(),
            retryable: None,
            error_kind: Some("revision_conflict".into()),
        };
        assert_eq!(
            error.core_error_kind(),
            Some(CoreErrorKind::RevisionConflict)
        );
    }

    /// The kind's default is a fallback for services that did not answer, not
    /// a rule that overrides one that did.
    #[test]
    fn the_services_own_retryability_wins_over_the_kind_default() {
        let server = |retryable| ClientError::Server {
            operation: "/core/v2/submit",
            code: ResponseCode::OtherError,
            msg: "boom".into(),
            retryable,
            error_kind: Some("backend_unavailable".into()),
        };
        assert!(server(None).retryable());
        assert!(!server(Some(false)).retryable());
    }

    #[test]
    fn an_unknown_kind_keeps_its_raw_string() {
        let error = ClientError::Server {
            operation: "/core/apply",
            code: ResponseCode::OtherError,
            msg: "boom".into(),
            retryable: None,
            error_kind: Some("a_future_kind".into()),
        };
        assert!(error.core_error_kind().is_none());
        match error {
            ClientError::Server { error_kind, .. } => {
                assert_eq!(error_kind.as_deref(), Some("a_future_kind"));
            }
            other => panic!("expected a server error, got: {other:?}"),
        }
    }
}
