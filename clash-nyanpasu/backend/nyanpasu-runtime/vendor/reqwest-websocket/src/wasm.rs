use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures_util::{Sink, Stream};
use reqwest::{Request, Url};
use tokio::sync::{mpsc, oneshot};
use web_sys::{
    js_sys::{Array, ArrayBuffer, JsString, Uint8Array},
    wasm_bindgen::{closure::Closure, JsCast, JsValue},
    CloseEvent, ErrorEvent, Event, MessageEvent,
};

use crate::protocol::{CloseCode, Message};

#[derive(Debug, thiserror::Error)]
pub enum WebSysError {
    #[error("invalid url: {0}")]
    InvalidUrl(Url),

    #[error("connection failed")]
    ConnectionFailed,

    #[error("{0}")]
    ErrorEvent(String),

    #[error("unknown error")]
    Unknown,
}

impl From<ErrorEvent> for WebSysError {
    fn from(event: ErrorEvent) -> Self {
        Self::ErrorEvent(event.message())
    }
}

impl From<JsValue> for WebSysError {
    fn from(_value: JsValue) -> Self {
        Self::Unknown
    }
}

#[derive(Debug)]
pub struct WebSysWebSocketStream {
    inner: web_sys::WebSocket,

    rx: mpsc::UnboundedReceiver<Option<Result<Message, WebSysError>>>,

    #[allow(dead_code)]
    on_message_callback: Closure<dyn FnMut(MessageEvent)>,

    #[allow(dead_code)]
    on_error_callback: Closure<dyn FnMut(Event)>,

    #[allow(dead_code)]
    on_close_callback: Closure<dyn FnMut(CloseEvent)>,
}

impl WebSysWebSocketStream {
    pub async fn new(request: Request, protocols: &Vec<String>) -> Result<Self, WebSysError> {
        let mut url = request.url().clone();
        let scheme = match url.scheme() {
            "http" | "ws" => "ws",
            "https" | "wss" => "wss",
            _ => return Err(WebSysError::InvalidUrl(url)),
        };
        if let Err(_) = url.set_scheme(scheme) {
            return Err(WebSysError::InvalidUrl(url));
        }

        // the channel for messages and errors
        let (tx, rx) = mpsc::unbounded_channel();

        // channel to signal when the websocket has been opened
        let (open_success_tx, open_success_rx) = oneshot::channel();
        let mut open_success_tx = Some(open_success_tx);

        // channel to signal an error while opening the channel
        let (open_error_tx, open_error_rx) = oneshot::channel();
        let mut open_error_tx = Some(open_error_tx);

        // create websocket
        let inner = web_sys::WebSocket::new_with_str_sequence(
            &url.to_string(),
            &protocols
                .into_iter()
                .map(|s| JsString::from(s.to_owned()))
                .collect::<Array>(),
        )
        .map_err(|_| WebSysError::ConnectionFailed)?;

        inner.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // register message handler
        let on_message_callback = {
            let tx = tx.clone();
            Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                tracing::debug!(event = ?event.data(), "message event");

                if let Ok(abuf) = event.data().dyn_into::<ArrayBuffer>() {
                    let array = Uint8Array::new(&abuf);
                    let data = array.to_vec();
                    let _ = tx.send(Some(Ok(Message::Binary(data.into()))));
                } else if let Ok(text) = event.data().dyn_into::<JsString>() {
                    let _ = tx.send(Some(Ok(Message::Text(text.into()))));
                } else {
                    tracing::debug!(event = ?event.data(), "received unknown message event");
                }
            })
        };
        inner.set_onmessage(Some(on_message_callback.as_ref().unchecked_ref()));

        // register error handler
        // this will try to put the first error into a oneshot channel for errors that
        // happen during opening. once that has been used, or the oneshot
        // channel is dropped, this uses the regular message channel
        let on_error_callback = {
            let tx = tx.clone();
            Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                let error = match event.dyn_into::<ErrorEvent>() {
                    Ok(error) => WebSysError::from(error),
                    Err(_event) => WebSysError::Unknown,
                };
                tracing::debug!("received error event: {error}");

                let error = if let Some(open_error_tx) = open_error_tx.take() {
                    match open_error_tx.send(error) {
                        Ok(()) => return,
                        Err(error) => error,
                    }
                } else {
                    error
                };

                let _ = tx.send(Some(Err(error)));
            })
        };
        inner.set_onerror(Some(on_error_callback.as_ref().unchecked_ref()));

        // register close handler
        let on_close_callback = {
            let tx = tx.clone();
            Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
                tracing::debug!("received close event");

                let _ = tx.send(Some(Ok(Message::Close {
                    code: event.code().into(),
                    reason: event.reason(),
                })));
                let _ = tx.send(None);
            })
        };
        inner.set_onclose(Some(on_close_callback.as_ref().unchecked_ref()));

        // register open handler
        let on_open_callback = Closure::<dyn FnMut(Event)>::new(move |_event: Event| {
            tracing::debug!("received open event");
            if let Some(tx) = open_success_tx.take() {
                let _ = tx.send(());
            }
        });
        inner.set_onopen(Some(on_open_callback.as_ref().unchecked_ref()));

        // wait for either the open event or an error
        tokio::select! {
            Ok(()) = open_success_rx => {},
            Ok(error) = open_error_rx => {
                // cleanup
                let _result = inner.close();
                inner.set_onopen(None);
                inner.set_onmessage(None);
                inner.set_onclose(None);
                inner.set_onerror(None);
                return Err(error);
            },
            else => {
                tracing::warn!("open sender dropped");
            }
        };

        // remove open handler
        inner.set_onopen(None);

        Ok(Self {
            inner,
            on_message_callback,
            on_error_callback,
            on_close_callback,
            rx,
        })
    }

    pub fn protocol(&self) -> String {
        self.inner.protocol()
    }

    pub fn close(self, code: CloseCode, reason: &str) -> Result<(), WebSysError> {
        self.inner.close_with_code_and_reason(code.into(), reason)?;
        Ok(())
    }
}

impl Drop for WebSysWebSocketStream {
    fn drop(&mut self) {
        tracing::debug!("websocket stream dropped");
        let _result = self.inner.close();
        self.inner.set_onmessage(None);
        self.inner.set_onclose(None);
        self.inner.set_onerror(None);
    }
}

impl Stream for WebSysWebSocketStream {
    type Item = Result<Message, WebSysError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx
            .poll_recv(cx)
            .map(|ready_value| ready_value.flatten())
    }
}

impl Sink<Message> for WebSysWebSocketStream {
    type Error = WebSysError;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
        match item {
            Message::Text(text) => self.inner.send_with_str(&text)?,
            Message::Binary(data) => self.inner.send_with_u8_array(&data)?,
            Message::Close { code, reason } => self
                .inner
                .close_with_code_and_reason(code.into(), &reason)?,
            #[allow(deprecated)]
            Message::Ping(_) | Message::Pong(_) => {
                // ignored!
            }
        }
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(self.inner.close().map_err(Into::into))
    }
}
