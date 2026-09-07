use std::{io, marker::PhantomData, pin::Pin, task::Poll};

use futures_core::Stream;
use futures_util::{StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;
use tokio_util::{codec::FramedRead, io::StreamReader};

use crate::{Error, Result};

/// A newline-delimited JSON response stream from Mihomo.
pub struct HttpStream<T> {
    inner: Pin<Box<dyn Stream<Item = Result<T>> + Send>>,
    _value: PhantomData<T>,
}

impl<T> HttpStream<T>
where
    T: DeserializeOwned + Send + 'static,
{
    pub(crate) fn from_response(response: reqwest::Response, operation: &'static str) -> Self {
        let body = response.bytes_stream().map_err(io::Error::other);
        let reader = StreamReader::new(body);
        let lines = FramedRead::new(
            reader,
            tokio_util::codec::LinesCodec::new_with_max_length(1024 * 1024),
        );
        let stream = lines.filter_map(move |line| async move {
            match line {
                Ok(line) if line.trim().is_empty() => None,
                Ok(line) => Some(
                    serde_json::from_str(&line)
                        .map_err(|source| Error::Decode { operation, source }),
                ),
                Err(source) => Some(Err(Error::Stream { operation, source })),
            }
        });

        Self {
            inner: Box::pin(stream),
            _value: PhantomData,
        }
    }
}

impl<T> Stream for HttpStream<T> {
    type Item = Result<T>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.as_mut().poll_next(cx)
    }
}

impl<T> Unpin for HttpStream<T> {}

impl<T> std::fmt::Debug for HttpStream<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("HttpStream").finish_non_exhaustive()
    }
}

/// Owned, typed JSON WebSocket subscription. Dropping it closes the transport.
/// Control frames are handled by the socket; decoding errors affect one frame.
/// No reconnect or replay occurs inside this stream.
pub struct WebSocketStream<T> {
    inner: Pin<Box<dyn Stream<Item = Result<T>> + Send>>,
}
impl<T: DeserializeOwned + Send + 'static> WebSocketStream<T> {
    pub(crate) fn new(socket: reqwest_websocket::WebSocket, operation: &'static str) -> Self {
        use reqwest_websocket::Message;
        let stream = socket
            .take_while(|frame| {
                futures_util::future::ready(!matches!(frame, Ok(Message::Close { .. })))
            })
            .filter_map(move |frame| async move {
                match frame {
                    Ok(Message::Text(text)) => Some(
                        serde_json::from_str(&text)
                            .map_err(|source| Error::Decode { operation, source }),
                    ),
                    Ok(Message::Binary(bytes)) => Some(
                        serde_json::from_slice(&bytes)
                            .map_err(|source| Error::Decode { operation, source }),
                    ),
                    Ok(_) => None,
                    Err(source) => Some(Err(Error::WebSocketRead { operation, source })),
                }
            });
        Self {
            inner: Box::pin(stream),
        }
    }
}
impl<T> Stream for WebSocketStream<T> {
    type Item = Result<T>;
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.as_mut().poll_next(cx)
    }
}
impl<T> std::fmt::Debug for WebSocketStream<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebSocketStream")
            .finish_non_exhaustive()
    }
}

// Some compatible controllers encode numeric samples as decimal strings.
pub(crate) fn deserialize_number<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + std::str::FromStr,
    T::Err: std::fmt::Display,
{
    use serde::{Deserialize, de::Error};
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number<T> {
        Number(T),
        Text(String),
    }
    match Number::<T>::deserialize(deserializer)? {
        Number::Number(value) => Ok(value),
        Number::Text(value) => value.parse().map_err(D::Error::custom),
    }
}
