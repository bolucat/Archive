use std::{borrow::Cow, sync::Arc};

use axum::{
    body::{Body, to_bytes},
    http::{
        Method, Request, StatusCode,
        header::{
            CONNECTION, CONTENT_TYPE, HeaderName, SEC_WEBSOCKET_KEY, SEC_WEBSOCKET_VERSION, UPGRADE,
        },
    },
    response::Response,
};
use camino::Utf8PathBuf;
use nyanpasu_core_manager::LocalIpcPolicy;
use nyanpasu_ipc::api::{
    ResponseCode,
    contract::{
        CoreApply, CoreCheck, CoreRecover, CoreRestart, CoreStart, CoreStop, IpcOperation,
        LogsInspect, LogsRetrieve, NetworkSetDns, Status as StatusOp,
    },
    core::{
        apply::{CoreApplyReq, CoreApplyRes},
        check::{CoreCheckReq, CoreCheckRes},
        recover::CoreRecoverRes,
        stop::{CORE_STOP_ENDPOINT, CoreStopRes},
    },
    status::{CoreState, CoreStateDetail, STATUS_ENDPOINT, StatusRes},
    ws::events::EVENT_URI,
};
use nyanpasu_utils::core::{ClashCoreType, CoreType};
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use tower::ServiceExt;

use super::{AppState, create_router};
use crate::server::{CoreManager, EventHub, Logger, consts::RuntimeInfos};

struct TestEnv {
    state: AppState,
    _dir: TempDir,
}

impl TestEnv {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let runtime_dir =
            Utf8PathBuf::from_path_buf(root.join("core-runtime")).expect("temp path is UTF-8");
        let data_dir =
            Utf8PathBuf::from_path_buf(root.join("nyanpasu-data")).expect("temp path is UTF-8");
        let core_manager = CoreManager::new(runtime_dir, LocalIpcPolicy::Disable, data_dir)
            .await
            .unwrap();
        let runtime = Arc::new(RuntimeInfos {
            service_data_dir: root.join("service-data"),
            service_config_dir: root.join("service-config"),
            nyanpasu_config_dir: root.join("nyanpasu-config"),
            nyanpasu_data_dir: root.join("nyanpasu-data"),
            nyanpasu_app_dir: root.join("nyanpasu-app"),
        });
        let state = AppState {
            core_manager,
            hub: EventHub::new(),
            runtime,
            logger: Logger::new(),
        };
        Self { state, _dir: dir }
    }
}

async fn body_of<T: DeserializeOwned>(response: Response) -> T {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn status_reports_a_stopped_core_and_echoes_the_injected_runtime_dirs() {
    let env = TestEnv::new().await;
    let runtime = env.state.runtime.clone();
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let envelope: StatusRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::Ok);
    assert_eq!(envelope.msg, ResponseCode::Ok.msg());
    let body = envelope.data.unwrap();
    assert_eq!(body.version, crate::consts::APP_VERSION);
    assert!(matches!(body.core_infos.state, CoreState::Stopped(None)));
    assert!(body.core_infos.r#type.is_none());
    assert!(body.core_infos.config_path.is_none());
    assert_eq!(
        body.runtime_infos.service_data_dir.as_ref(),
        &runtime.service_data_dir
    );
    assert_eq!(
        body.runtime_infos.service_config_dir.as_ref(),
        &runtime.service_config_dir
    );
    assert_eq!(
        body.runtime_infos.nyanpasu_config_dir.as_ref(),
        &runtime.nyanpasu_config_dir
    );
    assert_eq!(
        body.runtime_infos.nyanpasu_data_dir.as_ref(),
        &runtime.nyanpasu_data_dir
    );
}

#[tokio::test]
async fn stopping_an_idle_core_keeps_the_legacy_error_envelope() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(CORE_STOP_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreStopRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.msg, "core is already stopped");
    assert!(envelope.data.is_none());
}

#[tokio::test]
async fn restart_before_any_start_reports_the_legacy_error() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(CoreRestart::PATH)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreStopRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.msg, "core have not been started yet");
    assert!(envelope.data.is_none());
}

#[tokio::test]
async fn two_states_are_independent() {
    let first = TestEnv::new().await;
    let second = TestEnv::new().await;

    assert_ne!(
        first.state.runtime.service_data_dir,
        second.state.runtime.service_data_dir
    );

    let first_response = create_router(first.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second_response = create_router(second.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(first_response.status(), StatusCode::OK);
    assert_eq!(second_response.status(), StatusCode::OK);
    let first_envelope: StatusRes<'static> = body_of(first_response).await;
    let second_envelope: StatusRes<'static> = body_of(second_response).await;
    let first_body = first_envelope.data.unwrap();
    let second_body = second_envelope.data.unwrap();
    assert_eq!(
        first_body.runtime_infos.service_data_dir.as_ref(),
        &first.state.runtime.service_data_dir
    );
    assert_eq!(
        second_body.runtime_infos.service_data_dir.as_ref(),
        &second.state.runtime.service_data_dir
    );
}

/// Ask the router for `Op`'s address and report only whether it is mounted.
/// A body-less POST to `/core/start` is answered 4xx by the extractor, which
/// still proves the route exists — 404/405 are the only failures here.
async fn probe(state: AppState, method: Method, path: &str) -> StatusCode {
    create_router(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn every_operation_is_mounted_where_its_contract_says() {
    let env = TestEnv::new().await;
    let addresses = [
        (StatusOp::METHOD, StatusOp::PATH),
        (CoreStart::METHOD, CoreStart::PATH),
        (CoreStop::METHOD, CoreStop::PATH),
        (CoreRestart::METHOD, CoreRestart::PATH),
        (CoreApply::METHOD, CoreApply::PATH),
        (CoreCheck::METHOD, CoreCheck::PATH),
        (CoreRecover::METHOD, CoreRecover::PATH),
        (LogsRetrieve::METHOD, LogsRetrieve::PATH),
        (LogsInspect::METHOD, LogsInspect::PATH),
        (NetworkSetDns::METHOD, NetworkSetDns::PATH),
    ];
    for (method, path) in addresses {
        let status = probe(env.state.clone(), method, path).await;
        assert_ne!(status, StatusCode::NOT_FOUND, "{path} is not mounted");
        assert_ne!(
            status,
            StatusCode::METHOD_NOT_ALLOWED,
            "{path} is mounted with the wrong method"
        );
    }
}

#[tokio::test]
async fn an_unknown_path_answers_with_the_envelope() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri("/does/not/exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let envelope: CoreStopRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.msg, "not found");
}

#[tokio::test]
async fn a_wrong_method_answers_with_the_envelope() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let envelope: CoreStopRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.msg, "method not allowed");
}

#[tokio::test]
async fn responses_carry_a_request_id() {
    let env = TestEnv::new().await;
    let header = HeaderName::from_static("x-request-id");

    let generated = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(generated.headers().contains_key(&header));

    let echoed = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .header(&header, "caller-supplied")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(echoed.headers().get(&header).unwrap(), "caller-supplied");
}

#[tokio::test]
async fn status_projects_the_new_fields_from_the_manager_snapshot() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let envelope: StatusRes<'static> = body_of(response).await;
    let body = envelope.data.unwrap();
    // A never-started core: the manager has no instance to report an
    // endpoint, health observation or revision for.
    assert!(body.core_infos.controller.is_none());
    assert!(body.core_infos.health.is_none());
    assert!(body.core_infos.revision.is_none());
    // `detail` is populated even when the others are not, and unlike the
    // two-valued `state` it names the stop reason slot explicitly.
    assert_eq!(
        body.core_infos.detail,
        Some(CoreStateDetail::Stopped { reason: None })
    );
}

/// A JSON POST through the production router.
async fn post_json<T: serde::Serialize>(state: AppState, path: &str, payload: &T) -> Response {
    create_router(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(path)
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

/// `apply` refuses a stopped core instead of starting one, and the refusal
/// carries both the legacy string and the new machine-readable kind.
///
/// The manager rejects a stopped core before it reads either file
/// (`manager/apply.rs:25-26`), but the bridge resolves both first, so both have
/// to exist — the "binary" only has to be findable, never runnable.
#[tokio::test]
async fn applying_to_a_stopped_core_reports_not_started_with_its_kind() {
    let env = TestEnv::new().await;
    let core_type = CoreType::Clash(ClashCoreType::Mihomo);
    let data_dir = &env.state.runtime.nyanpasu_data_dir;
    std::fs::create_dir_all(data_dir).unwrap();
    std::fs::write(data_dir.join(core_type.get_executable_name()), b"").unwrap();
    let config = data_dir.join("config.yaml");
    std::fs::write(&config, b"mixed-port: 7890\n").unwrap();

    let response = post_json(
        env.state.clone(),
        CoreApply::PATH,
        &CoreApplyReq {
            core_type: Cow::Borrowed(&core_type),
            config_file: Cow::Borrowed(&config),
            expected_revision: None,
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreApplyRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.msg, "core have not been started yet");
    assert_eq!(envelope.error_kind.as_deref(), Some("not_started"));
    assert!(envelope.data.is_none());
}

/// An unresolvable config path is answered in the envelope, not by a panic the
/// catch layer has to convert — and with the kind that says which of the two
/// paths in the request was the bad one.
#[tokio::test]
async fn checking_an_unresolvable_config_answers_in_the_envelope() {
    let env = TestEnv::new().await;
    let core_type = CoreType::Clash(ClashCoreType::Mihomo);
    let missing = env.state.runtime.nyanpasu_data_dir.join("nope.yaml");

    let response = post_json(
        env.state.clone(),
        CoreCheck::PATH,
        &CoreCheckReq {
            core_type: Cow::Borrowed(&core_type),
            config_file: Cow::Borrowed(&missing),
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreCheckRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.error_kind.as_deref(), Some("config_not_found"));
    assert!(
        envelope.msg.contains("nope.yaml"),
        "the failure must name the path: {}",
        envelope.msg
    );
}

/// The mirror image of the fixture above: the config resolves and the binary
/// does not. Both failures reach the client through the same envelope, and the
/// kind is what tells them apart without parsing the message.
#[tokio::test]
async fn applying_without_a_core_binary_reports_binary_not_found() {
    let env = TestEnv::new().await;
    let core_type = CoreType::Clash(ClashCoreType::Mihomo);
    let data_dir = &env.state.runtime.nyanpasu_data_dir;
    std::fs::create_dir_all(data_dir).unwrap();
    let config = data_dir.join("config.yaml");
    std::fs::write(&config, b"mixed-port: 7890\n").unwrap();

    let response = post_json(
        env.state.clone(),
        CoreApply::PATH,
        &CoreApplyReq {
            core_type: Cow::Borrowed(&core_type),
            config_file: Cow::Borrowed(&config),
            expected_revision: None,
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreApplyRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.error_kind.as_deref(), Some("binary_not_found"));
    assert!(
        envelope.msg.contains(core_type.get_executable_name()),
        "the failure must name the binary: {}",
        envelope.msg
    );
}

/// Recovery is idempotent: with nothing quarantined it succeeds, which is what
/// makes it safe for a client to call before retrying an operation.
#[tokio::test]
async fn recovering_without_a_quarantine_succeeds() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(CoreRecover::PATH)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let envelope: CoreRecoverRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::Ok);
    assert!(envelope.error_kind.is_none());
    assert!(envelope.data.is_none());
}

/// The query string is not protocol: `/ws/events` takes no parameters and must
/// ignore whatever it is handed — including the duplicated key that a `Query`
/// extractor would reject with 400, which is the regression this pins.
///
/// 426 is as far as it can get here: `tower::oneshot` hands the router no hyper
/// upgrade state, so `WebSocketUpgrade` rejects with `ConnectionNotUpgradable`
/// after every header check has passed. That is what makes it a useful
/// assertion — a 400 would mean the query string was rejected, a 404/405 that
/// the route moved.
#[tokio::test]
async fn the_event_stream_ignores_any_query() {
    let env = TestEnv::new().await;
    for uri in [EVENT_URI, "/ws/events?v=1&v=2"] {
        let response = create_router(env.state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(uri)
                    .header(CONNECTION, "upgrade")
                    .header(UPGRADE, "websocket")
                    .header(SEC_WEBSOCKET_VERSION, "13")
                    .header(SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED, "{uri}");
    }
}

/// `/status` is where a caller learns where the logs are, now that the service
/// does not stream them. The core directory comes from the manager, so this
/// pins the forwarder as well as the field.
#[tokio::test]
async fn the_status_response_reports_the_log_directories() {
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri(STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body: StatusRes<'static> = body_of(response).await;
    let logs = body
        .data
        .expect("status returns a body")
        .logs
        .expect("status must report the log directories");
    assert_eq!(logs.service_dir, crate::utils::dirs::service_logs_dir());
    let core_dir = logs.core_dir.expect("the sink is on by default");
    assert!(
        core_dir.ends_with("logs"),
        "core log dir should be the manager's archive: {core_dir:?}"
    );
}

/// The full v2 submit -> long-poll path over the wire, on a stopped manager:
/// admission succeeds, the transaction runs to its own classified failure,
/// and the caller reads it back from the registry.
#[tokio::test]
async fn v2_submit_stop_runs_to_a_classified_terminal_failure() {
    use nyanpasu_ipc::api::core::v2::{
        CORE_V2_OPERATION_ENDPOINT, CORE_V2_SUBMIT_ENDPOINT, CoreCommandInfo, CoreOperationReq,
        CoreOperationRes, CoreSubmitReq, CoreSubmitRes, OperationPhase,
    };
    let env = TestEnv::new().await;
    let id = "0011223344556677-8899aabb-ccddeeff";
    let response = post_json(
        env.state.clone(),
        CORE_V2_SUBMIT_ENDPOINT,
        &CoreSubmitReq {
            operation_id: Cow::Borrowed(id),
            command: CoreCommandInfo::Stop,
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let envelope: CoreSubmitRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::Ok);
    let info = envelope
        .data
        .expect("submit returns the admission snapshot");
    assert_eq!(info.id, id);

    let response = post_json(
        env.state.clone(),
        CORE_V2_OPERATION_ENDPOINT,
        &CoreOperationReq {
            operation_id: Cow::Borrowed(id),
            wait_ms: Some(5_000),
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let envelope: CoreOperationRes<'static> = body_of(response).await;
    let info = envelope.data.expect("query returns the operation");
    assert_eq!(info.phase, OperationPhase::Failed);
    let error = info.error.expect("a failed operation carries its error");
    assert_eq!(error.kind.as_deref(), Some("not_started"));
    assert!(!error.retryable);
}

/// Same id, different payload: the idempotency registry refuses at admission,
/// through the wire, with the pinned kind.
#[tokio::test]
async fn v2_submit_reuses_of_an_id_with_a_different_payload_conflict() {
    use nyanpasu_ipc::api::core::v2::{
        CORE_V2_SUBMIT_ENDPOINT, CoreCommandInfo, CoreSubmitReq, CoreSubmitRes,
    };
    let env = TestEnv::new().await;
    let id = "0011223344556677-8899aabb-ccddeeff";
    let response = post_json(
        env.state.clone(),
        CORE_V2_SUBMIT_ENDPOINT,
        &CoreSubmitReq {
            operation_id: Cow::Borrowed(id),
            command: CoreCommandInfo::Stop,
        },
    )
    .await;
    let first: CoreSubmitRes<'static> = body_of(response).await;
    assert_eq!(first.code, ResponseCode::Ok);

    let response = post_json(
        env.state.clone(),
        CORE_V2_SUBMIT_ENDPOINT,
        &CoreSubmitReq {
            operation_id: Cow::Borrowed(id),
            command: CoreCommandInfo::Recover,
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let second: CoreSubmitRes<'static> = body_of(response).await;
    assert_eq!(second.code, ResponseCode::OtherError);
    assert_eq!(second.error_kind.as_deref(), Some("operation_conflict"));
    // Retrying a conflict re-submits the same losing payload, so the answer is
    // an explicit `false` rather than an absent field the caller has to guess at.
    assert_eq!(second.retryable, Some(false));
}

/// A malformed id and an unknown id both come back as error envelopes, and
/// the unknown one points the caller at the recovery contract (status + CAS).
#[tokio::test]
async fn v2_operation_rejects_malformed_and_unknown_ids() {
    use nyanpasu_ipc::api::core::v2::{
        CORE_V2_OPERATION_ENDPOINT, CoreOperationReq, CoreOperationRes,
    };
    let env = TestEnv::new().await;
    let response = post_json(
        env.state.clone(),
        CORE_V2_OPERATION_ENDPOINT,
        &CoreOperationReq {
            operation_id: Cow::Borrowed("not-hex"),
            wait_ms: None,
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreOperationRes<'static> = body_of(response).await;
    assert!(envelope.msg.contains("malformed operation id"));

    let response = post_json(
        env.state.clone(),
        CORE_V2_OPERATION_ENDPOINT,
        &CoreOperationReq {
            operation_id: Cow::Borrowed("ffeeddccbbaa9988-77665544-33221100"),
            wait_ms: None,
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreOperationRes<'static> = body_of(response).await;
    assert!(envelope.msg.contains("unknown operation"));
}

/// `/v2/core/status` serves the canonical projection without touching the
/// runtime.
#[tokio::test]
async fn v2_status_serves_the_canonical_projection() {
    use nyanpasu_ipc::api::core::v2::CORE_V2_STATUS_ENDPOINT;
    let env = TestEnv::new().await;
    let response = create_router(env.state.clone())
        .oneshot(
            Request::builder()
                .uri(CORE_V2_STATUS_ENDPOINT)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let envelope: nyanpasu_ipc::api::R<'static, nyanpasu_ipc::api::status::CoreInfos> =
        body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::Ok);
    let infos = envelope.data.expect("status always has a body");
    assert!(matches!(infos.state, CoreState::Stopped(_)));
    assert!(infos.r#type.is_none(), "no core was ever requested");
}

/// Daemon shutdown used to call the manager directly, which left the v2
/// control plane wide open: a submit that landed a moment later was admitted
/// and ran a core transaction after the daemon had already stopped its core.
#[tokio::test]
async fn shutdown_closes_the_v2_control_plane_to_new_work() {
    use nyanpasu_ipc::api::core::v2::{
        CORE_V2_SUBMIT_ENDPOINT, CoreCommandInfo, CoreSubmitReq, CoreSubmitRes,
    };
    let env = TestEnv::new().await;
    env.state.core_manager.shutdown().await;

    let response = post_json(
        env.state.clone(),
        CORE_V2_SUBMIT_ENDPOINT,
        &CoreSubmitReq {
            operation_id: Cow::Borrowed("0011223344556677-8899aabb-ccddeeff"),
            command: CoreCommandInfo::Stop,
        },
    )
    .await;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let envelope: CoreSubmitRes<'static> = body_of(response).await;
    assert_eq!(envelope.code, ResponseCode::OtherError);
    assert_eq!(envelope.error_kind.as_deref(), Some("shutting_down"));
    assert_eq!(envelope.retryable, Some(false));
}
