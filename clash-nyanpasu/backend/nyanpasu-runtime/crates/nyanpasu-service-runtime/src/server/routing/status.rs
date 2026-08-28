use std::borrow::Cow;

use axum::{Json, Router, extract::State, http::StatusCode};

use nyanpasu_ipc::{
    api::{
        RBuilder,
        contract::Status as StatusOp,
        status::{LogPathsInfo, RuntimeInfos, StatusRes, StatusResBody},
    },
    server::RegisterOperation,
};

use super::AppState;

pub fn setup() -> Router<AppState> {
    Router::new().register(StatusOp, status)
}

pub async fn status(State(state): State<AppState>) -> (StatusCode, Json<StatusRes<'static>>) {
    let status = state.core_manager.status().await;
    let res = RBuilder::success(StatusResBody {
        version: Cow::Borrowed(crate::consts::APP_VERSION),
        core_infos: status,
        runtime_infos: RuntimeInfos {
            service_data_dir: Cow::Owned(state.runtime.service_data_dir.clone()),
            service_config_dir: Cow::Owned(state.runtime.service_config_dir.clone()),
            nyanpasu_config_dir: Cow::Owned(state.runtime.nyanpasu_config_dir.clone()),
            nyanpasu_data_dir: Cow::Owned(state.runtime.nyanpasu_data_dir.clone()),
        },
        // Always sent. The field is optional on the wire only so a payload
        // without it stays byte-identical to the pre-L3 format.
        logs: Some(LogPathsInfo {
            service_dir: crate::utils::dirs::service_logs_dir(),
            core_dir: state.core_manager.core_log_dir(),
        }),
    });

    (StatusCode::OK, Json(res))
}
