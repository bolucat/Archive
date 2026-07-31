use reqwest::Method;
use serde::{Serialize, de::DeserializeOwned};

use crate::{Client, Error, Result, retry::RequestMetadata};

#[derive(Clone, Debug, PartialEq, Eq, Hash, specta::Type)]
#[specta(transparent)]
pub struct StorageKey(String);

impl StorageKey {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty() {
            return Err(Error::InvalidArgument {
                argument: "key",
                message: "must not be empty".to_owned(),
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Client {
    pub async fn storage_get<T>(&self, key: &StorageKey) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let url = self.endpoint_with_segments("/storage", [key.as_str()])?;
        self.send_json(
            RequestMetadata::new("storage_get", Method::GET, true),
            || Ok(self.request_url(Method::GET, url.clone())),
        )
        .await
    }

    pub async fn storage_put<T>(&self, key: &StorageKey, value: &T) -> Result<()>
    where
        T: Serialize + ?Sized,
    {
        let url = self.endpoint_with_segments("/storage", [key.as_str()])?;
        self.send_empty(
            RequestMetadata::new("storage_put", Method::PUT, false),
            || Ok(self.request_url(Method::PUT, url.clone()).json(value)),
        )
        .await
    }

    pub async fn storage_delete(&self, key: &StorageKey) -> Result<()> {
        let url = self.endpoint_with_segments("/storage", [key.as_str()])?;
        self.send_empty(
            RequestMetadata::new("storage_delete", Method::DELETE, true),
            || Ok(self.request_url(Method::DELETE, url.clone())),
        )
        .await
    }
}
