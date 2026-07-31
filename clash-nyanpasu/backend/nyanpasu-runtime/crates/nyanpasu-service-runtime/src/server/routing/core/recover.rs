use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{RBuilder, core::recover::CoreRecoverRes};

use crate::server::routing::AppState;

pub async fn recover(State(state): State<AppState>) -> (StatusCode, Json<CoreRecoverRes<'static>>) {
    match state.core_manager.recover().await {
        Ok(()) => (StatusCode::OK, Json(RBuilder::success(()))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(error.into_envelope()),
        ),
    }
}
