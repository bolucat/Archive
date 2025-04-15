use crate::{model::E, platform::Client};
use async_trait::async_trait;
use hyper::Method;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{
    MihomoData,
    model::{MihomoClient, MihomoManager},
};

impl MihomoManager {
    pub fn new(socket_path: String) -> Self {
        let client = Client::new();
        Self {
            socket_path,
            client: Arc::new(Mutex::new(client)),
            data: Arc::new(Mutex::new(MihomoData::default())),
        }
    }
}

#[async_trait]
impl MihomoClient for MihomoManager {
    async fn set_data_proxies(&self, data: Value) {
        self.data.lock().await.proxies = data;
    }

    async fn set_data_providers_proxies(&self, data: Value) {
        self.data.lock().await.providers_proxies = data;
    }

    async fn get_data_proxies(&self) -> Value {
        self.data.lock().await.proxies.clone()
    }

    async fn get_data_providers_proxies(&self) -> Value {
        self.data.lock().await.providers_proxies.clone()
    }

    async fn send_request(
        &self,
        path: &str,
        method: Method,
        body: Option<Value>,
    ) -> Result<Value, E> {
        let client = self.client.lock().await;
        client
            .send_request(self.socket_path.clone(), path, method, body)
            .await
    }

    async fn get_version(&self) -> Result<Value, E> {
        let data = self.send_request("/version", Method::GET, None).await?;
        Ok(data)
    }

    async fn is_mihomo_running(&self) -> Result<(), E> {
        self.get_version().await?;
        Ok(())
    }

    async fn put_configs_force(&self, clash_config_path: &str) -> Result<(), E> {
        let body = serde_json::json!({
            "path": clash_config_path
        });
        let _ = self
            .send_request("/configs?force=true", Method::PUT, Some(body))
            .await?;
        Ok(())
    }

    async fn patch_configs(&self, config: Value) -> Result<(), E> {
        let _ = self
            .send_request("/configs", Method::PATCH, Some(config))
            .await?;
        Ok(())
    }

    async fn refresh_proxies(&self) -> Result<&Self, E> {
        let data = self.send_request("/proxies", Method::GET, None).await?;
        self.set_data_proxies(data).await;
        Ok(self)
    }

    async fn refresh_providers_proxies(&self) -> Result<&Self, E> {
        let data = self
            .send_request("/providers/proxies", Method::GET, None)
            .await?;
        self.set_data_providers_proxies(data).await;
        Ok(self)
    }

    async fn get_connections(&self) -> Result<Value, E> {
        let data = self.send_request("/connections", Method::GET, None).await?;
        Ok(data)
    }

    async fn delete_connection(&self, id: &str) -> Result<(), E> {
        let _ = self
            .send_request(&format!("/connections/{}", id), Method::DELETE, None)
            .await?;
        Ok(())
    }

    async fn test_proxy_delay(
        &self,
        name: &str,
        test_url: Option<String>,
        timeout: i32,
    ) -> Result<Value, E> {
        let test_url = test_url.unwrap_or("http://cp.cloudflare.com/generate_204".to_string());
        let data = self
            .send_request(
                &format!(
                    "/proxies/{}/delay?url={}&timeout={}",
                    name, test_url, timeout
                ),
                Method::GET,
                None,
            )
            .await?;
        Ok(data)
    }
}
