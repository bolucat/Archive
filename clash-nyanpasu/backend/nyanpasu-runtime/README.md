# Nyanpasu Runtime

The runtime monorepo for Nyanpasu ([hitokoto-osc/nyanpasu-runtime](https://github.com/hitokoto-osc/nyanpasu-runtime)), hosting the privileged service and its supporting crates.

## Nyanpasu Service

A Service for Nyanpasu to make it easier to operate the privileged actions.

### Relations

```mermaid
flowchart LR
    UI["Nyanpasu UI"] <--> IPC

    subgraph IPC["IPC Bridge"]
        NP["Named Pipe (Windows)"]
        US["Unix Socket (Unix)"]
    end

    IPC <--> SVC

    subgraph SVC["Nyanapsu Service"]
        CM["Core Manager"]
        NM["Net Manager"]
        MORE["..."]
    end
```

## 

### nyanpasu-service (`nyanpasu_service/`)

The main entrance of the service — a thin binary shell (`main.rs` + `win_service.rs`) over `nyanpasu-service-runtime`. It provides a control plane to manage the service, and a `rpc` subcommand to test the service.

CLI subcommands:

- `install` / `uninstall` / `start` / `stop` / `restart` — service control plane (requires elevation).
- `server` — the actual service body, invoked by the service manager (SCM / systemd / launchd).
- `status` — service status and health check (if running), supports `--json`.
- `update` — self-update (`--check` works without elevation).
- `rpc` — debug RPC shortcuts: `start-core` / `stop-core` / `restart-core` / `apply-config` / `check-config` / `recover-core` / `inspect-logs` / `set-dns`.

View the service info:

```shell
./nyanpasu-service status # service status and health check(if running)
./nyanpasu-service version # build info only
```

### nyanpasu-ipc (`nyanpasu_ipc/`)

An IPC bridge crate between the service and the client. It defines the local IPC protocol: a set of HTTP/JSON operations carried over `named_pipe` on Windows and `unix_socket` on unix-like systems (default endpoint: `\\.\pipe\nyanpasu_ipc` / `/var/run/nyanpasu_ipc.sock`).

Structure:

- `api` — the protocol contract (`IpcOperation`), response envelope `R<'a, T>`, request/response bodies for core lifecycle (`/core/start|stop|check`, and the v2 control plane `/v2/core/submit|operation|status`), logs, `set_dns`, `status`, and a WebSocket event stream (`/ws/events`).
- `client` (feature `client`) — a reqwest-based `Client` plus a `shortcuts` mod for swift client rpc calls (`status()`, `start_core()`, `submit_core()`, ...).
- `server` (feature `server`) — a `create_server` fn to hold an axum server on the local transport.
- `types` — wire status types (`CoreState`, `CoreInfos`, ...).
- `utils` — socket path resolution and endpoint permissions.

Security notes:

- When installing the service, it collects the users info (sid in windows, username in unix) for security.
  - Windows: grant ACL (SDDL) to the pipe.
  - Unix: add user to the `nyanpasu` group and grant the group to the socket.

### nyanpasu-service-runtime (`crates/nyanpasu-service-runtime`)

The whole runtime implementation behind the `nyanpasu-service` binary (its lib name is intentionally kept as `nyanpasu_service`). It is only consumed by the binary shell.

- Entry point: `handler() -> ExitCode` — parses the CLI, dispatches subcommands, maps errors to exit codes.
- `cmds/` — clap CLI definitions and subcommand implementations.
- `server/` — the axum RPC server (`run()`): HTTP routing (core lifecycle, status, logs, network, ws events), `CoreManagerService` bridging to `nyanpasu-core-manager`, an `EventHub` broadcaster and an in-memory log buffer.
- `utils/` — service install/start/stop across platforms (macOS goes through `launchctl`), elevation checks, service data/runtime directories, Windows ACL helpers.
- Features: `deadlock_detection`, `tracing` (console-subscriber), `debug` (both), `hardware-lock-elision`.

## Crates

Supporting libraries under `crates/`.

### nyanpasu-core-manager (`crates/nyanpasu-core-manager`)

The core process lifecycle manager for Clash-family cores (mihomo, clash-rs, clash premium, meow): epoch-based instantiation, health probing, crash recovery with backoff, orphan process reaping, lossless switching and hot config application.

Key API:

- `CoreManager::new(ManagerOptions { runtime_dir, .. })` then `start(spec)` / `stop()` / `switch(spec)` / `restart()` / `apply_config(spec, expected_revision)` / `check_config(spec)` / `status()` / `subscribe()` (a `watch` channel of `CoreStatus`).
- `InstanceSpec` / `CoreKind` describe what to run; `HealthPolicy` / `HealthProbe` customize probing.
- Config is applied through a CAS pipeline with typed outcomes (`ApplyOutcome`, `SwitchOutcome`), atomically staged in the runtime dir.

See `crates/nyanpasu-core-manager/README.md` for state machine diagrams and the switch degradation matrix.

### nyanpasu-core-metadata (`crates/nyanpasu-core-metadata`)

A pure data/metadata crate — the shared "vocabulary" between the core manager and the IPC layer:

- `CoreKind` / `ClashCoreKind` and `ClashCoreResourceVariant` (with platform-aware `binary_name()`).
- `CoreDistribution` / `VariantTag` — identity of an installed core build.
- `Feature` / `FeatureSupport` — version-gated capability checks (e.g. `NamedPipeIpc`, `UnixSocketIpc`).
- `LogFrame` / `LogLevel` — normalized core console log records shared by the log pipeline.

### clash-api (`crates/clash-api`)

A strongly-typed async client for the Mihomo/Clash External Controller (REST + WebSocket), built on reqwest.

- `Client` / `ClientBuilder` with `Host` transports: `Http`, `NamedPipe` (Windows only), `UnixSocket` (Unix only); supports bearer `secret` and pluggable `RetryPolicy`.
- Typed endpoint methods grouped by domain: configs, proxies/providers, rules, connections, DNS, logs/traffic/memory streams, maintenance (restart/upgrade).
- DTOs derive `specta::Type` so TypeScript bindings can be generated for the frontend.

### nyanpasu-utils (`crates/nyanpasu-utils`)

Shared utility library for Nyanpasu apps, feature-gated by module:

- `core` (`core_manager`) — core type definitions (`ClashCoreType`) and per-core launch arguments.
- `dirs` — platform-specific config/data directory suggestions.
- `runtime` — a global shared Tokio runtime handle.
- `os` — elevation checks, graceful child kill, PID files.
- `process` — a supervised subprocess framework (`Command`, `Supervisor`, epoch PID records with orphan reaping).
- `io` / `atomic_fs` — line reading, directory locks and atomic file writes.
- `network` — macOS-only DNS get/set helpers.

## Development

Run with development preference:

```shell
cargo debug-run
```

Build with development preference:

```shell
cargo debug-build
```
