use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{
    RBuilder,
    core::check::{CoreCheckReq, CoreCheckRes},
};

use crate::server::routing::AppState;

pub async fn check(
    State(state): State<AppState>,
    Json(payload): Json<CoreCheckReq<'_>>,
) -> (StatusCode, Json<CoreCheckRes<'static>>) {
    match state
        .core_manager
        .check(&state.runtime, &payload.core_type, &payload.config_file)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(RBuilder::success(()))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error.into_envelope()),
        ),
    }
}
