use std::borrow::Cow;

use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{RBuilder, core::stop::CoreStopRes};

use crate::server::routing::AppState;

pub async fn stop(State(state): State<AppState>) -> (StatusCode, Json<CoreStopRes<'static>>) {
    let res = state.core_manager.stop().await;
    match res {
        Ok(_) => (StatusCode::OK, Json(RBuilder::success(()))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBuilder::other_error(Cow::Owned(e.to_string()))),
        ),
    }
}
