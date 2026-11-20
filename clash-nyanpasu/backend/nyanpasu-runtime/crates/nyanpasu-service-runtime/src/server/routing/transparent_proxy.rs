use std::borrow::Cow;

use axum::{Json, Router, extract::State, http::StatusCode};
use nyanpasu_ipc::{
    api::{
        R, RBuilder,
        contract::{
            NetworkTransparentProxyDiagnosticsQuery, NetworkTransparentProxyReconcile,
            NetworkTransparentProxyStatusQuery,
        },
        network::transparent_proxy::{
            NetworkTransparentProxyDiagnostics, NetworkTransparentProxyRequest,
            NetworkTransparentProxyStatus,
        },
    },
    server::RegisterOperation,
};

use super::AppState;

pub fn setup() -> Router<AppState> {
    Router::new()
        .register(NetworkTransparentProxyReconcile, reconcile)
        .register(NetworkTransparentProxyStatusQuery, status)
        .register(NetworkTransparentProxyDiagnosticsQuery, diagnostics)
}

async fn reconcile(
    State(state): State<AppState>,
    Json(request): Json<NetworkTransparentProxyRequest>,
) -> (StatusCode, Json<R<'static, NetworkTransparentProxyStatus>>) {
    match state.transparent_proxy.reconcile(request).await {
        Ok(status) => (StatusCode::OK, Json(RBuilder::success(status))),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBuilder::other_error(Cow::Owned(error))),
        ),
    }
}

async fn status(
    State(state): State<AppState>,
) -> (StatusCode, Json<R<'static, NetworkTransparentProxyStatus>>) {
    (
        StatusCode::OK,
        Json(RBuilder::success(state.transparent_proxy.status().await)),
    )
}

async fn diagnostics(
    State(state): State<AppState>,
) -> (
    StatusCode,
    Json<R<'static, NetworkTransparentProxyDiagnostics>>,
) {
    (
        StatusCode::OK,
        Json(RBuilder::success(
            state.transparent_proxy.diagnostics().await,
        )),
    )
}
