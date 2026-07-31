use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{
    RBuilder,
    core::apply::{CoreApplyReq, CoreApplyRes},
};

use crate::server::routing::AppState;

pub async fn apply(
    State(state): State<AppState>,
    Json(payload): Json<CoreApplyReq<'_>>,
) -> (StatusCode, Json<CoreApplyRes<'static>>) {
    match state
        .core_manager
        .apply(
            &state.runtime,
            &payload.core_type,
            &payload.config_file,
            payload.expected_revision.as_ref(),
        )
        .await
    {
        Ok(data) => (StatusCode::OK, Json(RBuilder::success(data))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error.into_envelope()),
        ),
    }
}
