use reqwest::StatusCode;
use tokio_util::codec::LinesCodecError;

/// The error payload returned by Mihomo's HTTP handlers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorBody {
    message: Option<String>,
    text: String,
}

impl ErrorBody {
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        const MAX_ERROR_BODY_LENGTH: usize = 16 * 1024;

        let bytes = &bytes[..bytes.len().min(MAX_ERROR_BODY_LENGTH)];
        let text = String::from_utf8_lossy(bytes).into_owned();
        let message = serde_json::from_slice::<MihomoError>(bytes)
            .ok()
            .map(|body| body.message);

        Self { message, text }
    }

    /// Mihomo's structured `message`, when the response had its standard error schema.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// The truncated response body, retained for non-standard controllers.
    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(serde::Deserialize)]
struct MihomoError {
    message: String,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid Clash API base URL `{value}`: {source}")]
    InvalidBaseUrl {
        value: String,
        #[source]
        source: url::ParseError,
    },

    #[error("unsupported Clash API URL scheme `{scheme}`")]
    UnsupportedUrlScheme { scheme: String },

    #[error("URL cannot be used as a Clash API base URL: {url}")]
    UrlCannotBeABase { url: url::Url },

    #[error("Clash API base URL must not contain a query or fragment: {url}")]
    BaseUrlHasQueryOrFragment { url: url::Url },

    #[error("Clash API endpoint must be relative, got `{endpoint}`")]
    AbsoluteEndpoint { endpoint: String },

    #[error("invalid Clash API endpoint `{endpoint}`: {source}")]
    InvalidEndpoint {
        endpoint: String,
        #[source]
        source: url::ParseError,
    },

    #[error("cannot append a path segment to Clash API URL `{url}`")]
    CannotAppendPathSegment { url: url::Url },

    #[error("invalid argument `{argument}`: {message}")]
    InvalidArgument {
        argument: &'static str,
        message: String,
    },

    #[error("{transport} transport is not supported on {platform}")]
    UnsupportedTransport {
        transport: &'static str,
        platform: &'static str,
    },

    #[error("the Clash API secret cannot be represented as an HTTP header")]
    InvalidSecret(#[source] reqwest::header::InvalidHeaderValue),

    #[error("failed to build the Clash API HTTP client: {0}")]
    BuildClient(#[source] reqwest::Error),

    #[error("Clash API request `{operation}` failed: {source}")]
    Request {
        operation: &'static str,
        #[source]
        source: reqwest::Error,
    },

    #[error("Clash API request `{operation}` returned HTTP {status}")]
    HttpStatus {
        operation: &'static str,
        status: StatusCode,
        body: Option<ErrorBody>,
    },

    #[error("failed to decode Clash API response for `{operation}`: {source}")]
    Decode {
        operation: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to read Clash API stream `{operation}`: {source}")]
    Stream {
        operation: &'static str,
        #[source]
        source: LinesCodecError,
    },

    #[error("Clash API WebSocket handshake `{operation}` failed: {source}")]
    WebSocket {
        operation: &'static str,
        #[source]
        source: reqwest_websocket::Error,
    },
}

impl Error {
    /// HTTP status returned by the controller, if the request reached it.
    pub fn status(&self) -> Option<StatusCode> {
        match self {
            Self::HttpStatus { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Structured or textual response body retained for an HTTP error.
    pub fn error_body(&self) -> Option<&ErrorBody> {
        match self {
            Self::HttpStatus { body, .. } => body.as_ref(),
            _ => None,
        }
    }
}
