use dotenv::dotenv;
use mihomo_api::{self, model::MihomoClient};
use std::env;

lazy_static::lazy_static! {
    static ref LOCAL_SOCK: String = {
            dotenv().ok();

            env::var("LOCAL_SOCK")
            .expect("LOCAL_SOCK must be set in .env or environment variables")
            .trim_matches('"')
            .to_string()
        };
}

#[tokio::test]
async fn test_env() {
    assert_eq!(LOCAL_SOCK.to_string(), LOCAL_SOCK.to_string());
}

#[tokio::test]
async fn test_mihomo_manager_init() {
    let manager = mihomo_api::MihomoManager::new(LOCAL_SOCK.to_string());
    let proxies = manager.get_data_proxies().await;
    let providers = manager.get_data_providers_proxies().await;
    assert_eq!(proxies, serde_json::Value::Null);
    assert_eq!(providers, serde_json::Value::Null);
}

#[tokio::test]
async fn test_get_version() {
    let manager = mihomo_api::MihomoManager::new(LOCAL_SOCK.to_string());
    let version = manager.get_version().await;
    assert!(version.is_ok());
    if let Ok(version) = version {
        assert!(!version.get("version").is_none());
    }
}

#[tokio::test]
async fn test_refresh_proxies() {
    let manager = mihomo_api::MihomoManager::new(LOCAL_SOCK.to_string());
    let manager = manager.refresh_proxies().await.unwrap();
    let proxies = manager.get_data_proxies().await;
    let providers = manager.get_data_providers_proxies().await;
    assert_ne!(proxies, serde_json::Value::Null);
    assert_eq!(providers, serde_json::Value::Null);
}

#[tokio::test]
async fn test_refresh_providers_proxies() {
    let manager = mihomo_api::MihomoManager::new(LOCAL_SOCK.to_string());
    let manager = manager.refresh_providers_proxies().await.unwrap();
    let proxies = manager.get_data_proxies().await;
    let providers = manager.get_data_providers_proxies().await;
    assert_eq!(proxies, serde_json::Value::Null);
    assert_ne!(providers, serde_json::Value::Null);
}
