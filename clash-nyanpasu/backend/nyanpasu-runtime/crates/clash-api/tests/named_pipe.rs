#![cfg(windows)]

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use axum::{
    Router,
    body::Body,
    extract::{FromRequestParts, WebSocketUpgrade, ws::Message as AxumMessage},
    http::{HeaderMap, request::Parts},
    response::{IntoResponse, Response},
    routing::get,
};
use clash_api::{Client, Host};
use futures_util::StreamExt;
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use tokio::net::windows::named_pipe::ServerOptions;

struct OptionalWebSocket(Option<WebSocketUpgrade>);

impl<S> FromRequestParts<S> for OptionalWebSocket
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            WebSocketUpgrade::from_request_parts(parts, state)
                .await
                .ok(),
        ))
    }
}

fn unique_pipe_name() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    PathBuf::from(format!(
        r"\\.\pipe\clash-api-{}-{nonce}",
        std::process::id()
    ))
}

fn spawn_pipe_server(name: PathBuf, app: Router) -> tokio::task::JoinHandle<()> {
    let first = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .unwrap();
    tokio::spawn(async move {
        let mut server = first;
        loop {
            server.connect().await.unwrap();
            let connected = server;
            server = ServerOptions::new().create(&name).unwrap();
            let service = TowerToHyperService::new(app.clone());
            tokio::spawn(async move {
                http1::Builder::new()
                    .serve_connection(TokioIo::new(connected), service)
                    .with_upgrades()
                    .await
                    .unwrap();
            });
        }
    })
}

#[tokio::test]
async fn named_pipe_supports_http_and_websocket_without_bearer_auth() {
    async fn traffic(
        OptionalWebSocket(websocket): OptionalWebSocket,
        headers: HeaderMap,
    ) -> Response {
        assert!(!headers.contains_key("authorization"));
        if let Some(websocket) = websocket {
            return websocket
                .on_upgrade(|mut socket| async move {
                    socket
                        .send(AxumMessage::Text(
                            "{\"up\":1,\"down\":2,\"upTotal\":3,\"downTotal\":4}\n".into(),
                        ))
                        .await
                        .unwrap();
                })
                .into_response();
        }

        Body::from("{\"up\":1,\"down\":2,\"upTotal\":3,\"downTotal\":4}\n").into_response()
    }

    let name = unique_pipe_name();
    let app = Router::new().route("/traffic", get(traffic));
    let accept_loop = spawn_pipe_server(name.clone(), app);

    let client = Client::builder(Host::named_pipe(name))
        .secret("must-not-be-sent-to-local-transport")
        .build()
        .unwrap();

    let mut stream = client.traffic().await.unwrap();
    assert_eq!(
        stream
            .next()
            .await
            .unwrap()
            .unwrap()
            .up_total
            .unwrap()
            .get(),
        3
    );

    let mut websocket = client.traffic_ws().await.unwrap();
    let traffic = websocket.next().await.unwrap().unwrap();
    assert_eq!(traffic.down_total.unwrap().get(), 4);
    accept_loop.abort();
}

#[tokio::test]
async fn busy_pipe_rest_mutation_and_websocket_recover_without_business_retries() {
    use axum::{Json, routing::post};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::{net::windows::named_pipe::ClientOptions, time::timeout};

    let name = unique_pipe_name();
    let occupied = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .unwrap();
    let _occupant = ClientOptions::new().open(&name).unwrap();
    occupied.connect().await.unwrap();
    let client = Client::builder(Host::named_pipe(name.clone()))
        .build()
        .unwrap();
    let mutations = Arc::new(AtomicUsize::new(0));
    let handler_mutations = mutations.clone();
    let app = Router::new()
        .route(
            "/version",
            get(|| async { Json(serde_json::json!({"version":"test"})) }),
        )
        .route(
            "/restart",
            post(move || {
                handler_mutations.fetch_add(1, Ordering::SeqCst);
                async { Json(serde_json::json!({"status":"ok"})) }
            }),
        )
        .route(
            "/traffic",
            get(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(|mut socket| async move {
                    socket
                        .send(AxumMessage::Text("{\"up\":1,\"down\":2}".into()))
                        .await
                        .unwrap();
                })
            }),
        );
    let requests = async { tokio::join!(client.version(), client.restart(), client.traffic_ws()) };
    tokio::pin!(requests);
    tokio::select! {
        biased;
        _ = &mut requests => panic!("requests must wait while the pipe is busy"),
        _ = tokio::time::sleep(Duration::from_millis(100)) => {},
    }
    // Replenish the listener while retaining its original occupied instance.
    let first = ServerOptions::new().create(&name).unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    let server = tokio::spawn(async move {
        let mut next = first;
        loop {
            next.connect().await.unwrap();
            let connected = next;
            next = ServerOptions::new().create(&name).unwrap();
            let service = TowerToHyperService::new(app.clone());
            tasks.spawn(async move {
                http1::Builder::new()
                    .serve_connection(TokioIo::new(connected), service)
                    .with_upgrades()
                    .await
                    .unwrap();
            });
        }
    });
    let result = timeout(Duration::from_secs(3), async {
        let (version, restart, websocket) = requests.await;
        assert_eq!(version.unwrap().version, "test");
        restart.unwrap();
        assert_eq!(mutations.load(Ordering::SeqCst), 1);
        let mut websocket = websocket.unwrap();
        assert!(websocket.next().await.unwrap().is_ok());
    })
    .await;
    server.abort();
    let _ = server.await;
    result.unwrap();
}

#[tokio::test]
async fn exhausted_pipe_budget_does_not_enter_the_business_retry_policy() {
    #[derive(Debug)]
    struct UnexpectedRetry;
    impl clash_api::RetryPolicy for UnexpectedRetry {
        fn delays(
            &self,
            _: &clash_api::RequestMetadata,
        ) -> Box<dyn Iterator<Item = Duration> + Send> {
            Box::new(std::iter::repeat(Duration::ZERO))
        }

        fn is_retryable(&self, _: &clash_api::RequestMetadata, _: &clash_api::Error) -> bool {
            panic!("pipe contention must not receive another retry budget");
        }
    }
    let name = unique_pipe_name();
    let occupied = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .unwrap();
    let _occupant = tokio::net::windows::named_pipe::ClientOptions::new()
        .open(&name)
        .unwrap();
    occupied.connect().await.unwrap();
    let client = Client::builder(Host::named_pipe(name))
        .retry_policy(UnexpectedRetry)
        .build()
        .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(3), client.version())
        .await
        .unwrap()
        .unwrap_err();
    assert!(nyanpasu_utils::reqwest_ext::is_named_pipe_busy(&error));
}

#[tokio::test]
async fn named_pipe_mutations_do_not_follow_redirects() {
    use axum::{
        http::{StatusCode, header::LOCATION},
        routing::post,
    };
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let name = unique_pipe_name();
    let followed = Arc::new(AtomicUsize::new(0));
    let handler_followed = followed.clone();
    let app = Router::new()
        .route(
            "/restart",
            post(|| async { (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, "/redirected")]) }),
        )
        .route(
            "/redirected",
            post(move || {
                handler_followed.fetch_add(1, Ordering::SeqCst);
                async { axum::Json(serde_json::json!({"status":"ok"})) }
            }),
        );
    let server = spawn_pipe_server(name.clone(), app);
    let client = Client::builder(Host::named_pipe(name)).build().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), client.restart()).await;
    server.abort();
    let _ = server.await;
    assert!(matches!(
        result.unwrap(),
        Err(clash_api::Error::HttpStatus {
            status: StatusCode::TEMPORARY_REDIRECT,
            ..
        })
    ));
    assert_eq!(followed.load(Ordering::SeqCst), 0);
}
