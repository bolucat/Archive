# Wind Architecture

```
┌─────────────────────────────────────────────────────┐
│                   wind CLI                           │
│                                                      │
│  inbounds[]              outbounds[]                 │
│  ┌──────────┐            ┌──────────┐                │
│  │ socks    │            │ tuic     │                │
│  │ :6666    │            │ :9443    │                │
│  └────┬─────┘            └────┬─────┘                │
│       │                       │                      │
│       ▼                       ▼                      │
│  ┌──────────────┐      ┌──────────────┐              │
│  │ SocksInbound │      │ TuicOutbound │              │
│  └──────┬───────┘      └──────┬───────┘              │
│         │                     │                      │
│         │  InboundCallback    │ OutboundAction       │
│         ▼                     │                      │
│  ┌──────────┐                 │                      │
│  │Dispatcher│◄────────────────┘                      │
│  │          │  named handlers                        │
│  │  Router  │  (tuic-out, naive-out, ...)            │
│  └──────────┘                                        │
│                                                      │
│  Runtime: AppContext (CancellationToken + Tasks)     │
└─────────────────────────────────────────────────────┘
```

## Inbounds

An **inbound** accepts connections from the outside world and feeds them into
the dispatcher. Each inbound implements `AbstractInbound`.

| Type | Protocol | Port | Description |
|------|----------|------|-------------|
| `socks` | SOCKS5 | 6666 | SOCKS5 proxy inbound |
| `tuic` | TUIC over QUIC | 443 | TUIC server inbound (planned) |

### Adding a new inbound

1. Create a config variant in `InboundConfig` (in `persistent.rs`)
2. Add the runtime opts variant in `InboundOpts` (in `runtime.rs`)
3. Add a variant to `InboundHandle` enum (in `main.rs`)
4. Implement `listen()` dispatch

## Outbounds

An **outbound** forwards traffic to a remote proxy server. Each outbound
implements `AbstractOutbound` and is wrapped as `OutboundAction` for storage
in the dispatcher.

| Type | Protocol | Library | Description |
|------|----------|---------|-------------|
| `tuic` | TUIC v5 | quinn + QUIC + BBR | High-performance QUIC proxy |
| `naive` | NaiveProxy | cronet-rs | NaiveProxy via Cronet (HTTP/2 or QUIC+padding) |

All outbounds are registered with a **tag** (name) in the `Dispatcher`. The
`Router` picks which outbound to use.

### Adding a new outbound

1. Create a config variant in `OutboundConfig` (in `persistent.rs`)
2. Add the runtime opts variant in `OutboundOpts` (in `runtime.rs`)
3. Implement `AbstractOutbound` for the new type
4. In `build_dispatcher()`, use `OutboundAsAction { inner: out }` to wrap it
5. Register with `disp.add_handler("tag-name", Arc::new(action))`

## Dispatcher

The `Dispatcher<R>` (from `wind-core`) sits between inbounds and outbounds:

1. Receives every connection through `InboundCallback`
2. Calls `Router::route()` to decide which outbound to use
3. Forwards to the matching `OutboundAction` handler

The built-in `AclRouter` supports Clash/Mihomo-style rule syntax:

```yaml
rules:
  - DOMAIN-SUFFIX,google.com,tuic-out
  - DOMAIN-KEYWORD,facebook,reject
  - IP-CIDR,10.0.0.0/8,direct
  - MATCH,tuic-out
```

## Config Format

```yaml
# config.yaml —— example with all features
inbounds:
  - type: socks
    tag: socks-in
    listen_addr: "127.0.0.1:6666"
    allow_udp: true

  - type: tuic          # future: TUIC server inbound
    tag: tuic-in
    listen_addr: "0.0.0.0:443"
    # ... TLS cert, users, etc.

outbounds:
  - type: tuic
    tag: tuic-out
    server_addr: "127.0.0.1:9443"
    uuid: "c1e6dbe2-..."
    password: "test_passwd"
    skip_cert_verify: false

  - type: naive
    tag: naive-out
    server_address: "your-server.com:443"
    username: "user"
    password: "pass"
    quic_enabled: true
    cronet_lib_path: "/usr/lib/libcronet.so"

# routing:
#   default: tuic-out
#   rules:
#     - DOMAIN-SUFFIX,example.com,naive-out
```

## Glossary

| Term | Definition |
|------|------------|
| **Inbound** | Listens for connections and feeds them into the proxy chain |
| **Outbound** | Forwards traffic to a remote proxy or directly to destination |
| **Dispatcher** | Central router that connects inbounds to outbounds |
| **Router** | Policy engine that decides which outbound to use |
| **Tag** | A human-readable name for an inbound or outbound instance |
| **Action** | Wrapper trait (`OutboundAction`) for storing outbounds in the dispatcher |
