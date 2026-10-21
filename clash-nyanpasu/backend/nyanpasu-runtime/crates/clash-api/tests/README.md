# Clash API integration tests

`connections_decode.rs` checks the complete connection wire/IPC contract,
including unknown fields, duplicate keys, missing/null values and port bounds.
Run it with `cargo test -p clash-api --test connections_decode`.

`scalar_decode.rs` compares the scalar derives with Serde reference models for
open string enums and numeric/string samples, including externally tagged unit
objects, escaped strings, reader/owned input, integer bounds and invalid inputs.
Run it with `cargo test -p clash-api --test scalar_decode`.

`cargo run -p clash-api --example connections_allocations` measures global
allocation requests and requested bytes for parsing and dropping 100 synthetic
frames of 150 connections, with and without nested unknown fields. Fixture
construction and parser warm-up are excluded. Compare the same fixture, build
profile and dependency lockfile before and after a change. This probe measures
allocation churn, not parsing speed or Windows process memory; application
Private Commit and slow-consumer retention still need separate measurements.

`cargo run --release -p clash-api --example connections_timing` measures parse/drop
time with the system allocator, separately from DHAT. It uses the same two frames,
20 warm-up frames, and the median of nine groups of 500 frames. Run with other
builds and CPU-heavy work stopped. Compare the same toolchain, lockfile and Cargo
profile: this workspace optimizes release builds for size with LTO, so standalone
benchmark crates using Cargo's default release settings are a different workload.

Connection wire models derive `nyanpasu_serde::DeserializeMap` and their flattened
known field group derives `DeserializeFields`. The [derive library](../../nyanpasu-serde/README.md)
documents supported attributes and limitations. These derives preserve direct
typed field reads without maintaining a second field list in handwritten visitors.

`mihomo.rs` starts the platform core from `tests/bin`, then tests HTTP plus
the local transport available on that platform:

- Windows: HTTP and named pipe;
- Linux/macOS: HTTP and Unix domain socket.

The real-core test is ignored by the default test run because the downloaded
binary is not committed. Prepare and run it with:

```shell
deno run -A scripts/prepare-mihomo.ts
cargo test -p clash-api --test mihomo -- --ignored
```

## API coverage

Every public async `Client` endpoint is called against the real mihomo process
by `mihomo.rs`:

| Area | Methods |
| --- | --- |
| Base/config | `hello`, `version`, `configs`, `update_config`, `patch_config` |
| Streams | `traffic`, `traffic_ws`, `memory`, `memory_ws`, `logs`, `logs_ws`, `structured_logs`, `structured_logs_ws` |
| Connections | `connections`, `connections_ws`, `close_connection`, `close_all_connections` |
| Proxies/groups | `groups`, `group`, `group_delay`, `proxies`, `proxy`, `proxy_delay`, `select_proxy`, `clear_proxy_selection` |
| Proxy providers | `proxy_providers`, `proxy_provider`, `update_proxy_provider`, `healthcheck_proxy_provider`, `provider_proxy`, `provider_proxy_delay` |
| Rules | `rules`, `patch_rules`, `rule_providers`, `update_rule_provider` |
| DNS/storage | `dns_query`, `storage_get`, `storage_put`, `storage_delete` |
| Cache/debug | `flush_fake_ip_cache`, `flush_dns_cache`, `collect_garbage` |
| External mutation | `restart`, `upgrade`, `upgrade_ui`, `update_geo_databases`, `upgrade_geo_databases` |

The test runs a temporary copy of the supplied core. `upgrade_ui` downloads a
local ZIP fixture, both Geo update routes run with Geo databases disabled, and
`restart` must bring the controller and WebSocket endpoint back. Mihomo's core
updater has hard-coded GitHub URLs, so `upgrade` is exercised through its real
route and deterministic pre-download error path; it cannot replace the source
binary or require external network access. The Axum test only supplements this
with exact route/query serialization checks.

Generate merged normal-test and real-mihomo coverage with:

```shell
cargo install cargo-llvm-cov --locked
rustup component add llvm-tools-preview
deno run -A scripts/coverage-clash-api.ts
```

The script fails if a newly added public async API has no call in the real-core
test. It writes HTML, LCOV, and JSON summary reports under `target/llvm-cov/`,
enforces an 80% whole-crate line/function baseline, and an 85% API-module
line/function baseline.
