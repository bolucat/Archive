#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// note: the tungstenite error variant got bigger, so now clippy complains. but it's only 136 bytes, so I think it's fine. Boxing this would require a breaking change.
#![allow(clippy::result_large_err)]

//! Provides wrappers for [`reqwest`][2] to enable [`WebSocket`][1] connections.
//!
//! # Example
//!
//! ```
//! # use reqwest::Client;
//! # use reqwest_websocket::{Message, Error};
//! # use futures_util::{TryStreamExt, SinkExt};
//! #
//! # fn main() {
//! #     // Intentionally ignore the future. We only care that it compiles.
//! #     let _ = run();
//! # }
//! #
//! # async fn run() -> Result<(), Error> {
//! // Extends the `reqwest::RequestBuilder` to allow WebSocket upgrades.
//! use reqwest_websocket::Upgrade;
//!
//! // Creates a GET request, upgrades and sends it.
//! let response = Client::default()
//!     .get("wss://echo.websocket.org/")
//!     .upgrade() // Prepares the WebSocket upgrade.
//!     .send()
//!     .await?;
//!
//! // Turns the response into a WebSocket stream.
//! let mut websocket = response.into_websocket().await?;
//!
//! // The WebSocket implements `Sink<Message>`.
//! websocket.send(Message::Text("Hello, World".into())).await?;
//!
//! // The WebSocket is also a `TryStream` over `Message`s.
//! while let Some(message) = websocket.try_next().await? {
//!     if let Message::Text(text) = message {
//!         println!("received: {text}")
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! [1]: https://en.wikipedia.org/wiki/WebSocket
//! [2]: https://docs.rs/reqwest/latest/reqwest/index.html

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "middleware")]
mod middleware;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod protocol;
#[cfg(target_arch = "wasm32")]
mod wasm;

use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(docsrs, doc(cfg(not(target_arch = "wasm32"))))]
pub use crate::native::HandshakeError;
pub use crate::protocol::{CloseCode, Message};
pub use bytes::Bytes;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use reqwest::IntoUrl;

/// Errors returned by `reqwest_websocket`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_arch = "wasm32"))))]
    #[error("websocket upgrade failed")]
    Handshake(#[from] HandshakeError),

    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),

    #[cfg(not(target_arch = "wasm32"))]
    #[cfg_attr(docsrs, doc(cfg(not(target_arch = "wasm32"))))]
    #[error("tungstenite error")]
    Tungstenite(#[from] tungstenite::Error),

    #[cfg(target_arch = "wasm32")]
    #[cfg_attr(docsrs, doc(cfg(target_arch = "wasm32")))]
    #[error("web_sys error")]
    WebSys(#[from] wasm::WebSysError),

    /// Error during serialization/deserialization.
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    #[error("serde_json error")]
    Json(#[from] serde_json::Error),

    #[cfg(feature = "middleware")]
    #[error("reqwest_middleware error")]
    ReqwestMiddleware(#[from] reqwest_middleware::Error),
}

/// Opens a `WebSocket` connection at the specified `URL`.
///
/// This is a shorthand for creating a [`Request`], sending it, and turning the
/// [`Response`] into a [`WebSocket`].
///
/// [`Request`]: reqwest::Request
/// [`Response`]: reqwest::Response
pub async fn websocket(url: impl IntoUrl) -> Result<WebSocket, Error> {
    builder_http1_only(reqwest::Client::builder())
        .build()?
        .get(url)
        .upgrade()
        .send()
        .await?
        .into_websocket()
        .await
}

#[inline]
#[cfg(not(target_arch = "wasm32"))]
fn builder_http1_only(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder.http1_only()
}

#[inline]
#[cfg(target_arch = "wasm32")]
fn builder_http1_only(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder
}

/// A generic client.
///
/// This is needed by [`RequestBuilder`] to be generic over the specific implementation of a client.
/// Its only requirement is to be able to execute [`reqwest::Request`]s.
///
/// This is implemented for [`reqwest::Client`] and [`reqwest_middleware::ClientWithMiddleware`] (with `middleware` feature).
/// It provides a single interface for executing a [`reqwest::Request`].
pub trait Client {
    fn execute(
        &self,
        request: reqwest::Request,
    ) -> impl Future<Output = Result<reqwest::Response, Error>> + '_;
}

impl Client for reqwest::Client {
    async fn execute(&self, request: reqwest::Request) -> Result<reqwest::Response, Error> {
        self.execute(request).await.map_err(Into::into)
    }
}

/// A generic request builder.
///
/// This is needed by [`Upgraded`] to be generic over the specific implementation of a request (and client).
/// Its only requirements are that it provides the specific client type, and can build itself into a client and a [`reqwest::Request`].
pub trait RequestBuilder {
    type Client: Client;

    fn build_split(self) -> (Self::Client, Result<reqwest::Request, Error>);
}

impl RequestBuilder for reqwest::RequestBuilder {
    type Client = reqwest::Client;

    fn build_split(self) -> (Self::Client, Result<reqwest::Request, Error>) {
        let (client, request) = reqwest::RequestBuilder::build_split(self);
        (client, request.map_err(Into::into))
    }
}

/// Extension trait for requests builders that can be upgraded to a websocket connection.
///
/// This is automatically implemented for anything that implements our [`RequestBuilder`] trait.
pub trait Upgrade: Sized {
    /// Upgrades the [`RequestBuilder`] to perform a `WebSocket` handshake.
    ///
    /// This returns a wrapped type, so you must do this after you set up
    /// your request, and just before sending the request.
    fn upgrade(self) -> Upgraded<Self>;
}

impl<R> Upgrade for R
where
    R: RequestBuilder,
{
    fn upgrade(self) -> Upgraded<Self> {
        Upgraded::new(self)
    }
}

/// Wrapper for a [`reqwest::RequestBuilder`] that performs the
/// `WebSocket` handshake when sent.
pub struct Upgraded<R> {
    inner: R,
    protocols: Vec<String>,
    #[cfg(not(target_arch = "wasm32"))]
    web_socket_config: Option<tungstenite::protocol::WebSocketConfig>,
}

impl<R> Upgraded<R>
where
    R: RequestBuilder,
{
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            protocols: vec![],
            #[cfg(not(target_arch = "wasm32"))]
            web_socket_config: None,
        }
    }

    /// Selects which sub-protocols are accepted by the client.
    pub fn protocols<S: Into<String>>(mut self, protocols: impl IntoIterator<Item = S>) -> Self {
        self.protocols = protocols.into_iter().map(Into::into).collect();

        self
    }

    /// Sets the WebSocket configuration.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn web_socket_config(mut self, config: tungstenite::protocol::WebSocketConfig) -> Self {
        self.web_socket_config = Some(config);
        self
    }

    /// Sends the request and returns an [`UpgradeResponse`].
    pub async fn send(self) -> Result<UpgradeResponse, Error> {
        #[cfg(not(target_arch = "wasm32"))]
        let inner = native::send_request(self.inner, &self.protocols).await?;

        #[cfg(target_arch = "wasm32")]
        let inner = {
            let request = self.inner.build_split().1?;
            wasm::WebSysWebSocketStream::new(request, &self.protocols).await?
        };

        Ok(UpgradeResponse {
            inner,
            protocols: self.protocols,
            #[cfg(not(target_arch = "wasm32"))]
            web_socket_config: self.web_socket_config,
        })
    }
}

/// The server's response to the `WebSocket` upgrade request.
///
/// On non-wasm platforms, this implements `Deref<Target = Response>`, so you
/// can access all the usual information from the [`reqwest::Response`].
pub struct UpgradeResponse {
    #[cfg(not(target_arch = "wasm32"))]
    inner: native::WebSocketResponse,

    #[cfg(target_arch = "wasm32")]
    inner: wasm::WebSysWebSocketStream,

    #[allow(dead_code)]
    protocols: Vec<String>,

    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    web_socket_config: Option<tungstenite::protocol::WebSocketConfig>,
}

#[cfg(not(target_arch = "wasm32"))]
impl std::ops::Deref for UpgradeResponse {
    type Target = reqwest::Response;

    fn deref(&self) -> &Self::Target {
        &self.inner.response
    }
}

impl UpgradeResponse {
    /// Turns the response into a `WebSocket`.
    /// This checks if the `WebSocket` handshake was successful.
    pub async fn into_websocket(self) -> Result<WebSocket, Error> {
        #[cfg(not(target_arch = "wasm32"))]
        let (inner, protocol) = self
            .inner
            .into_stream_and_protocol(self.protocols, self.web_socket_config)
            .await?;

        #[cfg(target_arch = "wasm32")]
        let (inner, protocol) = {
            let protocol = self.inner.protocol();
            (self.inner, Some(protocol))
        };

        Ok(WebSocket { inner, protocol })
    }

    /// Consumes the response and returns the inner [`reqwest::Response`].
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_inner(self) -> reqwest::Response {
        self.inner.response
    }
}

/// A `WebSocket` connection. Implements [`futures_util::Stream`] and
/// [`futures_util::Sink`].
#[derive(Debug)]
pub struct WebSocket {
    #[cfg(not(target_arch = "wasm32"))]
    inner: native::WebSocketStream,

    #[cfg(target_arch = "wasm32")]
    inner: wasm::WebSysWebSocketStream,

    protocol: Option<String>,
}

/// Native stream retaining tungstenite's reference-counted text payloads.
#[cfg(not(target_arch = "wasm32"))]
pub type NativeWebSocketStream = native::WebSocketStream;

impl WebSocket {
    /// Takes the native stream and negotiated protocol after handshake validation.
    #[must_use]
    #[cfg(not(target_arch = "wasm32"))]
    pub fn into_native_parts(self) -> (NativeWebSocketStream, Option<String>) {
        (self.inner, self.protocol)
    }

    /// Returns the protocol negotiated during the handshake.
    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_deref()
    }

    /// Closes the connection with a given code and (optional) reason.
    ///
    /// # WASM
    ///
    /// On wasm `code` must be [`CloseCode::Normal`], [`CloseCode::Iana(_)`],
    /// or [`CloseCode::Library(_)`]. Furthermore `reason` must be at most 123
    /// bytes long. Otherwise the call to [`close`][Self::close] will fail.
    pub async fn close(self, code: CloseCode, reason: Option<&str>) -> Result<(), Error> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut inner = self.inner;
            inner
                .close(Some(tungstenite::protocol::CloseFrame {
                    code: code.into(),
                    reason: reason.unwrap_or_default().into(),
                }))
                .await?;
        }

        #[cfg(target_arch = "wasm32")]
        self.inner.close(code.into(), reason.unwrap_or_default())?;

        Ok(())
    }
}

impl Stream for WebSocket {
    type Item = Result<Message, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match ready!(self.inner.poll_next_unpin(cx)) {
            None => Poll::Ready(None),
            Some(Err(error)) => Poll::Ready(Some(Err(error.into()))),
            Some(Ok(message)) => {
                match message.try_into() {
                    Ok(message) => Poll::Ready(Some(Ok(message))),

                    #[cfg(target_arch = "wasm32")]
                    Err(e) => match e {},

                    #[cfg(not(target_arch = "wasm32"))]
                    Err(e) => {
                        // this fails only for raw frames (which are not received)
                        panic!("Received an invalid frame: {e}");
                    }
                }
            }
        }
    }
}

impl Sink<Message> for WebSocket {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready_unpin(cx).map_err(Into::into)
    }

    fn start_send(mut self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        self.inner.start_send_unpin(item.into()).map_err(Into::into)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_flush_unpin(cx).map_err(Into::into)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_close_unpin(cx).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use futures_util::{SinkExt, TryStreamExt};
    use reqwest::Client;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{websocket, CloseCode, Message, Upgrade, WebSocket};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg(not(target_arch = "wasm32"))]
    #[derive(Debug)]
    pub struct TestServer {
        shutdown_sender: Option<tokio::sync::oneshot::Sender<()>>,
        http_url: String,
        ws_url: String,
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl TestServer {
        pub async fn new() -> Self {
            async fn handle_connection(mut socket: axum::extract::ws::WebSocket) {
                if let Some(protocol) = socket.protocol() {
                    if let Ok(protocol) = protocol.to_str() {
                        println!("server/protocol: {protocol:?}");
                        if let Err(error) = socket
                            .send(axum::extract::ws::Message::Text(
                                format!("protocol: {protocol}").into(),
                            ))
                            .await
                        {
                            eprintln!("server/send: {error}");
                            return;
                        }
                    } else {
                        println!("server/protocol: could not convert to utf-8");
                    }
                }

                while let Some(message) = socket.recv().await {
                    match message {
                        Ok(message) => match &message {
                            axum::extract::ws::Message::Text(_)
                            | axum::extract::ws::Message::Binary(_) => {
                                if let Err(error) = socket.send(message).await {
                                    eprintln!("server/send: {error}");
                                    break;
                                }
                            }
                            _ => {}
                        },
                        Err(error) => {
                            eprintln!("server/recv: {error}");
                            break;
                        }
                    }
                }
            }

            let (shutdown_sender, shutdown_receiver) = tokio::sync::oneshot::channel();
            let listener = tokio::net::TcpListener::bind(("localhost", 0))
                .await
                .unwrap();
            let port = listener.local_addr().unwrap().port();
            let app = axum::Router::new().route(
                "/",
                axum::routing::any(|ws: axum::extract::ws::WebSocketUpgrade| async move {
                    ws.protocols(["chat"]).on_upgrade(handle_connection)
                }),
            );

            // todo: I think we'll need to spawn this on a proper thread (for which we create a separate runtime) for this to be shared across multiple tests
            let _join_handle = tokio::spawn(async move {
                axum::serve(listener, app)
                    .with_graceful_shutdown(async move {
                        let _ = shutdown_receiver.await;
                    })
                    .await
                    .unwrap();
            });
            Self {
                shutdown_sender: Some(shutdown_sender),
                http_url: format!("http://localhost:{port}/"),
                ws_url: format!("ws://localhost:{port}/"),
            }
        }

        pub fn http_url(&self) -> &str {
            &self.http_url
        }

        pub fn ws_url(&self) -> &str {
            &self.ws_url
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(shutdown_sender) = self.shutdown_sender.take() {
                println!("Shutting down server");
                let _ = shutdown_sender.send(());
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub struct TestServer;

    #[cfg(target_arch = "wasm32")]
    impl TestServer {
        pub async fn new() -> Self {
            Self
        }

        pub fn http_url(&self) -> &str {
            "https://echo.websocket.org/"
        }

        pub fn ws_url(&self) -> &str {
            "wss://echo.websocket.org/"
        }
    }

    pub async fn test_websocket(mut websocket: WebSocket) {
        let text = "Hello, World!";
        websocket.send(Message::Text(text.into())).await.unwrap();

        while let Some(message) = websocket.try_next().await.unwrap() {
            match message {
                Message::Text(s) => {
                    if s == text {
                        return;
                    }
                }
                _ => {}
            }
        }

        panic!("didn't receive text back");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_with_request_builder() {
        let echo = TestServer::new().await;

        let websocket = Client::default()
            .get(echo.http_url())
            .upgrade()
            .send()
            .await
            .unwrap()
            .into_websocket()
            .await
            .unwrap();

        test_websocket(websocket).await;
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_shorthand() {
        let echo = TestServer::new().await;

        let websocket = websocket(echo.http_url()).await.unwrap();
        test_websocket(websocket).await;
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_with_ws_scheme() {
        let echo = TestServer::new().await;
        let websocket = websocket(echo.ws_url()).await.unwrap();

        test_websocket(websocket).await;
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_close() {
        let echo = TestServer::new().await;

        let websocket = websocket(echo.http_url()).await.unwrap();
        websocket
            .close(CloseCode::Normal, Some("test"))
            .await
            .expect("close returned an error");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn test_send_close_frame() {
        let echo = TestServer::new().await;

        let mut websocket = websocket(echo.http_url()).await.unwrap();
        websocket
            .send(Message::Close {
                code: CloseCode::Normal,
                reason: "Can you please reply with a close frame?".into(),
            })
            .await
            .unwrap();

        let mut close_received = false;
        while let Some(message) = websocket.try_next().await.unwrap() {
            match message {
                Message::Close { code, .. } => {
                    assert_eq!(code, CloseCode::Normal);
                    close_received = true;
                }
                _ => {}
            }
        }

        assert!(close_received, "No close frame was received");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    #[cfg_attr(
        target_arch = "wasm32",
        ignore = "echo.websocket.org ignores subprotocols"
    )]
    async fn test_with_subprotocol() {
        let echo = TestServer::new().await;

        let mut websocket = Client::default()
            .get(echo.http_url())
            .upgrade()
            .protocols(["chat"])
            .send()
            .await
            .unwrap()
            .into_websocket()
            .await
            .unwrap();

        assert_eq!(websocket.protocol(), Some("chat"));

        let message = websocket.try_next().await.unwrap().unwrap();
        match message {
            Message::Text(s) => {
                assert_eq!(s, "protocol: chat");
            }
            _ => {
                panic!("Expected text message with selected protocol");
            }
        }
    }

    #[test]
    fn close_code_from_u16() {
        let byte = 1008u16;
        assert_eq!(CloseCode::from(byte), CloseCode::Policy);
    }

    #[test]
    fn close_code_into_u16() {
        let text = CloseCode::Away;
        let byte: u16 = text.into();
        assert_eq!(byte, 1001u16);
        assert_eq!(u16::from(text), 1001u16);
    }
}
