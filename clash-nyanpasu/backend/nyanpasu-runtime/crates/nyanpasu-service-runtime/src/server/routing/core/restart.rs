use std::borrow::Cow;

use axum::{Json, extract::State, http::StatusCode};
use nyanpasu_ipc::api::{RBuilder, core::restart::CoreRestartRes};

use crate::server::routing::AppState;

pub async fn restart(State(state): State<AppState>) -> (StatusCode, Json<CoreRestartRes<'static>>) {
    let res = state.core_manager.restart().await;
    match res {
        Ok(_) => (StatusCode::OK, Json(RBuilder::success(()))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBuilder::other_error(Cow::Owned(e.to_string()))),
        ),
    }
}
