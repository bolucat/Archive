use axum::Router;
use nyanpasu_ipc::{
    api::contract::{CoreApply, CoreCheck, CoreRecover, CoreRestart, CoreStart, CoreStop},
    server::RegisterOperation,
};

use super::AppState;

pub mod apply;
pub mod check;
pub mod recover;
pub mod restart;
pub mod start;
pub mod stop;

pub fn setup() -> Router<AppState> {
    Router::new()
        .register(CoreStart, start::start)
        .register(CoreStop, stop::stop)
        .register(CoreRestart, restart::restart)
        .register(CoreApply, apply::apply)
        .register(CoreCheck, check::check)
        .register(CoreRecover, recover::recover)
}
