# wind-naive

NaiveProxy outbound for the [Wind](https://github.com/rust-proxy/wind) proxy toolkit.

Uses [cronet-rs](https://github.com/rust-proxy/cronet-rs) (Chromium Cronet C API bindings) to
establish HTTP/2 or QUIC CONNECT tunnels with NaiveProxy padding protocol.

## Usage

```toml
[dependencies]
wind-naive = "0.1"
# feature "dynamic" (default): load libcronet at runtime via dlopen
# feature "static-link":      link libcronet.a at compile time
```

```rust
use wind_naive::{NaiveOutbound, NaiveOutboundOpts};

let outbound = NaiveOutbound::new(NaiveOutboundOpts {
    server_address: "your-server.com:443".into(),
    username: Some("user".into()),
    password: Some("pass".into()),
    quic_enabled: true,
    ..Default::default()
}).await?;
```

## Configuration

### In Wind CLI (`config.yaml`)

```yaml
inbounds:
  - type: socks
    tag: socks-in
    listen_addr: "127.0.0.1:6666"

outbounds:
  - type: naive
    tag: naive-out
    server_address: "your-server.com:443"
    username: "user"
    password: "pass"
    quic_enabled: true
    quic_congestion_control: "bbr"                  # default|bbr|bbrv2|cubic|reno
    cronet_lib_path: "/usr/lib/libcronet.so.119"   # optional
```

### Standalone Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `server_address` | `String` | required | NaiveProxy server (host:port) |
| `server_name` | `Option<String>` | server host | SNI override |
| `username` | `Option<String>` | `None` | Basic auth |
| `password` | `Option<String>` | `None` | Basic auth |
| `concurrency` | `u32` | `1` | Cronet connection pool size |
| `quic_enabled` | `bool` | `false` | Use QUIC instead of HTTP/2 |
| `quic_congestion_control` | `String` | `default` | `default` / `bbr` / `bbrv2` / `cubic` / `reno` |
| `trusted_root_certificates` | `Option<String>` | `None` | Custom CA PEM |
| `ech_enabled` | `bool` | `false` | Encrypted Client Hello |
| `extra_headers` | `HashMap<String,String>` | `{}` | Extra CONNECT headers |
| `cronet_lib_path` | `Option<String>` | `None` | Path to libcronet.so |

## libcronet

`wind-naive` requires the **Chromium Cronet** C shared library at runtime.

### Quick start (Linux)

| Step | Command |
|------|---------|
| 1. Get libcronet | Download from [naiveproxy releases](https://github.com/klzgrad/naiveproxy/releases) or build from Chromium |
| 2. Place it | `cp libcronet.so.119 /usr/local/lib/libcronet.so` |
| 3. Set path | `cronet_lib_path: "/usr/local/lib/libcronet.so"` in config, or `LD_LIBRARY_PATH` |

### Search order

When `cronet_lib_path` is `None`, the loader tries (in order):

```
1. libcronet.so            ← LD_LIBRARY_PATH / system default
2. ./libcronet.so          ← CWD
3. /usr/local/lib/libcronet.so
4. /opt/cronet/libcronet.so
```

## Architecture

```
[SOCKS5 :6666]
     │
     ▼
NaiveOutbound (cronet-rs)
     │
     ├── load_library("libcronet.so")
     ├── NaiveClient::start()        ── Cronet engine init
     ├── dial_and_handshake(target)  ── HTTP/2 CONNECT + padding
     │
     ▼
[NaiveProxy Server :443]
     │
     ▼
[Internet]
```

### Data relay

```
┌─────────────────────────┐
│    tokio async task     │
│  ┌───────────────────┐  │
│  │ select! loop      │  │
│  │ stream.read()    ──┼──┐ write_tx
│  │ read_rx.recv()  ◀──┼──┘ read_tx
│  └───────────────────┘  │
└─────────────────────────┘
         │ channels
┌─────────────────────────┐
│   std::thread (sync)    │
│  ┌───────────────────┐  │
│  │ NaiveConn         │  │  ← owns the Cronet stream
│  │ read() / write()  │  │  ← blocking FFI calls
│  └───────────────────┘  │
└─────────────────────────┘
```

## UDP (UDP-over-TCP v2)

NaiveProxy's classic protocol only tunnels **TCP** (an HTTP `CONNECT` byte
stream). To carry UDP, `wind-naive` layers [sing-box's UDP-over-TCP **v2**
framing](https://github.com/sagernet/sing/tree/main/common/uot) over a single
CONNECT tunnel:

- All datagrams for one UDP association are multiplexed over **one** tunnel,
  opened to the magic authority `sp.v2.udp-over-tcp.arpa` (instead of a literal
  TCP CONNECT per packet).
- Each frame is `address ‖ u16 length ‖ payload`, so replies flow back to the
  client (no more black-holed responses), and a single tunnel can fan out to
  many destinations.

> **Server requirement:** the upstream must understand UoT v2 (e.g. a sing-box
> server with a `naive` inbound). A stock Chromium NaiveProxy server only speaks
> TCP CONNECT and will reject the magic authority.

```
UdpStream ──frame──▶ [uplink chan] ──▶ ┌── std::thread (owns NaiveConn) ──┐
UdpStream ◀─packet── [downlink chan] ◀─┤  write_all(frame) / read_packet  │ ──▶ [Naive/UoT server]
                                       └──────────────────────────────────┘
```

## Building

```bash
# Default (dynamic loading)
cargo build

# With wind CLI integration
cargo build --features naive -p wind

# Static link (macOS/iOS recommended)
cargo build --no-default-features --features static-link
```

## License

MIT OR Apache-2.0
