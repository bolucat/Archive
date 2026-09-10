use axum::Router;
use nyanpasu_ipc::{
    api::contract::{CoreCheck, CoreStart, CoreStop, CoreV2Operation, CoreV2Status, CoreV2Submit},
    server::RegisterOperation,
};

use super::AppState;

pub mod check;
pub mod start;
pub mod stop;
pub mod v2;

pub fn setup() -> Router<AppState> {
    Router::new()
        .register(CoreStart, start::start)
        .register(CoreStop, stop::stop)
        .register(CoreCheck, check::check)
        .register(CoreV2Submit, v2::submit)
        .register(CoreV2Operation, v2::operation)
        .register(CoreV2Status, v2::status)
        .register(
            nyanpasu_ipc::api::contract::CoreV2EffectiveConfig,
            v2::effective_config,
        )
        .register(
            nyanpasu_ipc::api::contract::CoreV2ApiConnection,
            v2::api_connection,
        )
}
