use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::{Query, WebSocketUpgrade, ws::Message as AxumMessage},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::NaiveTime;
use clash_api::{Client, ConnectionStreamQuery, LogLevel, LogQuery, StructuredLogLevel};
use futures_util::StreamExt;
use indexmap::IndexMap;
use reqwest_websocket::Message;

async fn spawn_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (address.to_string(), task)
}

#[tokio::test]
async fn memory_and_structured_logs_use_distinct_typed_streams() {
    async fn memory() -> Response {
        Body::from("{\"inuse\":0,\"oslimit\":0}\n{\"inuse\":42,\"oslimit\":7}\n").into_response()
    }

    async fn logs(Query(query): Query<IndexMap<String, String>>) -> Response {
        assert_eq!(query.get("level").map(String::as_str), Some("warning"));
        assert_eq!(query.get("format").map(String::as_str), Some("structured"));
        Body::from(
            "{\"time\":\"12:34:56\",\"level\":\"warn\",\"message\":\"hello\",\"fields\":[]}\n",
        )
        .into_response()
    }

    let app = Router::new()
        .route("/memory", get(memory))
        .route("/logs", get(logs));
    let (address, server) = spawn_server(app).await;
    let client = Client::new_http(address).unwrap();

    let mut memory = client.memory().await.unwrap();
    assert_eq!(memory.next().await.unwrap().unwrap().in_use, 0);
    assert_eq!(memory.next().await.unwrap().unwrap().in_use, 42);

    let mut logs = client
        .structured_logs(LogQuery::new(LogLevel::Warning))
        .await
        .unwrap();
    let log = logs.next().await.unwrap().unwrap();
    assert_eq!(log.time, NaiveTime::from_hms_opt(12, 34, 56).unwrap());
    assert_eq!(log.level, StructuredLogLevel::Warn);
    server.abort();
}

#[tokio::test]
async fn connections_http_is_a_snapshot_and_websocket_honors_interval() {
    const SNAPSHOT: &str = r#"{"downloadTotal":-1,"uploadTotal":2,"connections":null,"memory":3}"#;

    async fn snapshot() -> Response {
        Body::from(SNAPSHOT).into_response()
    }

    let (address, server) = spawn_server(Router::new().route("/connections", get(snapshot))).await;
    let client = Client::new_http(address).unwrap();
    let snapshot = client.connections().await.unwrap();
    assert_eq!(snapshot.download_total, -1);
    assert_eq!(snapshot.connections, None);
    server.abort();

    async fn websocket(
        Query(query): Query<IndexMap<String, String>>,
        ws: WebSocketUpgrade,
    ) -> impl IntoResponse {
        assert_eq!(query.get("interval").map(String::as_str), Some("250"));
        ws.on_upgrade(|mut socket| async move {
            socket
                .send(AxumMessage::Text(format!("{SNAPSHOT}\n").into()))
                .await
                .unwrap();
        })
    }

    let (address, server) = spawn_server(Router::new().route("/connections", get(websocket))).await;
    let client = Client::new_http(address).unwrap();
    let mut websocket = client
        .connections_ws(ConnectionStreamQuery::new(Duration::from_millis(250)).unwrap())
        .await
        .unwrap();
    let Message::Text(frame) = websocket.next().await.unwrap().unwrap() else {
        panic!("expected text frame");
    };
    let decoded: clash_api::ConnectionsSnapshot = serde_json::from_str(&frame).unwrap();
    assert_eq!(decoded.memory, 3);
    server.abort();
}
