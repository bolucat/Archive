use axum::{
    Json, Router,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
};
use clash_api::{
    Client, ConfigPatch, Host, ProviderName, ProxyName, StorageKey, UpdateConfigOptions,
    UpdateConfigRequest, UpgradeOptions,
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

async fn spawn_server(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (address.to_string(), task)
}

fn assert_auth(headers: &HeaderMap) {
    assert_eq!(headers["authorization"], "Bearer controller-secret");
}

#[tokio::test]
async fn typed_rest_methods_preserve_queries_bodies_and_empty_responses() {
    async fn update_config(
        headers: HeaderMap,
        Query(query): Query<IndexMap<String, String>>,
        Json(body): Json<serde_json::Value>,
    ) -> StatusCode {
        assert_auth(&headers);
        assert_eq!(query.get("force").map(String::as_str), Some("true"));
        assert_eq!(body, serde_json::json!({"path":"","payload":"mode: rule"}));
        StatusCode::NO_CONTENT
    }

    async fn patch_config(headers: HeaderMap, Json(body): Json<serde_json::Value>) -> StatusCode {
        assert_auth(&headers);
        assert_eq!(body, serde_json::json!({"allow-lan":true}));
        StatusCode::NO_CONTENT
    }

    async fn provider_healthcheck(headers: HeaderMap, Path(provider): Path<String>) -> StatusCode {
        assert_auth(&headers);
        assert_eq!(provider, "provider/日本");
        StatusCode::NO_CONTENT
    }

    async fn select_proxy(
        headers: HeaderMap,
        Path(group): Path<String>,
        Json(body): Json<serde_json::Value>,
    ) -> StatusCode {
        assert_auth(&headers);
        assert_eq!(group, "GLOBAL/日本");
        assert_eq!(body, serde_json::json!({"name":"DIRECT"}));
        StatusCode::NO_CONTENT
    }

    let app = Router::new()
        .route("/configs", put(update_config).patch(patch_config))
        .route(
            "/providers/proxies/{provider}/healthcheck",
            get(provider_healthcheck),
        )
        .route("/proxies/{group}/", put(select_proxy));
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .secret("controller-secret")
        .build()
        .unwrap();

    client
        .update_config(
            &UpdateConfigRequest::from_payload("mode: rule"),
            UpdateConfigOptions { force: true },
        )
        .await
        .unwrap();
    client
        .patch_config(&ConfigPatch {
            allow_lan: Some(true),
            ..ConfigPatch::default()
        })
        .await
        .unwrap();
    client
        .healthcheck_proxy_provider(&ProviderName::from("provider/日本"))
        .await
        .unwrap();
    client
        .select_proxy(&ProxyName::from("GLOBAL/日本"), &ProxyName::from("DIRECT"))
        .await
        .unwrap();
    server.abort();
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct StoredValue {
    enabled: bool,
    label: String,
}

#[tokio::test]
async fn storage_get_is_generic_and_storage_keys_are_path_segments() {
    async fn get_storage(headers: HeaderMap, Path(key): Path<String>) -> Json<StoredValue> {
        assert_auth(&headers);
        assert_eq!(key, "key/with ?#% 日本");
        Json(StoredValue {
            enabled: true,
            label: "typed".to_owned(),
        })
    }

    let app = Router::new().route("/storage/{key}", get(get_storage));
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .secret("controller-secret")
        .build()
        .unwrap();
    let key = StorageKey::new("key/with ?#% 日本").unwrap();

    let value: Option<StoredValue> = client.storage_get(&key).await.unwrap();
    assert_eq!(
        value,
        Some(StoredValue {
            enabled: true,
            label: "typed".to_owned(),
        })
    );
    server.abort();
}

#[tokio::test]
async fn externally_mutating_maintenance_calls_have_the_expected_routes_and_queries() {
    async fn restart(headers: HeaderMap) -> Json<serde_json::Value> {
        assert_auth(&headers);
        Json(serde_json::json!({"status":"ok"}))
    }

    async fn upgrade(
        headers: HeaderMap,
        Query(query): Query<IndexMap<String, String>>,
    ) -> Json<serde_json::Value> {
        assert_auth(&headers);
        assert_eq!(query.get("channel").map(String::as_str), Some("stable"));
        assert_eq!(query.get("force").map(String::as_str), Some("true"));
        Json(serde_json::json!({"status":"ok"}))
    }

    async fn no_content(headers: HeaderMap) -> StatusCode {
        assert_auth(&headers);
        StatusCode::NO_CONTENT
    }

    let app = Router::new()
        .route("/restart", axum::routing::post(restart))
        .route("/upgrade", axum::routing::post(upgrade))
        .route("/upgrade/ui", axum::routing::post(restart))
        .route("/upgrade/geo", axum::routing::post(no_content))
        .route("/configs/geo", axum::routing::post(no_content));
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .secret("controller-secret")
        .build()
        .unwrap();

    assert_eq!(client.restart().await.unwrap().status, "ok");
    assert_eq!(
        client
            .upgrade(&UpgradeOptions {
                channel: Some("stable".to_owned()),
                force: true,
            })
            .await
            .unwrap()
            .status,
        "ok"
    );
    assert_eq!(client.upgrade_ui().await.unwrap().status, "ok");
    client.upgrade_geo_databases().await.unwrap();
    client.update_geo_databases().await.unwrap();
    server.abort();
}
