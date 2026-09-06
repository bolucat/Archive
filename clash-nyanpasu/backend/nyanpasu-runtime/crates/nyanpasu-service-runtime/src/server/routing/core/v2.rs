//! The v2 control-plane routes: thin adapters over the bridge's submit /
//! operation / status methods. Additive next to the v1 routes.

use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{
    R, RBuilder,
    core::v2::{CoreOperationReq, CoreOperationRes, CoreSubmitReq, CoreSubmitRes},
    status::CoreInfos,
};

use crate::server::routing::AppState;

pub async fn submit(
    State(state): State<AppState>,
    Json(payload): Json<CoreSubmitReq<'_>>,
) -> (StatusCode, Json<CoreSubmitRes<'static>>) {
    match state.core_manager.submit_v2(&state.runtime, &payload) {
        Ok(info) => (StatusCode::OK, Json(RBuilder::success(info))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error.into_envelope()),
        ),
    }
}

pub async fn operation(
    State(state): State<AppState>,
    Json(payload): Json<CoreOperationReq<'_>>,
) -> (StatusCode, Json<CoreOperationRes<'static>>) {
    match state.core_manager.operation_v2(&payload).await {
        Ok(info) => (StatusCode::OK, Json(RBuilder::success(info))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error.into_envelope()),
        ),
    }
}

pub async fn status(State(state): State<AppState>) -> (StatusCode, Json<R<'static, CoreInfos>>) {
    let infos = state.core_manager.status().await;
    (StatusCode::OK, Json(RBuilder::success(infos)))
}

/// Credentials are fetched explicitly, never broadcast with status snapshots.
pub async fn api_connection(
    State(state): State<AppState>,
) -> (
    StatusCode,
    Json<R<'static, Option<nyanpasu_ipc::api::core::v2::CoreApiConnection>>>,
) {
    let connection = state.core_manager.api_connection().await;
    (StatusCode::OK, Json(RBuilder::success(connection)))
}
