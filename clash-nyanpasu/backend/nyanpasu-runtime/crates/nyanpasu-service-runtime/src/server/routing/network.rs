use axum::{Json, Router, http::StatusCode};
use nyanpasu_ipc::{
    api::{
        RBuilder,
        contract::NetworkSetDns,
        network::set_dns::{NetworkSetDnsReq, NetworkSetDnsRes},
    },
    server::RegisterOperation,
};
use std::borrow::Cow;

#[cfg(target_os = "macos")]
use nyanpasu_utils::network::macos::{get_default_network_hardware_port, set_dns};

pub fn setup<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new().register(NetworkSetDns, network)
}

#[cfg(target_os = "macos")]
pub async fn network(
    Json(mut req): Json<NetworkSetDnsReq<'static>>,
) -> (StatusCode, Json<NetworkSetDnsRes<'static>>) {
    let default_interface = match get_default_network_hardware_port() {
        Ok(interface) => interface,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(RBuilder::other_error(Cow::Owned(e.to_string()))),
            );
        }
    };
    let dns_servers = req
        .dns_servers
        .take()
        .map(|v| v.into_iter().map(|v| v.into_owned()).collect::<Vec<_>>());
    match set_dns(&default_interface, dns_servers) {
        Ok(_) => (StatusCode::OK, Json(RBuilder::success(()))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(RBuilder::other_error(Cow::Owned(e.to_string()))),
        ),
    }
}

#[cfg(not(target_os = "macos"))]
pub async fn network(
    Json(_req): Json<NetworkSetDnsReq<'static>>,
) -> (StatusCode, Json<NetworkSetDnsRes<'static>>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(RBuilder::other_error(Cow::Borrowed("Not implemented"))),
    )
}
