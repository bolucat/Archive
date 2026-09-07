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
    let decoded = websocket.next().await.unwrap().unwrap();
    assert_eq!(decoded.memory, Some(3));
    server.abort();
}

#[tokio::test]
async fn websocket_controls_binary_decode_errors_and_close_are_typed() {
    async fn websocket(ws: WebSocketUpgrade) -> impl IntoResponse {
        ws.on_upgrade(|mut socket| async move {
            socket
                .send(AxumMessage::Ping(vec![1, 2].into()))
                .await
                .unwrap();
            // Polling the typed stream must service ping/pong without exposing it.
            assert!(matches!(
                socket.recv().await,
                Some(Ok(AxumMessage::Pong(_)))
            ));
            socket
                .send(AxumMessage::Text("invalid json".into()))
                .await
                .unwrap();
            socket
                .send(AxumMessage::Binary(br#"{"up":4,"down":9}"#.to_vec().into()))
                .await
                .unwrap();
            socket
                .send(AxumMessage::Text(r#"{"up":5,"down":10}"#.into()))
                .await
                .unwrap();
            socket.send(AxumMessage::Close(None)).await.unwrap();
        })
    }
    let (address, server) = spawn_server(Router::new().route("/traffic", get(websocket))).await;
    let client = Client::new_http(address).unwrap();
    let mut stream = client.traffic_ws().await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        assert!(matches!(
            stream.next().await,
            Some(Err(clash_api::Error::Decode {
                operation: "traffic_ws",
                ..
            }))
        ));
        let binary = stream.next().await.unwrap().unwrap();
        assert_eq!(binary.up.get(), 4);
        assert_eq!(binary.up_total, None);
        assert_eq!(stream.next().await.unwrap().unwrap().down.get(), 10);
        assert!(stream.next().await.is_none());
        assert!(stream.next().await.is_none());
    })
    .await
    .unwrap();
    server.abort();
}

#[tokio::test]
async fn dropping_a_subscription_releases_its_idle_socket() {
    let (closed, received) = tokio::sync::oneshot::channel();
    let closed = std::sync::Arc::new(tokio::sync::Mutex::new(Some(closed)));
    let app = Router::new().route(
        "/memory",
        get(move |ws: WebSocketUpgrade| {
            let closed = closed.clone();
            async move {
                ws.on_upgrade(move |mut socket| async move {
                    let _ = socket.recv().await;
                    closed.lock().await.take().unwrap().send(()).unwrap();
                })
            }
        }),
    );
    let (address, server) = spawn_server(app).await;
    let stream = Client::new_http(address)
        .unwrap()
        .memory_ws()
        .await
        .unwrap();
    drop(stream);
    tokio::time::timeout(Duration::from_secs(2), received)
        .await
        .unwrap()
        .unwrap();
    server.abort();
}

#[test]
fn common_connections_preserve_absence_unknown_enums_and_extensions() {
    let input = serde_json::json!({
        "downloadTotal": 10, "uploadTotal": 20,
        "connections": [{
            "id": "550e8400-e29b-41d4-a716-446655440000", "upload": 1, "download": 2,
            "start": "2026-09-07T12:00:00Z", "chains": ["DIRECT"], "rule": "Match", "rulePayload": "",
            "metadata": {"network": "future-network", "type": "FutureInbound", "dnsMode": "future-dns", "sourcePort": 1234,
                "destinationPort": "443", "extension": {"nested": [1, "two"]}},
            "newConnectionField": true
        }]
    });
    let snapshot: clash_api::ConnectionsSnapshot = serde_json::from_value(input).unwrap();
    assert_eq!(snapshot.memory, None);
    let connection = &snapshot.connections.as_ref().unwrap()[0];
    assert_eq!(connection.provider_chains, None);
    let metadata = connection.metadata.as_ref().unwrap();
    assert_eq!(metadata.source_port, Some(1234));
    assert_eq!(metadata.destination_port, Some(443));
    assert_eq!(metadata.process_path, None);
    assert!(
        matches!(&metadata.network, Some(clash_api::ConfigEnum::Unknown(value)) if value == "future-network")
    );
    let serialized = serde_json::to_value(connection).unwrap();
    assert_eq!(serialized["metadata"]["type"], "FutureInbound");
    assert_eq!(serialized["metadata"]["dnsMode"], "future-dns");
    assert_eq!(serialized["metadata"]["extension"]["nested"][1], "two");
    assert_eq!(serialized["newConnectionField"], true);
    assert!(serialized.get("providerChains").is_none());
    for port in [
        serde_json::json!(-1),
        serde_json::json!(65536),
        serde_json::json!("invalid"),
    ] {
        assert!(
            serde_json::from_value::<clash_api::ConnectionMetadata>(
                serde_json::json!({"sourcePort": port})
            )
            .is_err()
        );
    }
    let log: clash_api::LogEntry =
        serde_json::from_str(r#"{"type":"trace","payload":"hello"}"#).unwrap();
    assert_eq!(log.level.as_str(), "trace");
    let memory: clash_api::Memory = serde_json::from_str(r#"{"inuse":12}"#).unwrap();
    assert_eq!(memory.os_limit, 0);
}

#[test]
fn decimal_string_samples_keep_the_existing_controller_compatibility() {
    let memory: clash_api::Memory =
        serde_json::from_str(r#"{"inuse":"12","oslimit":"24"}"#).unwrap();
    assert_eq!(memory.in_use, 12);
    assert_eq!(memory.os_limit, 24);
    let traffic: clash_api::Traffic = serde_json::from_str(r#"{"up":"3","down":"4"}"#).unwrap();
    assert_eq!(traffic.up.get(), 3);
    assert_eq!(traffic.down.get(), 4);
    assert!(serde_json::from_str::<clash_api::Memory>(r#"{"inuse":"invalid"}"#).is_err());
}
