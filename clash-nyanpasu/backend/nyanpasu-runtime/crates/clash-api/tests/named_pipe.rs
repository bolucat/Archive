#![cfg(windows)]

use std::{path::PathBuf, time::SystemTime};

use axum::{
    Router,
    body::Body,
    extract::{FromRequestParts, WebSocketUpgrade, ws::Message as AxumMessage},
    http::{HeaderMap, request::Parts},
    response::{IntoResponse, Response},
    routing::get,
};
use clash_api::{Client, Host, Traffic};
use futures_util::StreamExt;
use hyper::server::conn::http1;
use hyper_util::{rt::TokioIo, service::TowerToHyperService};
use reqwest_websocket::Message;
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
    assert_eq!(stream.next().await.unwrap().unwrap().up_total.get(), 3);

    let mut websocket = client.traffic_ws().await.unwrap();
    let Message::Text(text) = websocket.next().await.unwrap().unwrap() else {
        panic!("expected a text frame");
    };
    let traffic: Traffic = serde_json::from_str(&text).unwrap();
    assert_eq!(traffic.down_total.get(), 4);
    accept_loop.abort();
}
