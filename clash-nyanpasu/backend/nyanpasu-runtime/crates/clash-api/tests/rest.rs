use axum::{
    Json, Router,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
};
use clash_api::{
    Client, ConfigPatch, Host, ProviderName, ProxyName, ProxySelection, StorageKey,
    UpdateConfigOptions, UpdateConfigRequest, UpgradeOptions,
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
        .select_proxy(ProxySelection {
            group: &ProxyName::from("GLOBAL/日本"),
            target: &ProxyName::from("DIRECT"),
        })
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

#[tokio::test]
async fn rules_accept_absent_extensions_but_reject_malformed_fields() {
    for (body, valid) in [
        (
            serde_json::json!({"type":"Domain","payload":"example.com","proxy":"DIRECT"}),
            true,
        ),
        (
            serde_json::json!({"type":"Domain","payload":"example.com","proxy":"DIRECT","index":0,"size":0}),
            true,
        ),
        (
            serde_json::json!({"type":"Domain","payload":"example.com","proxy":"DIRECT","index":"bad"}),
            false,
        ),
        (
            serde_json::json!({"type":"Domain","payload":"example.com"}),
            false,
        ),
    ] {
        let expected_index = body.get("index").and_then(serde_json::Value::as_i64);
        let expected_size = body.get("size").and_then(serde_json::Value::as_i64);
        let app = Router::new().route(
            "/rules/",
            get(move || async move { Json(serde_json::json!({"rules":[body]})) }),
        );
        let (address, server) = spawn_server(app).await;
        let client = Client::builder(Host::http(address).unwrap())
            .build()
            .unwrap();
        let result = client.rules().await;
        assert_eq!(result.is_ok(), valid);
        if valid {
            let rules = result.unwrap();
            assert_eq!(rules[0].index, expected_index);
            assert_eq!(rules[0].size, expected_size);
            assert_eq!(rules[0].proxy, "DIRECT");
        }
        server.abort();
    }
}

#[tokio::test]
async fn rule_providers_preserve_missing_metadata_unknown_strings_and_order() {
    let app = Router::new().route("/providers/rules/", get(|| async {
        axum::response::Response::builder().header("content-type", "application/json")
            .body(axum::body::Body::from(r#"{"providers":{"z":{"name":"z"},"a":{"name":"a","behavior":"FutureBehavior","format":"FutureFormat","type":"FutureType","vehicleType":"FutureVehicle","ruleCount":0,"updatedAt":"2026-09-07T10:00:00+08:00"},"known":{"name":"known","behavior":"IPCIDR","format":"MrsRule","type":"Rule","vehicleType":"HTTP"}}}"#)).unwrap()
    }));
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .build()
        .unwrap();
    let providers = client.rule_providers().await.unwrap();
    assert_eq!(
        providers
            .keys()
            .map(|name| name.as_str())
            .collect::<Vec<_>>(),
        ["z", "a", "known"]
    );
    let minimal = &providers[&clash_api::RuleProviderName::from("z")];
    assert_eq!(
        serde_json::to_value(minimal).unwrap(),
        serde_json::json!({"name":"z","payload":null})
    );
    let future = &providers[&clash_api::RuleProviderName::from("a")];
    assert_eq!(future.behavior.as_ref().unwrap().as_str(), "FutureBehavior");
    assert_eq!(future.format.as_ref().unwrap().as_str(), "FutureFormat");
    assert_eq!(
        future.provider_type.as_ref().unwrap().as_str(),
        "FutureType"
    );
    assert_eq!(
        future.vehicle_type.as_ref().unwrap().as_str(),
        "FutureVehicle"
    );
    let wire = serde_json::to_value(future).unwrap();
    assert_eq!(wire["behavior"], "FutureBehavior");
    assert_eq!(wire["format"], "FutureFormat");
    assert_eq!(wire["type"], "FutureType");
    assert_eq!(wire["vehicleType"], "FutureVehicle");
    let known = &providers[&clash_api::RuleProviderName::from("known")];
    assert_eq!(
        known.behavior,
        Some(clash_api::RuleProviderBehavior::IpCidr)
    );
    assert_eq!(known.format, Some(clash_api::RuleFormat::MrsRule));
    assert_eq!(known.provider_type, Some(clash_api::ProviderType::Rule));
    assert_eq!(known.vehicle_type, Some(clash_api::VehicleType::Http));
    server.abort();
}

#[tokio::test]
async fn config_reads_preserve_absence_and_unknown_response_enums() {
    let (address, server) = spawn_server(Router::new().route("/configs", get(|| async {
        Json(serde_json::json!({"mode":"future-mode","log-level":"trace","mixed-port":0,"allow-lan":false}))
    }))).await;
    let client = Client::builder(Host::http(address).unwrap())
        .build()
        .unwrap();
    let config = client.configs().await.unwrap();
    assert_eq!(config.mode.as_ref().unwrap().as_str(), "future-mode");
    assert_eq!(config.log_level.as_ref().unwrap().as_str(), "trace");
    assert_eq!(config.port, None);
    assert_eq!(config.mixed_port, Some(0));
    assert_eq!(config.allow_lan, Some(false));
    assert_eq!(
        serde_json::to_value(config).unwrap(),
        serde_json::json!({
            "mode":"future-mode","log-level":"trace","mixed-port":0,"allow-lan":false
        })
    );
    server.abort();
}

#[test]
fn config_response_validation_does_not_widen_writes_or_subscriptions() {
    assert!(serde_json::from_str::<clash_api::ConfigPatch>(r#"{"mode":"future-mode"}"#).is_err());
    assert!(serde_json::from_str::<clash_api::LogLevel>(r#""trace""#).is_err());
    for value in [
        serde_json::json!({"mode":42}),
        serde_json::json!({"mode":{"rule":null}}),
        serde_json::json!({"log-level":{"info":null}}),
        serde_json::json!({"log-level":false}),
        serde_json::json!({"mixed-port":"zero"}),
        serde_json::json!({"ipv6":"false"}),
    ] {
        assert!(serde_json::from_value::<clash_api::RuntimeConfig>(value).is_err());
    }
    let known: clash_api::RuntimeConfig =
        serde_json::from_value(serde_json::json!({"mode":"rule","log-level":"info"})).unwrap();
    assert_eq!(
        known.mode,
        Some(clash_api::ConfigEnum::Known(clash_api::TunnelMode::Rule))
    );
    assert_eq!(
        known.log_level,
        Some(clash_api::ConfigEnum::Known(clash_api::LogLevel::Info))
    );
}

#[tokio::test]
async fn proxy_lists_accept_base_nodes_and_partial_subscription_usage() {
    let app = Router::new()
        .route("/proxies/", get(|| async { Json(serde_json::json!({"proxies":{
            "node":{"name":"node","type":"Vless","udp":true,"history":[{"time":"2026-09-07T00:00:00Z","delay":-1}]}
        }})) }))
        .route("/providers/proxies/", get(|| async { Json(serde_json::json!({"providers":{
            "p":{"name":"p","type":"FutureProxy","vehicleType":"FutureTransport","proxies":[],"subscriptionInfo":{"Expire":123,"upload":5}}
        }})) }));
    let (address, server) = spawn_server(app).await;
    let client = Client::builder(Host::http(address).unwrap())
        .build()
        .unwrap();
    let proxies = client.proxies().await.unwrap();
    let proxy = &proxies[&ProxyName::from("node")];
    assert_eq!(proxy.alive, None);
    assert_eq!(proxy.xudp, None);
    assert_eq!(proxy.history[0].delay, -1);
    let providers = client.proxy_providers().await.unwrap();
    let provider = &providers[&ProviderName::from("p")];
    assert_eq!(provider.test_url, None);
    assert_eq!(provider.expected_status, None);
    assert_eq!(provider.provider_type.as_str(), "FutureProxy");
    assert_eq!(provider.vehicle_type.as_str(), "FutureTransport");
    let usage = provider.subscription_info.as_ref().unwrap();
    assert_eq!(
        (usage.upload, usage.download, usage.total, usage.expire),
        (5, 0, 0, 123)
    );
    server.abort();
}

#[test]
fn proxy_metadata_still_rejects_malformed_present_values() {
    for field in ["alive", "xudp", "tfo"] {
        let mut body = serde_json::json!({"name":"p","type":"Direct","udp":true,"history":[]});
        body[field] = serde_json::json!("yes");
        assert!(serde_json::from_value::<clash_api::Proxy>(body).is_err());
    }
    assert!(
        serde_json::from_value::<clash_api::Proxy>(
            serde_json::json!({"name":"p","type":"Direct","history":[]})
        )
        .is_err()
    );
}
