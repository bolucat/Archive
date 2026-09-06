//! The layer stack shared by every route: request id, timeout, panic capture
//! and the `R`-envelope fallbacks.
//!
//! The rule for all four: they only produce a response where the client
//! previously got something worse (an empty body, a dropped connection, or no
//! answer at all). None of them can fire on a working operation.

use std::{any::Any, borrow::Cow, time::Duration};

use axum::{
    Json,
    extract::Request,
    http::{Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use nyanpasu_ipc::api::{R, RBuilder};
use tower_http::{catch_panic::ResponseForPanic, trace::MakeSpan};

/// Header carrying the per-request correlation id. Must match the header
/// `tower_http::request_id`'s `x_request_id` constructors use.
const REQUEST_ID_HEADER: &str = "x-request-id";

/// Upper bound on a single request/response operation.
///
/// This is a guard rail against a wedged handler holding an IPC connection
/// forever, NOT a policy timeout: the core manager already bounds its own work
/// at `reconcile_timeout(30s) + stop_timeout(10s) + startup_timeout(30s) +
/// stop_timeout(10s) = 80s` worst case (`nyanpasu_core_manager::spec`), so this
/// sits above anything a working `/core/start` or `Reconcile` submission can take.
/// The ws endpoint is a long-lived stream and is deliberately not bounded.
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// An error response in the legacy envelope: `{code, msg, data, ts}`.
fn error_envelope(status: StatusCode, msg: Cow<'static, str>) -> axum::response::Response {
    let body: R<'static, ()> = RBuilder::other_error(msg);
    (status, Json(body)).into_response()
}

/// Answer unrouted paths with the envelope instead of an empty 404 body.
pub(super) async fn not_found() -> axum::response::Response {
    error_envelope(StatusCode::NOT_FOUND, Cow::Borrowed("not found"))
}

/// Answer a known path called with the wrong method with the envelope.
pub(super) async fn method_not_allowed() -> axum::response::Response {
    error_envelope(
        StatusCode::METHOD_NOT_ALLOWED,
        Cow::Borrowed("method not allowed"),
    )
}

/// Bound a request/response operation at [`REQUEST_TIMEOUT`].
pub(super) async fn enforce_timeout(request: Request, next: Next) -> axum::response::Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(request)).await {
        Ok(response) => response,
        Err(_) => {
            tracing::error!("request exceeded {REQUEST_TIMEOUT:?}; answering with a timeout");
            error_envelope(
                StatusCode::REQUEST_TIMEOUT,
                Cow::Borrowed("request timed out"),
            )
        }
    }
}

/// Turn a handler panic into a 500 envelope instead of a dropped connection.
///
/// The panic payload is logged, never sent: it can carry paths and internal
/// state, and the client only needs to know the request failed.
#[derive(Clone, Copy)]
pub(super) struct PanicEnvelope;

impl ResponseForPanic for PanicEnvelope {
    type ResponseBody = axum::body::Body;

    fn response_for_panic(
        &mut self,
        err: Box<dyn Any + Send + 'static>,
    ) -> Response<Self::ResponseBody> {
        let detail = if let Some(message) = err.downcast_ref::<String>() {
            message.as_str()
        } else if let Some(message) = err.downcast_ref::<&str>() {
            message
        } else {
            "unknown panic payload"
        };
        tracing::error!("request handler panicked: {detail}");
        error_envelope(
            StatusCode::INTERNAL_SERVER_ERROR,
            Cow::Borrowed("internal server error"),
        )
    }
}

/// `DefaultMakeSpan`'s fields plus the request id, at the same DEBUG level, so
/// a trace span can be tied to the `x-request-id` the caller sees. The service
/// filters at INFO by default, so these spans change nothing in `/logs` unless
/// the operator asked for debug logging.
#[derive(Clone, Copy)]
pub(super) struct RequestSpan;

impl<B> MakeSpan<B> for RequestSpan {
    fn make_span(&mut self, request: &axum::http::Request<B>) -> tracing::Span {
        tracing::debug_span!(
            "request",
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
            request_id = request
                .headers()
                .get(REQUEST_ID_HEADER)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("-"),
        )
    }
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::to_bytes, http::Request, routing::get};
    use nyanpasu_ipc::api::{R, ResponseCode};
    use tower::ServiceExt;
    use tower_http::catch_panic::CatchPanicLayer;

    use super::*;

    async fn envelope_of(response: axum::response::Response) -> R<'static, ()> {
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn panic_handler() -> &'static str {
        panic!("boom")
    }

    #[tokio::test]
    async fn a_panicking_handler_answers_with_the_envelope() {
        let app = Router::new()
            .route("/boom", get(panic_handler))
            .layer(CatchPanicLayer::custom(PanicEnvelope));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/boom")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let envelope = envelope_of(response).await;
        assert_eq!(envelope.code, ResponseCode::OtherError);
        assert_eq!(envelope.msg, "internal server error");
        assert!(envelope.data.is_none());
    }

    // `start_paused` auto-advances the clock while the runtime is idle, so the
    // real 120s constant is exercised without the test taking 120s.
    #[tokio::test(start_paused = true)]
    async fn a_wedged_handler_answers_with_the_timeout_envelope() {
        let app = Router::new()
            .route(
                "/slow",
                get(|| async {
                    tokio::time::sleep(REQUEST_TIMEOUT * 2).await;
                    "never"
                }),
            )
            .layer(axum::middleware::from_fn(enforce_timeout));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/slow")
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        let envelope = envelope_of(response).await;
        assert_eq!(envelope.code, ResponseCode::OtherError);
        assert_eq!(envelope.msg, "request timed out");
    }
}
