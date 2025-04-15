use crate::model::E;
use http_body_util::{BodyExt, Full};
use hyper::{
    Method, Request,
    body::Bytes,
    header::{HeaderName, HeaderValue},
};
use hyper_util::client::legacy::Client;
use hyperlocal::{UnixClientExt, Uri};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct UnixClient {
    client: Arc<Mutex<Client<hyperlocal::UnixConnector, Full<Bytes>>>>,
}

impl UnixClient {
    pub fn new() -> Self {
        let client: Client<_, Full<Bytes>> = Client::unix();
        Self {
            client: Arc::new(Mutex::new(client)),
        }
    }

    pub async fn generate_unix_path(&self, socket_path: &str, path: &str) -> Uri {
        Uri::new(socket_path, path).into()
    }

    pub async fn send_request(
        &self,
        socket_path: String,
        path: &str,
        method: Method,
        body: Option<Value>,
    ) -> Result<Value, E> {
        let uri = self.generate_unix_path(socket_path.as_str(), path).await;

        let mut request_builder = Request::builder().method(method).uri(uri);

        let body_bytes = if let Some(body) = body {
            request_builder = request_builder.header(
                HeaderName::from_static("Content-Type"),
                HeaderValue::from_static("application/json"),
            );
            Bytes::from(serde_json::to_vec(&body)?)
        } else {
            Bytes::new()
        };

        let request = request_builder.body(Full::new(body_bytes))?;

        let response = self.client.lock().await.request(request).await?;
        let body_bytes = response.into_body().collect().await?.to_bytes();
        let json_value = serde_json::from_slice(&body_bytes)?;

        Ok(json_value)
    }
}
