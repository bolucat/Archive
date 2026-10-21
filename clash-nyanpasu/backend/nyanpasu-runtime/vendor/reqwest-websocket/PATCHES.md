# Local changes to reqwest-websocket 0.6.0

Source: the published 0.6.0 crate, licensed under MIT (see LICENSE).

Expose `NativeWebSocketStream` and `WebSocket::into_native_parts` on native
platforms. This additive API retains tungstenite text payloads without the
wrapper converting them to owned Strings. Upgrade validation, configuration,
protocol negotiation, transport handling and the existing message API are unchanged.

Both the runtime and application workspaces patch crates.io to this directory,
so nyanpasu-utils and clash-api use the same upgrade response type. Standalone
consumers must apply the same Cargo patch until an upstream API is available.
