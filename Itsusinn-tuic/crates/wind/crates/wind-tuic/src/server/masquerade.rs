//! HTTP/3 masquerade: serve non-TUIC (real HTTP/3) clients as a reverse proxy.
//!
//! When the connection classifier in [`super`] decides a peer is speaking
//! actual HTTP/3 rather than TUIC (its first stream's leading bytes aren't
//! valid TUIC framing), it hands the connection here. We run a real HTTP/3
//! server over the
//! backend-agnostic [`wind_quic::h3_adapter`] and reverse-proxy every request
//! to a configured upstream site with a `reqwest` client, relaying the response
//! back. To an active prober the server is indistinguishable from a normal
//! HTTP/3 web server.

use std::{
	sync::{Arc, OnceLock},
	time::Duration,
};

use bytes::{Buf as _, Bytes};
use http::header::HeaderName;
use reqwest::{Client, Url};
use tokio::sync::{Notify, mpsc};
use tokio_stream::StreamExt as _;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use wind_quic::{
	PrefixedRecv, QuicConnection,
	h3_adapter::{self, H3Conn},
};

use super::MasqueradeConfig;

/// Cap on a single proxied request body (bounds per-request memory).
const MAX_REQUEST_BODY_SIZE: usize = 16 * 1024 * 1024;
/// Cap on a single proxied response body.
const MAX_RESPONSE_BODY_SIZE: usize = 64 * 1024 * 1024;
/// Upstream request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Build the reverse-proxy client. `reqwest` validates its options here and
/// reports an unusable combination as an error, so this stays fallible instead
/// of panicking: a panic would abort the masquerade task rather than tear the
/// connection down.
fn build_client() -> Result<Client, reqwest::Error> {
	build_client_with(REQUEST_TIMEOUT, None)
}

/// [`build_client`] with an injectable user agent, so tests can drive the same
/// `ClientBuilder` chain through an option `reqwest` genuinely rejects (a
/// header value with a control byte) instead of only exercising the happy path.
fn build_client_with(timeout: Duration, user_agent: Option<&str>) -> Result<Client, reqwest::Error> {
	let builder = Client::builder().timeout(timeout);
	match user_agent {
		Some(value) => builder.user_agent(value).build(),
		None => builder.build(),
	}
}

/// The shared reverse-proxy client. Upstream-independent (it dials per request
/// URL), built once — probers are rare. `reqwest` owns TLS, roots, pooling and
/// ALPN, so there is no rustls/provider plumbing here.
///
/// The result is memoized, so a client that cannot be built at all is not
/// re-attempted (and re-reported) per request.
fn client() -> eyre::Result<Client> {
	static CLIENT: OnceLock<Result<Client, String>> = OnceLock::new();
	#[cfg(test)]
	if test_rejects_user_agent() {
		return build_client_with(REQUEST_TIMEOUT, Some(REJECTED_USER_AGENT))
			.map_err(|e| eyre::Report::msg(client_build_error(&e)));
	}
	CLIENT
		.get_or_init(|| build_client().map_err(|e| client_build_error(&e)))
		.clone()
		.map_err(eyre::Report::msg)
}

/// The message every client-build failure carries, so the memoized and the
/// injected path report the same thing.
fn client_build_error(err: &reqwest::Error) -> String {
	format!("could not build the masquerade reverse-proxy client: {err}")
}

/// A user agent `http` rejects (`0x0A` is a control byte), so the injection
/// below makes `reqwest` fail the build for real.
#[cfg(test)]
const REJECTED_USER_AGENT: &str = "\n";

/// Whether the test-only injection is armed: `reqwest` reports a build error
/// for [`REJECTED_USER_AGENT`].
#[cfg(test)]
fn test_rejects_user_agent() -> bool {
	static ARMED: OnceLock<bool> = OnceLock::new();
	*ARMED.get_or_init(|| build_client_with(REQUEST_TIMEOUT, Some(REJECTED_USER_AGENT)).is_err())
}

/// Run the HTTP/3 masquerade server over `conn`. The per-stream router in
/// [`super`] feeds h3 streams to `recv_rx` / `bidi_rx` and fires `go` on the
/// first one. We wait for `go` before building the h3 server (whose setup opens
/// our control stream + SETTINGS), so a pure-TUIC connection — which never
/// fires `go` — never has h3 streams opened on it. Returns when the peer
/// disconnects or `cancel` fires.
pub async fn run_masquerade<C: QuicConnection>(
	conn: C,
	recv_rx: mpsc::UnboundedReceiver<PrefixedRecv<C::RecvStream>>,
	bidi_rx: mpsc::UnboundedReceiver<(C::SendStream, PrefixedRecv<C::RecvStream>)>,
	go: Arc<Notify>,
	cfg: MasqueradeConfig,
	cancel: CancellationToken,
) -> eyre::Result<()> {
	tokio::select! {
		_ = cancel.cancelled() => return Ok(()),
		// Bind the parked task to the connection's lifetime. A pure-TUIC peer
		// never fires `go`, and the per-connection teardown only cancels the
		// acceptor/UDP child tokens — not this `cancel` — so on a normal peer
		// disconnect this task (holding a clone of the connection handle) would
		// otherwise stay parked until global shutdown, leaking one task and one
		// connection handle per closed connection when masquerade is enabled.
		_ = conn.closed() => return Ok(()),
		_ = go.notified() => {}
	}

	let backend = Url::parse(&cfg.upstream).map_err(|e| eyre::eyre!("invalid masquerade upstream {:?}: {e}", cfg.upstream))?;
	// Resolved once, up front (and *outside* the parked `select!`): a client
	// that cannot be built is a plain task error, which `spawn_h3_router`
	// turns into a connection close. Building it per request instead would
	// only turn it into a `502`.
	let client = Arc::new(client()?);

	let adapter = h3_adapter::server_connection(conn, recv_rx, bidi_rx);
	let mut h3conn = h3::server::Connection::new(adapter)
		.await
		.map_err(|e| eyre::eyre!("h3 server setup failed: {e}"))?;

	loop {
		let resolver = tokio::select! {
			_ = cancel.cancelled() => break,
			res = h3conn.accept() => match res {
				Ok(Some(r)) => r,
				Ok(None) => break,
				Err(e) => {
					debug!("masquerade accept ended: {e}");
					break;
				}
			},
		};

		let backend = backend.clone();
		let client = client.clone();
		tokio::spawn(async move {
			if let Err(e) = handle_request::<C>(resolver, &client, &backend).await {
				debug!("masquerade request error: {e:?}");
			}
		});
	}

	Ok(())
}

/// Resolve one HTTP/3 request and reverse-proxy it; on any failure answer `502`
/// like a real web server rather than resetting the stream.
async fn handle_request<C: QuicConnection>(
	resolver: h3::server::RequestResolver<H3Conn<C>, Bytes>,
	client: &Arc<Client>,
	backend: &Url,
) -> eyre::Result<()> {
	let (request, mut stream) = resolver.resolve_request().await?;

	if let Err(e) = forward(client, backend, request, &mut stream).await {
		debug!("masquerade upstream failed: {e}");
		let resp = http::Response::builder()
			.status(http::StatusCode::BAD_GATEWAY)
			.body(())
			.expect("502 response is valid");
		let _ = stream.send_response(resp).await;
		let _ = stream.finish().await;
	}
	Ok(())
}

/// Translate the HTTP/3 request into a `reqwest` call to `backend`, stream the
/// response body back (size-capped), and finish the stream.
async fn forward<S>(
	client: &Client,
	backend: &Url,
	request: http::Request<()>,
	stream: &mut h3::server::RequestStream<S, Bytes>,
) -> eyre::Result<()>
where
	S: h3::quic::BidiStream<Bytes>,
{
	let target = rewrite_target(backend, request.uri())?;
	let mut req = client.request(request.method().clone(), target);
	for (name, value) in request.headers() {
		if is_forwardable(name) {
			req = req.header(name, value);
		}
	}

	let body = read_request_body(stream).await?;
	if !body.is_empty() {
		req = req.body(body);
	}

	let resp = req.send().await?;

	// Reject an over-cap response *before* committing the response head, so the
	// `502` fallback in `handle_request` can still fire cleanly. (A streamed
	// response that under-reports or omits `Content-Length` is still bounded
	// mid-relay below.)
	if let Some(len) = resp.content_length()
		&& len > MAX_RESPONSE_BODY_SIZE as u64
	{
		return Err(eyre::eyre!(
			"upstream Content-Length {len} exceeds {MAX_RESPONSE_BODY_SIZE} bytes"
		));
	}

	// From here the response head is committed to the h3 stream. A later
	// failure (over-cap body, send error) must NOT become a second `502`
	// response on a stream that already sent `200` — that yields a
	// truncated-but-"successful" reply. Reset the stream instead, so the
	// prober sees an aborted response.
	if let Err(e) = relay_response(stream, resp).await {
		debug!("masquerade response failed after the head was sent; resetting h3 stream: {e}");
		stream.stop_stream(h3::error::Code::H3_INTERNAL_ERROR);
	}
	Ok(())
}

/// Send the upstream response head, stream its body back (size-capped), and
/// finish the stream. Any error leaves the already-committed response
/// incomplete; the caller resets the stream rather than sending a fresh status.
async fn relay_response<S>(stream: &mut h3::server::RequestStream<S, Bytes>, resp: reqwest::Response) -> eyre::Result<()>
where
	S: h3::quic::BidiStream<Bytes>,
{
	let mut builder = http::Response::builder().status(resp.status());
	for (name, value) in resp.headers() {
		if is_forwardable(name) {
			builder = builder.header(name, value);
		}
	}
	stream.send_response(builder.body(())?).await?;

	let mut sent = 0usize;
	let mut body_stream = resp.bytes_stream();
	while let Some(chunk) = body_stream.next().await {
		let chunk = chunk?;
		sent += chunk.len();
		if sent > MAX_RESPONSE_BODY_SIZE {
			return Err(eyre::eyre!("upstream response body exceeds {MAX_RESPONSE_BODY_SIZE} bytes"));
		}
		if !chunk.is_empty() {
			stream.send_data(chunk).await?;
		}
	}
	stream.finish().await?;
	Ok(())
}

/// Drain the HTTP/3 request body (usually empty for a probe's GET), capped.
async fn read_request_body<S>(stream: &mut h3::server::RequestStream<S, Bytes>) -> eyre::Result<Bytes>
where
	S: h3::quic::BidiStream<Bytes>,
{
	let mut body = Vec::new();
	while let Some(mut chunk) = stream.recv_data().await? {
		let n = chunk.remaining();
		body.extend_from_slice(chunk.copy_to_bytes(n).as_ref());
		if body.len() > MAX_REQUEST_BODY_SIZE {
			return Err(eyre::eyre!("request body exceeds {MAX_REQUEST_BODY_SIZE} bytes"));
		}
	}
	let _ = stream.recv_trailers().await?;
	Ok(Bytes::from(body))
}

/// Point the request at the backend: keep the backend's scheme/host/port,
/// append the incoming path and query.
fn rewrite_target(backend: &Url, uri: &http::Uri) -> eyre::Result<Url> {
	let path_and_query = uri.path_and_query().map(|v| v.as_str()).unwrap_or("/");
	let mut target = backend.clone();
	target.set_path("");
	target.set_query(None);
	Ok(target.join(path_and_query)?)
}

/// Whether a header may cross the proxy: drops hop-by-hop headers (RFC 9110
/// §7.6.1) plus framing headers each side manages itself.
fn is_forwardable(name: &HeaderName) -> bool {
	!matches!(
		name.as_str().to_ascii_lowercase().as_str(),
		"connection"
			| "keep-alive"
			| "proxy-connection"
			| "transfer-encoding"
			| "upgrade"
			| "te"
			| "trailer"
			| "host"
			| "content-length"
	)
}

#[cfg(test)]
mod tests {
	use std::time::Duration;

	use super::*;
	use crate::server::tests::DummyConn;

	/// The production builder accepts the options this module configures, so
	/// the fallible path stays a real (not a never-taken) branch.
	#[test]
	fn the_reverse_proxy_client_builds_with_the_configured_options() {
		build_client().expect("a timeout-only reqwest client builds");
	}

	/// `build_client` reporting an error must reach the caller as `Err`, not as
	/// a panic: the previous `.expect(...)` aborted the masquerade task,
	/// leaving the connection neither served nor closed.
	#[test]
	fn a_client_build_failure_is_reported_as_an_error() {
		assert!(test_rejects_user_agent(), "the injection must make reqwest reject the build");
		let rejected =
			build_client_with(REQUEST_TIMEOUT, Some(REJECTED_USER_AGENT)).expect_err("an unusable option must fail the build");

		let err = client().expect_err("the failed client build must surface as an error");
		let message = err.to_string();
		assert_eq!(message, client_build_error(&rejected), "the error must name the failing step");
		assert!(
			message.contains(&rejected.to_string()),
			"the error must carry reqwest's own message, got {message}"
		);
		assert_eq!(
			message,
			client()
				.expect_err("the failure stays memoized instead of being rebuilt per request")
				.to_string(),
			"the memoized failure must be identical"
		);
	}

	/// A client that cannot be built is not a per-request 502: the masquerade
	/// task fails, which is what closes the connection instead of stranding
	/// it.
	#[tokio::test]
	async fn a_client_build_failure_fails_the_task_rather_than_stranding_the_connection() {
		let (_uni_tx, uni_rx) = mpsc::unbounded_channel();
		let (_bidi_tx, bidi_rx) = mpsc::unbounded_channel();
		let go = Arc::new(Notify::new());
		go.notify_one();
		let cancel = CancellationToken::new();

		let result = tokio::time::timeout(
			Duration::from_secs(5),
			run_masquerade(
				DummyConn::default(),
				uni_rx,
				bidi_rx,
				go,
				MasqueradeConfig {
					upstream: "https://example.com".to_owned(),
				},
				cancel,
			),
		)
		.await
		.expect("the masquerade must resolve, not hang or panic, when the client cannot be built");

		let err = result.expect_err("the task must report the failed client build");
		assert!(
			err.to_string().contains("masquerade reverse-proxy client"),
			"the task must report the client-build failure itself, got {err}"
		);
	}
}
