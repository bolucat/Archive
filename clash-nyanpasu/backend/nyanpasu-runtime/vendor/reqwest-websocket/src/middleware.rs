use crate::{Client, Error, RequestBuilder};

impl Client for reqwest_middleware::ClientWithMiddleware {
    async fn execute(&self, request: reqwest::Request) -> Result<reqwest::Response, Error> {
        self.execute(request).await.map_err(Into::into)
    }
}

impl RequestBuilder for reqwest_middleware::RequestBuilder {
    type Client = reqwest_middleware::ClientWithMiddleware;

    fn build_split(self) -> (Self::Client, Result<reqwest::Request, Error>) {
        let (client, request) = reqwest_middleware::RequestBuilder::build_split(self);
        (client, request.map_err(Into::into))
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::{
        tests::{test_websocket, TestServer},
        Upgrade,
    };
    use std::sync::{Arc, Mutex};

    #[derive(Debug)]
    struct TestMiddleware {
        did_run: Arc<Mutex<bool>>,
    }

    #[async_trait::async_trait]
    impl reqwest_middleware::Middleware for TestMiddleware {
        async fn handle(
            &self,
            req: reqwest::Request,
            extensions: &mut http::Extensions,
            next: reqwest_middleware::Next<'_>,
        ) -> Result<reqwest::Response, reqwest_middleware::Error> {
            {
                let mut did_run = self.did_run.lock().unwrap();
                *did_run = true;
            }
            next.run(req, extensions).await
        }
    }

    //#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    //#[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[tokio::test]
    async fn websocket_with_middleware() {
        let echo = TestServer::new().await;

        let did_run = Arc::new(Mutex::new(false));
        let middleware = TestMiddleware {
            did_run: did_run.clone(),
        };

        let client = reqwest::Client::builder().http1_only().build().unwrap();
        let client = reqwest_middleware::ClientBuilder::new(client)
            .with(middleware)
            .build();

        let websocket = client
            .get(echo.http_url())
            .upgrade()
            .send()
            .await
            .unwrap()
            .into_websocket()
            .await
            .unwrap();

        test_websocket(websocket).await;

        let did_run = {
            let did_run = did_run.lock().unwrap();
            *did_run
        };
        assert!(did_run);
    }
}
