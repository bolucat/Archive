use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    body::{Body, Bytes as BodyBytes},
    extract::{State, WebSocketUpgrade, ws::Message as AxumMessage},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use clash_api::{Client, Error, ExponentialRetry, Host, Traffic};
use futures_util::{StreamExt, stream};
use reqwest_websocket::Message;

const TRAFFIC_ONE: &str = r#"{"up":1,"down":2,"upTotal":3,"downTotal":4}"#;

async fn spawn_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (address.to_string(), task)
}

#[tokio::test]
async fn traffic_http_decodes_json_across_arbitrary_chunks() {
    async fn handler(headers: HeaderMap) -> Response {
        assert_eq!(headers["authorization"], "Bearer controller-secret");
        let chunks = stream::iter([
            Ok::<_, Infallible>(BodyBytes::from_static(br#"{"up":1,"dow"#)),
            Ok(BodyBytes::from_static(
                br#"n":2,"upTotal":3,"downTotal":4}
{"up":5,"down":6,"upTotal":7,"downTotal":8}
"#,
            )),
        ]);
        Response::builder()
            .header("content-type", "application/json")
            .body(Body::from_stream(chunks))
            .unwrap()
    }

    let (address, server) = spawn_server(Router::new().route("/traffic", get(handler))).await;
    let client = Client::builder(Host::http(address).unwrap())
        .secret("controller-secret")
        .build()
        .unwrap();

    let mut stream = client.traffic().await.unwrap();
    let first = stream.next().await.unwrap().unwrap();
    let second = stream.next().await.unwrap().unwrap();

    assert_eq!(first.up.get(), 1);
    assert_eq!(first.down_total.get(), 4);
    assert_eq!(second.up_total.get(), 7);
    assert!(stream.next().await.is_none());
    server.abort();
}

#[tokio::test]
async fn traffic_ws_returns_the_raw_socket_and_preserves_the_frame() {
    async fn handler(ws: WebSocketUpgrade, headers: HeaderMap) -> impl IntoResponse {
        assert_eq!(headers["authorization"], "Bearer controller-secret");
        ws.on_upgrade(|mut socket| async move {
            socket
                .send(AxumMessage::Text(format!("{TRAFFIC_ONE}\n").into()))
                .await
                .unwrap();
        })
    }

    let (address, server) = spawn_server(Router::new().route("/traffic", get(handler))).await;
    let client = Client::builder(Host::http(address).unwrap())
        .secret("controller-secret")
        .build()
        .unwrap();

    let mut websocket: reqwest_websocket::WebSocket = client.traffic_ws().await.unwrap();
    let message = websocket.next().await.unwrap().unwrap();
    let Message::Text(text) = message else {
        panic!("expected a text frame");
    };
    let traffic: Traffic = serde_json::from_str(&text).unwrap();

    assert_eq!(traffic.up.get(), 1);
    assert_eq!(traffic.down_total.get(), 4);
    server.abort();
}

#[tokio::test]
async fn retry_policy_retries_only_transient_request_errors() {
    async fn handler(State(attempts): State<Arc<AtomicUsize>>) -> Response {
        if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"message":"warming up"}"#,
            )
                .into_response();
        }

        Body::from(format!("{TRAFFIC_ONE}\n")).into_response()
    }

    let attempts = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/traffic", get(handler))
        .with_state(attempts.clone());
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .retry_policy(ExponentialRetry::new(
            2,
            Duration::from_millis(1),
            Duration::from_millis(1),
        ))
        .build()
        .unwrap();

    let mut traffic = client.traffic().await.unwrap();
    assert_eq!(traffic.next().await.unwrap().unwrap().up.get(), 1);
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    server.abort();
}

#[tokio::test]
async fn retry_policy_covers_the_websocket_handshake_but_not_the_open_socket() {
    async fn handler(State(attempts): State<Arc<AtomicUsize>>, ws: WebSocketUpgrade) -> Response {
        if attempts.fetch_add(1, Ordering::SeqCst) < 2 {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"message":"warming up"}"#,
            )
                .into_response();
        }

        ws.on_upgrade(|mut socket| async move {
            socket
                .send(AxumMessage::Text(format!("{TRAFFIC_ONE}\n").into()))
                .await
                .unwrap();
        })
        .into_response()
    }

    let attempts = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/traffic", get(handler))
        .with_state(attempts.clone());
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .retry_policy(ExponentialRetry::new(
            2,
            Duration::from_millis(1),
            Duration::from_millis(1),
        ))
        .build()
        .unwrap();

    let mut websocket = client.traffic_ws().await.unwrap();
    assert!(matches!(
        websocket.next().await.unwrap().unwrap(),
        Message::Text(_)
    ));
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    server.abort();
}

#[tokio::test]
async fn unauthorized_response_is_not_retried_and_retains_mihomo_message() {
    async fn handler(State(attempts): State<Arc<AtomicUsize>>) -> impl IntoResponse {
        attempts.fetch_add(1, Ordering::SeqCst);
        (StatusCode::UNAUTHORIZED, r#"{"message":"Unauthorized"}"#)
    }

    let attempts = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/traffic", get(handler))
        .with_state(attempts.clone());
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .retry_policy(ExponentialRetry::new(
            3,
            Duration::from_millis(1),
            Duration::from_millis(1),
        ))
        .build()
        .unwrap();

    let error = client.traffic().await.unwrap_err();
    assert!(matches!(
        error,
        Error::HttpStatus {
            status: StatusCode::UNAUTHORIZED,
            ..
        }
    ));
    assert_eq!(
        error.error_body().and_then(|body| body.message()),
        Some("Unauthorized")
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    server.abort();
}
