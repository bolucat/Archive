# Wind Repository Guide for Agents

## Scope and instruction precedence

This file applies to the top-level Wind repository. The parent directory is a
collection of independent repositories, so run Git and Cargo commands from this
directory rather than from `rust-proxy/`.

Read `README.md`, `LLM.md`, the root `Cargo.toml`, and the relevant crate
manifest before changing code. More specific `AGENTS.md` files override this
guide. In particular, `forks/quiche/` is a separate Git submodule with its own
root and nested instructions.

## Repository layout

Wind is a Rust 2024 workspace. Its crates are intentionally split by layer:

| Path | Responsibility |
| --- | --- |
| `crates/wind/` | Application binary/library, CLI, configuration parsing and resolution, logging, and plugin assembly |
| `crates/wind-core/` | Shared application, dispatcher, flow, inbound/outbound, I/O, resolver, and shutdown abstractions |
| `crates/wind-base/` | Direct, lazy, load-balanced, loopback, resolved, and tunnel outbounds |
| `crates/wind-rule/` | Leaf crate for rule parsing and matching; keep it independent of other Wind crates |
| `crates/wind-acl/` | ACL syntax, typed IR, compilation, and routing engine |
| `crates/wind-geodata/` | GeoSite/GeoIP decoding, indexed snapshots, mmap loading, and queries |
| `crates/wind-dns/` | DNS configuration and Hickory-based resolver integration |
| `crates/wind-socks/` | SOCKS inbound and UDP handling |
| `crates/wind-quic/` | Backend-neutral QUIC traits and quinn/quiche adapters |
| `crates/wind-tuic/` | TUIC protocol implementation and quinn/quiche connection providers |
| `crates/wind-naive/` | Cronet-backed Naive outbound |
| `crates/wind-acme/` | ACME resolvers, HTTP-01 provisioning, and self-signed certificate support |
| `crates/wind-test/` | Shared protocol tests and benchmarks |
| `forks/quiche/` | Patched quiche Git submodule; also supplies the workspace's `datagram-socket` and `tokio-quiche` patches |
| `forks/rustls-acme/` | rustls-acme Git submodule used as the reference/editing checkout for the pinned Git dependency |

Search for an existing abstraction, configuration type, test helper, or backend
implementation before adding one. Keep protocol-neutral behavior in
`wind-core` or `wind-quic`, protocol behavior in its protocol crate, and
application wiring in `crates/wind`.

## Working rules

1. Check `git status --short` before editing. Preserve unrelated changes,
   including dirty submodules, and never reset or clean user work.
2. Keep changes narrow. Do not update dependencies, rewrite `Cargo.lock`, or
   alter a submodule pointer unless the task requires it.
3. Add focused tests close to the behavior under test. Run the smallest relevant
   package or test target first, then expand validation in proportion to the
   change.
4. Preserve public APIs, configuration compatibility, defaults, and feature
   combinations unless the requested change explicitly breaks them.
5. Treat authentication, TLS verification, certificate handling, routing and
   ACL verdicts, DNS behavior, traffic accounting, QUIC/TUIC interoperability,
   and graceful shutdown as security- or compatibility-sensitive.
6. Do not expose secrets from `.env`, `config.toml`, test certificates, UUIDs,
   credentials, or configured endpoints in source, tests, logs, diffs, or task
   summaries. Use synthetic values in fixtures.
7. Do not run release, publish, deployment, live-server, remote integration,
   stress-test, or benchmark-download workflows without an explicit request.

## Rust and architecture conventions

- Use edition 2024 idioms and the repository's `rustfmt.toml`: hard tabs,
  grouped imports, and a 128-column limit. Formatting requires nightly rustfmt.
- Prefer explicit error propagation with useful context. Avoid new panics in
  runtime paths; reserve `unwrap`/`expect` for tests or states whose invariant is
  locally proven.
- Keep async work cancellation-safe. New connection or session tasks must
  participate in the existing cancellation and graceful-shutdown hierarchy and
  must not leak sockets or detached tasks.
- Preserve backend neutrality. Shared TUIC semantics belong above the quinn and
  quiche providers; do not fix one backend by silently changing behavior in the
  other.
- Preserve feature hygiene. Test the smallest meaningful feature set when
  changing optional quinn, quiche, server, client, masquerade, ACME, or TLS
  provider code, and avoid making optional dependencies unconditional.
- Keep `wind-rule` a leaf crate. Routing vocabulary belongs there; ACL lowering,
  chain semantics, and verdict evaluation belong in `wind-acl`.
- Configuration has a persistent form and a resolved runtime form under
  `crates/wind/src/conf/`. When adding or changing a field, update parsing,
  defaults, resolution, validation, examples, and round-trip/regression tests as
  applicable. Do not silently change an existing default.
- Changes to the archived geodata snapshot layout require a deliberate format
  version bump and compatibility consideration. Query paths should remain
  bounded and panic-free for malformed or stale cache data.
- Never weaken certificate or hostname verification as a convenience. Any
  insecure mode must remain explicit and opt-in.

## Forks and submodules

`forks/quiche/` and `forks/rustls-acme/` are independent repositories. Before
editing either one, inspect its own status and instructions and validate it from
its own root. Do not treat an uncommitted submodule checkout as an ordinary
directory change.

The workspace intentionally patches `datagram-socket` and `tokio-quiche` to
paths under `forks/quiche/`. Preserve those patches unless the task explicitly
replaces them with a verified alternative. Initialize submodules only when the
task requires their contents:

```sh
git submodule update --init --recursive
```

Do not advance a submodule pointer merely because the local checkout is at a
different commit. A parent pointer update is a separate, reviewable change.

## Validation

Run commands from the Wind repository root. Start with targeted checks, for
example:

```sh
cargo test -p <package> <test-name>
cargo clippy -p <package> --all-targets
```

The normal repository-level validation is:

```sh
cargo +nightly fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
```

Use `cargo check`/`clippy` with the relevant explicit feature combinations when
changing feature-gated code. Do not assume a default-feature workspace build
covers both QUIC backends or every TLS provider.

Additional considerations:

- `wind-naive` runtime and end-to-end coverage requires the Chromium Cronet C
  shared library; report when that dependency prevents validation.
- ACME HTTP-01 and live certificate flows require network access, a domain, and
  often privileged port binding. Prefer unit tests and do not contact a real CA
  unless explicitly requested.
- Geodata benchmarks may download large external fixtures. Ordinary unit tests
  should use synthetic data instead.
- Platform-specific socket features such as TFO, MPTCP, transparent proxying,
  or interface binding require targeted validation on a supported OS.
- `just test` runs the ignored-test form and is not a substitute for
  `cargo test --workspace`.

If a check cannot run because of the platform, toolchain, network, native
library, privileges, or an external service, state exactly what ran and what
remains unverified.

## Documentation and commits

Update public documentation and examples when changing configuration, commands,
defaults, features, or observable protocol behavior. Keep comments focused on
the invariant or compatibility reason rather than restating the code.

Do not create commits unless asked. AI-assisted code, configuration, build
script, or substantial documentation commits must include the `Assisted-by:`
trailer specified in `LLM.md`, after any sign-off line.
