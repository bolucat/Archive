use std::borrow::Cow;

use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{
    RBuilder,
    core::start::{CoreStartReq, CoreStartRes},
};

use crate::server::routing::AppState;

pub async fn start(
    State(state): State<AppState>,
    Json(payload): Json<CoreStartReq<'_>>,
) -> (StatusCode, Json<CoreStartRes<'static>>) {
    let res = state
        .core_manager
        .start(
            &state.runtime,
            &payload.core_type,
            camino::Utf8Path::from_path(&payload.config_file)
                .expect("failed to convert config_file to Utf8Path"),
        )
        .await;

    match res {
        Ok(_) => (StatusCode::OK, Json(RBuilder::success(()))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBuilder::other_error(Cow::Owned(e.to_string()))),
        ),
    }
}
