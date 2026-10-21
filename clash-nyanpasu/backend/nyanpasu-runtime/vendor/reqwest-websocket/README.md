# `reqwest-websocket`

[![crates.io](https://img.shields.io/crates/v/reqwest-websocket.svg)](https://crates.io/crates/reqwest-websocket)
[![Documentation](https://docs.rs/reqwest-websocket/badge.svg)](https://docs.rs/reqwest-websocket)
[![MIT](https://img.shields.io/crates/l/reqwest-websocket.svg)](./LICENSE)
[![Build](https://github.com/jgraef/reqwest-websocket/actions/workflows/build.yaml/badge.svg)](https://github.com/jgraef/reqwest-websocket/actions/workflows/build.yaml)

Extension for [`reqwest`][2] to allow [websocket][1] connections.

This crate contains the extension trait [`Upgrade`][4], which adds an
`upgrade` method to `reqwest::RequestBuilder` that prepares the HTTP request to
upgrade the connection to a WebSocket. After you call `upgrade()`, you can send
your upgraded request as usual with `send()`, which will return an
`UpgradeResponse`. The `UpgradeResponse` wraps `reqwest::Response` (and also
dereferences to it), so you can inspect the response if needed. Finally, you can
use `into_websocket()` on the response to turn it into an async stream and sink
for messages. Both text and binary messages are supported.

## Example

For a full example take a look at [`hello_world.rs`](examples/hello_world.rs).

```rust
// Extends the `reqwest::RequestBuilder` to allow WebSocket upgrades.
use reqwest_websocket::Upgrade;

// Creates a GET request, upgrades and sends it.
let response = Client::default()
    .get("wss://echo.websocket.org/")
    .upgrade() // Prepares the WebSocket upgrade.
    .send()
    .await?;

// Turns the response into a WebSocket stream.
let mut websocket = response.into_websocket().await?;

// The WebSocket implements `Sink<Message>`.
websocket.send(Message::Text("Hello, World".into())).await?;

// The WebSocket is also a `TryStream` over `Message`s.
while let Some(message) = websocket.try_next().await? {
    if let Message::Text(text) = message {
        println!("received: {text}")
    }
}
```

## Support for WebAssembly

`reqwest-websocket` uses the HTTP upgrade functionality built into `reqwest`,
which is not available on WebAssembly. When you use `reqwest-websocket` in
WebAssembly, it falls back to using [`web_sys::WebSocket`][3]. This means that
everything except the URL (including query parameters) is not used for your
request.

[1]: https://en.wikipedia.org/wiki/WebSocket
[2]: https://docs.rs/reqwest/latest/reqwest/index.html
[3]: https://docs.rs/web-sys/latest/web_sys/struct.WebSocket.html
[4]: https://docs.rs/reqwest-websocket/latest/reqwest_websocket/trait.Upgrade.html
