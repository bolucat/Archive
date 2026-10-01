# Wind

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/rust-proxy/wind?utm_source=badge)

## Naive / cronet

The `naive` outbound (`crates/wind-naive`) uses Chromium Cronet through the
[`cronet-rs`](https://github.com/rust-proxy/cronet-rs) bindings, which need a
`libcronet` shared library at runtime. To avoid a manual install, build with the
opt-in `download` feature:

```bash
cargo build -p wind --features download
```

`cronet-sys`'s build script then fetches the prebuilt matching the target from
[`rust-proxy/cronet-binaries`](https://github.com/rust-proxy/cronet-binaries/releases)
into `OUT_DIR` and verifies its SHA-256 against a pinned checksum. The feature is
off by default so ordinary builds never touch the network. See
[`crates/wind-naive/README.md`](crates/wind-naive/README.md) for details.

## Acknowledgments

- [fast-socks5](https://github.com/dizda/fast-socks5)
- [yimu-rs](https://github.com/yfaming/yimu-rs)