use async_trait::async_trait;
use hyper::Method;
use serde_json::Value;
use std::{error::Error, sync::Arc};
use tokio::sync::Mutex;

pub struct MihomoData {
    pub(crate) proxies: serde_json::Value,
    pub(crate) providers_proxies: serde_json::Value,
}

impl Default for MihomoData {
    fn default() -> Self {
        Self {
            proxies: Value::Null,
            providers_proxies: Value::Null,
        }
    }
}

pub type E = Box<dyn Error + Send + Sync>;

#[async_trait]
pub trait MihomoClient: Sized {
    async fn set_data_proxies(&self, data: Value);
    async fn set_data_providers_proxies(&self, data: Value);
    async fn get_data_proxies(&self) -> Value;
    async fn get_data_providers_proxies(&self) -> Value;
    // async fn generate_unix_path(&self, path: &str) -> Uri;
    async fn send_request(
        &self,
        path: &str,
        method: Method,
        body: Option<Value>,
    ) -> Result<Value, E>;
    async fn get_version(&self) -> Result<Value, E>;
    async fn is_mihomo_running(&self) -> Result<(), E>;
    async fn put_configs_force(&self, clash_config_path: &str) -> Result<(), E>;
    async fn patch_configs(&self, config: Value) -> Result<(), E>;
    async fn refresh_proxies(&self) -> Result<&Self, E>;
    async fn refresh_providers_proxies(&self) -> Result<&Self, E>;
    async fn get_connections(&self) -> Result<Value, E>;
    async fn delete_connection(&self, id: &str) -> Result<(), E>;
    async fn test_proxy_delay(
        &self,
        name: &str,
        test_url: Option<String>,
        timeout: i32,
    ) -> Result<Value, E>;
}

use crate::platform::Client;
pub struct MihomoManager {
    pub(super) socket_path: String,
    pub(super) client: Arc<Mutex<Client>>,
    pub(super) data: Arc<Mutex<MihomoData>>,
}
