//! Characterize ERROR_PIPE_BUSY without touching the installed service.
//! Holding the listener before accept models a daemon that has not yet
//! replenished its available pipe instance; no timing-dependent load is needed.

use super::*;
use std::error::Error;

use nyanpasu_ipc::{
    api::{
        contract::{CoreStop, CoreV2ApiConnection, CoreV2Status, IpcOperation},
        core::v2::CoreApiConnection,
        status::CoreControllerInfo,
    },
    server::RegisterOperation,
};
use tokio::{net::windows::named_pipe::ClientOptions, time::timeout};

const DEADLINE: Duration = Duration::from_secs(5);

fn assert_pipe_busy(error: ClientError, expected_operation: &str) {
    let ClientError::Request { operation, source } = &error else {
        panic!("expected a transport failure, got {error:?}");
    };
    assert_eq!(*operation, expected_operation);
    assert!(source.is_connect(), "{error:?}");
    let mut cause: Option<&(dyn Error + 'static)> = Some(&error);
    while let Some(current) = cause {
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            assert_eq!(io.raw_os_error(), Some(231), "{error:?}");
            return;
        }
        cause = current.source();
    }
    panic!("missing Windows IO error in source chain: {error:?}");
}

#[tokio::test]
async fn busy_pipe_rejects_concurrent_requests_but_the_same_client_recovers() {
    let name = format!("nyanpasu-pipe-busy-test-{}", std::process::id());
    let path = socket_path(&name);
    let listener = ListenerOptions::new()
        .name(path.as_str().to_fs_name::<GenericFilePath>().unwrap())
        .nonblocking(ListenerNonblockingMode::Both)
        .create_tokio()
        .unwrap();
    // The pipe exists and is accessible, but its only instance is occupied.
    // Deliberately do not accept until all requests have completed.
    let occupant = ClientOptions::new().open(&path).unwrap();
    let client = Client::new(&name).unwrap();
    let (connections, status) = timeout(DEADLINE, async {
        tokio::join!(
            futures_util::future::join_all(
                (0..4).map(|_| client.call::<CoreV2ApiConnection>(None))
            ),
            client.call::<CoreV2Status>(None),
        )
    })
    .await
    .expect("busy pipe requests must finish within the test deadline");
    for result in connections {
        assert_pipe_busy(result.unwrap_err(), CoreV2ApiConnection::PATH);
    }
    assert_pipe_busy(status.unwrap_err(), CoreV2Status::PATH);

    // Accept explicitly replenishes the listener before the recovery request.
    let accepted = timeout(DEADLINE, listener.accept()).await.unwrap().unwrap();
    drop(accepted);
    drop(occupant);
    let expected = CoreApiConnection {
        instance_id: "pipe-busy-recovery".into(),
        controller: CoreControllerInfo::Http("http://127.0.0.1:9090/".into()),
        secret: None,
    };
    let response = expected.clone();
    let router = Router::new().register(CoreV2ApiConnection, move || {
        let response = response.clone();
        async move { Json(RBuilder::success(Some(response))) }
    });
    let server = tokio::spawn(async move {
        axum::serve(IpcListener(listener, path), router)
            .await
            .unwrap();
    });
    let recovered = timeout(DEADLINE, client.call::<CoreV2ApiConnection>(None)).await;
    server.abort();
    let _ = server.await;
    assert_eq!(recovered.unwrap().unwrap().data.flatten(), Some(expected));
}

#[tokio::test]
async fn concurrent_requests_wait_for_a_busy_pipe_without_resubmission() {
    let name = format!("nyanpasu-pipe-retry-test-{}", std::process::id());
    let path = socket_path(&name);
    let listener = ListenerOptions::new()
        .name(path.as_str().to_fs_name::<GenericFilePath>().unwrap())
        .nonblocking(ListenerNonblockingMode::Both)
        .create_tokio()
        .unwrap();
    let occupant = ClientOptions::new().open(&path).unwrap();
    let client = Client::new(&name).unwrap();
    let requests = async {
        tokio::join!(
            futures_util::future::join_all(
                (0..4).map(|_| client.call::<CoreV2ApiConnection>(None))
            ),
            client.call::<CoreV2Status>(None),
            client.call::<CoreStop>(None),
            client.events(),
        )
    };
    tokio::pin!(requests);
    tokio::select! {
        biased;
        result = &mut requests => panic!("requests must wait for the busy pipe: {result:?}"),
        _ = tokio::time::sleep(Duration::from_millis(100)) => {},
    }

    // Release the accept gate only after the original requests have started.
    let accepted = timeout(DEADLINE, listener.accept()).await.unwrap().unwrap();
    drop(accepted);
    drop(occupant);
    let expected = CoreApiConnection {
        instance_id: "pipe-retry-recovery".into(),
        controller: CoreControllerInfo::Http("http://127.0.0.1:9090/".into()),
        secret: None,
    };
    let response = expected.clone();
    let stop_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let handler_count = stop_count.clone();
    let router = Router::new()
        .register(CoreV2ApiConnection, move || {
            let response = response.clone();
            async move { Json(RBuilder::success(Some(response))) }
        })
        .register(CoreV2Status, || async {
            Json(RBuilder::success(test_snapshot()))
        })
        .register(CoreStop, move || {
            handler_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async { Json(RBuilder::success(())) }
        })
        .route(
            EVENT_URI,
            get(|ws: WebSocketUpgrade| async {
                ws.on_upgrade(|mut socket| async move {
                    socket
                        .send(Message::Text(
                            serde_json::to_string(&Event::CoreStatusChanged(test_snapshot()))
                                .unwrap()
                                .into(),
                        ))
                        .await
                        .unwrap();
                })
            }),
        );
    let server = tokio::spawn(async move {
        axum::serve(IpcListener(listener, path), router)
            .await
            .unwrap();
    });
    let recovered = timeout(DEADLINE, requests).await;
    server.abort();
    let _ = server.await;
    let (connections, status, stop, events) = recovered.unwrap();
    for result in connections {
        assert_eq!(result.unwrap().data.flatten(), Some(expected.clone()));
    }
    assert_eq!(
        serde_json::to_value(status.unwrap().data.unwrap()).unwrap(),
        serde_json::to_value(test_snapshot()).unwrap(),
    );
    stop.unwrap();
    assert_eq!(stop_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    let mut events = events.unwrap();
    assert!(matches!(
        timeout(DEADLINE, events.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        Event::CoreStatusChanged(_),
    ));
}

#[tokio::test]
async fn pipe_ipc_does_not_follow_redirects() {
    use axum::http::header::LOCATION;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let name = format!("nyanpasu-pipe-redirect-test-{}", std::process::id());
    let followed = Arc::new(AtomicUsize::new(0));
    let handler_followed = followed.clone();
    let router = Router::new()
        .route(
            CoreStop::PATH,
            post(|| async { (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, "/redirected")]) }),
        )
        .route(
            "/redirected",
            post(move || {
                handler_followed.fetch_add(1, Ordering::SeqCst);
                async { Json(RBuilder::success(())) }
            }),
        );
    let shutdown = spawn_server(&name, router).unwrap();
    let result = timeout(DEADLINE, Client::new(&name).unwrap().call::<CoreStop>(None)).await;
    let _ = shutdown.send(());
    assert!(matches!(
        result.unwrap(),
        Err(ClientError::HttpStatus {
            status: StatusCode::TEMPORARY_REDIRECT,
            ..
        })
    ));
    assert_eq!(followed.load(Ordering::SeqCst), 0);
}
