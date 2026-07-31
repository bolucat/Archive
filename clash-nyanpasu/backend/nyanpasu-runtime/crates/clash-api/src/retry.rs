use std::{fmt::Debug, sync::Arc, time::Duration};

use backon::{BackoffBuilder, ExponentialBuilder};
use reqwest::{Method, StatusCode};

use crate::Error;

/// Metadata exposed to an injected retry policy.
#[derive(Clone, Debug)]
pub struct RequestMetadata {
    operation: &'static str,
    method: Method,
    retry_safe: bool,
}

impl RequestMetadata {
    pub(crate) fn new(operation: &'static str, method: Method, retry_safe: bool) -> Self {
        Self {
            operation,
            method,
            retry_safe,
        }
    }

    pub fn operation(&self) -> &'static str {
        self.operation
    }

    pub fn method(&self) -> &Method {
        &self.method
    }

    /// Whether the endpoint explicitly permits repeating this operation.
    pub fn is_retry_safe(&self) -> bool {
        self.retry_safe
    }
}

/// Supplies retry delays and classifies errors for one logical API operation.
pub trait RetryPolicy: Debug + Send + Sync + 'static {
    fn delays(&self, request: &RequestMetadata) -> Box<dyn Iterator<Item = Duration> + Send>;

    fn is_retryable(&self, request: &RequestMetadata, error: &Error) -> bool;
}

pub(crate) type SharedRetryPolicy = Arc<dyn RetryPolicy>;

/// The default policy: every operation is attempted exactly once.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRetry;

impl RetryPolicy for NoRetry {
    fn delays(&self, _request: &RequestMetadata) -> Box<dyn Iterator<Item = Duration> + Send> {
        Box::new(std::iter::empty())
    }

    fn is_retryable(&self, _request: &RequestMetadata, _error: &Error) -> bool {
        false
    }
}

/// Capped exponential backoff for explicitly repeatable operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExponentialRetry {
    max_retries: usize,
    min_delay: Duration,
    max_delay: Duration,
    jitter: bool,
}

impl ExponentialRetry {
    /// A conservative policy suitable for a controller running on the same machine.
    pub const fn conservative() -> Self {
        Self {
            max_retries: 3,
            min_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(2),
            jitter: true,
        }
    }

    pub const fn new(max_retries: usize, min_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_retries,
            min_delay,
            max_delay,
            jitter: false,
        }
    }

    pub const fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    pub const fn max_retries(&self) -> usize {
        self.max_retries
    }
}

impl Default for ExponentialRetry {
    fn default() -> Self {
        Self::conservative()
    }
}

impl RetryPolicy for ExponentialRetry {
    fn delays(&self, _request: &RequestMetadata) -> Box<dyn Iterator<Item = Duration> + Send> {
        let builder = ExponentialBuilder::default()
            .with_min_delay(self.min_delay)
            .with_max_delay(self.max_delay)
            .with_max_times(self.max_retries);

        if self.jitter {
            Box::new(builder.with_jitter().build())
        } else {
            Box::new(builder.build())
        }
    }

    fn is_retryable(&self, request: &RequestMetadata, error: &Error) -> bool {
        request.is_retry_safe() && is_transient(error)
    }
}

fn is_transient(error: &Error) -> bool {
    match error {
        Error::Request { source, .. } => {
            source.is_connect() || source.is_timeout() || source.is_request()
        }
        Error::HttpStatus { status, .. } => matches!(
            *status,
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
        ),
        Error::WebSocket { source, .. } => match source {
            reqwest_websocket::Error::Reqwest(source) => {
                source.is_connect() || source.is_timeout() || source.is_request()
            }
            reqwest_websocket::Error::Handshake(
                reqwest_websocket::HandshakeError::UnexpectedStatusCode(status),
            ) => matches!(
                *status,
                StatusCode::BAD_GATEWAY
                    | StatusCode::SERVICE_UNAVAILABLE
                    | StatusCode::GATEWAY_TIMEOUT
            ),
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_retry_produces_exactly_the_configured_extra_attempts() {
        let policy = ExponentialRetry::new(2, Duration::from_millis(1), Duration::from_millis(2));
        let metadata = RequestMetadata::new("version", Method::GET, true);

        assert_eq!(policy.delays(&metadata).count(), 2);
    }
}
