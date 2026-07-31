# nyanpasu-core-manager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `nyanpasu-core-manager` crate per the approved spec (`docs/superpowers/specs/2026-07-18-nyanpasu-core-manager-design.md`): an epoch-based `Instance`/`CoreManager` on top of `nyanpasu-utils::process`, with version-probe startup confirmation, crash recovery, watch-based state notifications, and graceful core switching.

**Architecture:** `Instance` owns one epoch (immutable config) and wraps a `process::Supervisor` plus a monitor task that drives a `clash_api` version probe and a `watch<InstanceState>`. `CoreManager` serializes control commands behind one async `Mutex`, allocates epochs, orchestrates hard/graceful switches, and publishes a single `watch<CoreStatus>`. Health probing stays in this crate (no changes to `nyanpasu-utils`).

**Tech Stack:** Rust (edition 2024), tokio, `nyanpasu-utils` (`process` feature), `clash-api` (path deps), `serde_yaml_ng` for config introspection/derivation, thiserror, parking_lot, tokio-util (`CancellationToken`).

## Global Constraints

- Workspace: edition 2024, license GPL-3.0 (inherit via `edition.workspace = true` etc. like sibling crates).
- Spec decisions are binding: startup_timeout default **30s**, probe_interval default **250ms**, restart policy default `OnFailure { max_restarts: 5 }`, backoff exponential 1s→30s with jitter, stderr tail **32** lines, PATCH restore retry **3 tries × 500ms**, `changed_at` is unix **milliseconds**.
- `CoreState` / `CoreKind` / `ReadinessProbe` in `nyanpasu-utils` are NOT modified. Do not touch `nyanpasu-utils::core` (deprecation happens in P4, out of scope).
- Enums from `nyanpasu-utils::process` (`SupervisorEvent`, `ProcessEvent`, `ProcessError`) are `#[non_exhaustive]` — every `match` needs a `_` arm.
- Commit style: conventional commits scoped to the crate, e.g. `feat(nyanpasu-core-manager): ...` (matches repo history).
- All commands below run from the repo root `G:\Programs\Rust\nyanpasu-service` (PowerShell). `cargo test -p nyanpasu-core-manager` must be green at the end of every task.
- Tests must not hardcode ports or global pipe names: pick free ports by binding `127.0.0.1:0`, and parameterize pipe names with `std::process::id()`.
- Milestone note: the spec assigns `ControllerMode::Managed` to §M1 types; this plan deliberately introduces the `Managed` variant in M4 (Task 16+) so no task ships untestable stub arms. By end of M4 the spec's §4.2 is fully implemented. Similarly `switch`/`restart` return `Result<()>` in M3 and are migrated to `Result<SwitchOutcome>` in Task 19.

## File Structure

```
crates/nyanpasu-core-manager/
├── Cargo.toml                    # deps + [[bin]] fake core helper
├── src/
│   ├── lib.rs                    # module decls + re-exports (placeholder add() removed)
│   ├── kind.rs                   # CoreKind, launch args, SAFE_PATHS, parse_check_output, error_summary, check_config
│   ├── error.rs                  # Error (thiserror)
│   ├── state.rs                  # InstanceState/StopReason/CoreState/CoreStatus/SpecSummary/now_ms
│   ├── spec.rs                   # CoreSpec/InstanceSpec/InstanceOptions/ResolvedController/ControllerMode/ManagerOptions
│   ├── config.rs                 # YAML introspection (M1) + derivation (M4)
│   ├── health.rs                 # build_client + HealthCheck
│   ├── instance.rs               # Instance + monitor task (replaces the current draft file)
│   └── manager.rs                # CoreManager + switch orchestration
└── tests/
    ├── helpers/fake_core.rs      # [[bin]] nyanpasu-fake-core (mihomo simulator)
    ├── common/mod.rs             # shared test utilities
    ├── instance_lifecycle.rs     # M2 component tests
    ├── manager_orchestration.rs  # M3 component tests
    └── graceful_switch.rs        # M4 component tests
```

---

# M1 — Foundation types and config introspection

### Task 1: Crate scaffold + `kind.rs` (CoreKind, launch profiles, SAFE_PATHS)

**Files:**
- Modify: `crates/nyanpasu-core-manager/Cargo.toml`
- Modify: `crates/nyanpasu-core-manager/src/lib.rs`
- Create: `crates/nyanpasu-core-manager/src/kind.rs`
- Delete content of (rewritten later, keep file compiling): `crates/nyanpasu-core-manager/src/instance.rs`

**Interfaces:**
- Produces: `CoreKind` (`Mihomo | ClashPremium | ClashRs | Meow`, `#[non_exhaustive]`, `Copy`, `AsRef<str>`, `Display`), `CoreKind::run_args(&self, working_dir: &Utf8Path, config_path: &Utf8Path) -> Result<Vec<OsString>, Error>`, `CoreKind::check_args(working_dir: &Utf8Path, config_path: &Utf8Path) -> Vec<OsString>`, `pub fn mihomo_safe_paths(working_dir: &Utf8Path, config_dir: &Utf8Path) -> String`, `pub const MIHOMO_SAFE_PATHS_ENV_NAME: &str = "SAFE_PATHS"`.
- Consumes: `crate::error::Error` — Task 3 defines the full enum; this task creates a minimal `error.rs` with only `UnsupportedCore` so Task 1 compiles standalone.

- [ ] **Step 1: Replace Cargo.toml with the full dependency set**

```toml
[package]
name = "nyanpasu-core-manager"
version = "0.1.0"
edition.workspace = true
repository.workspace = true
license.workspace = true

[dependencies]
camino = { version = "1.1", features = ["serde1"] }
clash-api = { path = "../clash-api" }
nyanpasu-utils = { path = "../nyanpasu-utils", default-features = false, features = ["process"] }
parking_lot.workspace = true
serde_yaml_ng = "0.10"
thiserror.workspace = true
tokio.workspace = true
tokio-util.workspace = true
tracing.workspace = true

[dev-dependencies]
serde_json = "1"
tempfile = "3"

[[bin]]
name = "nyanpasu-fake-core"
path = "tests/helpers/fake_core.rs"
doc = false
```

Note: the `[[bin]]` target does not exist yet — create a placeholder now so the manifest stays valid:

```powershell
New-Item -ItemType Directory -Force crates/nyanpasu-core-manager/tests/helpers
Set-Content crates/nyanpasu-core-manager/tests/helpers/fake_core.rs "fn main() {}"
```

- [ ] **Step 2: Rewrite `src/lib.rs` (drop the `add` placeholder) and empty the draft `instance.rs`**

`src/lib.rs`:

```rust
//! Clash core lifecycle management: epoch-based instances, health-probed
//! startup, crash recovery, and core switching.
//!
//! Design: docs/superpowers/specs/2026-07-18-nyanpasu-core-manager-design.md

mod error;
pub mod instance;
pub mod kind;

pub use clash_api::Host;
pub use error::Error;
pub use kind::CoreKind;
```

`src/instance.rs` (the current draft types are superseded by the spec; Task 8 rewrites this file):

```rust
//! Single-epoch core instance. Implemented in M2.
```

Minimal `src/error.rs` (extended in Task 3):

```rust
use crate::kind::CoreKind;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("core kind `{0}` has no launch profile yet")]
    UnsupportedCore(CoreKind),
}
```

- [ ] **Step 3: Write `src/kind.rs` with failing-by-absence tests included**

```rust
//! Core kinds, launch profiles, and config checking.

use std::ffi::OsString;

use camino::Utf8Path;

use crate::error::Error;

/// The environment variable Mihomo consults for permitted file-system roots.
pub const MIHOMO_SAFE_PATHS_ENV_NAME: &str = "SAFE_PATHS";

#[cfg(windows)]
const SAFE_PATHS_SEPARATOR: &str = ";";
#[cfg(not(windows))]
const SAFE_PATHS_SEPARATOR: &str = ":";

/// A core family. Build variants (alpha builds, custom binaries) are expressed
/// through `CoreSpec::binary_path` and metadata, not extra kinds.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreKind {
    Mihomo,
    ClashPremium,
    ClashRs,
    /// Declared for a future core; has no launch profile yet.
    Meow,
}

impl AsRef<str> for CoreKind {
    fn as_ref(&self) -> &str {
        match self {
            CoreKind::Mihomo => "mihomo",
            CoreKind::ClashPremium => "clash",
            CoreKind::ClashRs => "clash-rs",
            CoreKind::Meow => "meow",
        }
    }
}

impl std::fmt::Display for CoreKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl CoreKind {
    /// Launch arguments for this kind. `Meow` has no launch profile yet.
    pub(crate) fn run_args(
        &self,
        working_dir: &Utf8Path,
        config_path: &Utf8Path,
    ) -> Result<Vec<OsString>, Error> {
        let dir = OsString::from(working_dir.as_str());
        let cfg = OsString::from(config_path.as_str());
        Ok(match self {
            CoreKind::Mihomo => vec!["-m".into(), "-d".into(), dir, "-f".into(), cfg],
            CoreKind::ClashRs => vec!["-d".into(), dir, "-c".into(), cfg],
            CoreKind::ClashPremium => vec!["-d".into(), dir, "-f".into(), cfg],
            CoreKind::Meow => return Err(Error::UnsupportedCore(*self)),
        })
    }

    /// Arguments for a one-shot `-t` config validation run (same for all kinds,
    /// matching the legacy `check_config_`).
    pub(crate) fn check_args(working_dir: &Utf8Path, config_path: &Utf8Path) -> Vec<OsString> {
        vec![
            "-t".into(),
            "-d".into(),
            working_dir.as_str().into(),
            "-f".into(),
            config_path.as_str().into(),
        ]
    }
}

/// Joins the directories Mihomo may touch into its `SAFE_PATHS` format.
pub fn mihomo_safe_paths(working_dir: &Utf8Path, config_dir: &Utf8Path) -> String {
    [working_dir.as_str(), config_dir.as_str()].join(SAFE_PATHS_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    #[test]
    fn run_args_match_legacy_profiles() {
        let dir = Utf8PathBuf::from("C:/data");
        let cfg = Utf8PathBuf::from("C:/data/config.yaml");
        let args = CoreKind::Mihomo.run_args(&dir, &cfg).unwrap();
        assert_eq!(args, ["-m", "-d", "C:/data", "-f", "C:/data/config.yaml"].map(OsString::from));
        let args = CoreKind::ClashRs.run_args(&dir, &cfg).unwrap();
        assert_eq!(args, ["-d", "C:/data", "-c", "C:/data/config.yaml"].map(OsString::from));
        let args = CoreKind::ClashPremium.run_args(&dir, &cfg).unwrap();
        assert_eq!(args, ["-d", "C:/data", "-f", "C:/data/config.yaml"].map(OsString::from));
    }

    #[test]
    fn meow_has_no_launch_profile() {
        let dir = Utf8PathBuf::from("/d");
        assert!(matches!(
            CoreKind::Meow.run_args(&dir, &dir),
            Err(Error::UnsupportedCore(CoreKind::Meow))
        ));
    }

    #[test]
    fn safe_paths_joins_with_platform_separator() {
        let joined = mihomo_safe_paths(Utf8Path::new("/a"), Utf8Path::new("/b"));
        #[cfg(windows)]
        assert_eq!(joined, "/a;/b");
        #[cfg(not(windows))]
        assert_eq!(joined, "/a:/b");
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS (3 tests). If `serde_yaml_ng = "0.10"` does not resolve, use the latest published version (`cargo add serde_yaml_ng -p nyanpasu-core-manager` reports it) and keep everything else unchanged.

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add core kinds and launch profiles"
```

### Task 2: `parse_check_output` migration + `error_summary`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/kind.rs`

**Interfaces:**
- Produces: `pub(crate) fn parse_check_output(log: String) -> String` (verbatim behavioral port of `nyanpasu-utils/src/core/utils.rs:16`), `pub(crate) fn error_summary(kind: CoreKind, stderr_tail: &str) -> String` (Mihomo: parse the last `level=error` line; otherwise return the tail unchanged).

- [ ] **Step 1: Append the failing tests to `kind.rs`'s test module**

```rust
    #[test]
    fn parse_check_output_extracts_mihomo_msg() {
        let log = r#"time="2026-07-18T10:00:00Z" level=error msg="configuration file /x.yaml test failed""#;
        assert_eq!(
            parse_check_output(log.to_string()),
            "configuration file /x.yaml test failed"
        );
    }

    #[test]
    fn parse_check_output_extracts_error_field() {
        assert_eq!(parse_check_output("error=bad path=/etc".to_string()), "bad");
    }

    #[test]
    fn parse_check_output_falls_back_to_input() {
        assert_eq!(parse_check_output("plain failure".to_string()), "plain failure");
    }

    #[test]
    fn error_summary_parses_last_mihomo_error_line() {
        let tail = "line one\ntime=\"x\" level=error msg=\"boom\"\nafter";
        assert_eq!(error_summary(CoreKind::Mihomo, tail), "boom");
        assert_eq!(error_summary(CoreKind::ClashRs, tail), tail);
        assert_eq!(error_summary(CoreKind::Mihomo, "no marker"), "no marker");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager parse_check`
Expected: FAIL — `parse_check_output` not found.

- [ ] **Step 3: Implement (port the legacy algorithm unchanged, add `error_summary`)**

```rust
/// Extracts the human-readable message from a Mihomo error log line.
/// Behavioral port of the legacy `core::utils::parse_check_output`.
pub(crate) fn parse_check_output(log: String) -> String {
    let t = log.find("time=");
    let m = log.find("msg=");
    let mr = log.rfind('"');

    if let (Some(_), Some(m), Some(mr)) = (t, m, mr) {
        let e = match log.find("level=error msg=") {
            Some(e) => e + 17,
            None => m + 5,
        };

        if mr > m {
            return log[e..mr].to_owned();
        }
    }

    let l = log.find("error=");
    let r = log.find("path=").or(Some(log.len()));

    if let (Some(l), Some(r)) = (l, r) {
        return log[(l + 6)..(r - 1)].to_owned();
    }

    log
}

/// Condenses a stderr tail into an error message. Mihomo logs are structured,
/// so the last `level=error` line carries the actual cause.
pub(crate) fn error_summary(kind: CoreKind, stderr_tail: &str) -> String {
    if matches!(kind, CoreKind::Mihomo)
        && let Some(line) = stderr_tail.lines().rev().find(|l| l.contains("level=error"))
    {
        return parse_check_output(line.to_string());
    }
    stderr_tail.to_owned()
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager/src/kind.rs
git commit -m "feat(nyanpasu-core-manager): migrate parse_check_output and add error_summary"
```

### Task 3: `error.rs`, `state.rs`, `spec.rs`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/error.rs`
- Create: `crates/nyanpasu-core-manager/src/state.rs`
- Create: `crates/nyanpasu-core-manager/src/spec.rs`
- Modify: `crates/nyanpasu-core-manager/src/lib.rs`

**Interfaces:**
- Produces (used by every later task):
  - `Error` variants: `AlreadyRunning`, `NotStarted`, `ConfigNotFound(Utf8PathBuf)`, `BinaryNotFound(Utf8PathBuf)`, `ControllerMissing`, `UnsupportedCore(CoreKind)`, `ConfigCheckFailed(String)`, `StartupTimeout { stderr_tail: String }`, `StartupFailed { stderr_tail: String }`, `Process(ProcessError)`, `Api(clash_api::Error)`, `Yaml(serde_yaml_ng::Error)`, `Io(std::io::Error)`.
  - `InstanceState { Starting, Running { pid: u32 }, Restarting { attempt: u32 }, Stopping, Stopped(StopReason) }` + `fn is_terminal(&self) -> bool`; `StopReason { Finished, User, Error(String) }`.
  - `CoreState { Stopped { reason: Option<StopReason> }, Starting { epoch: u64 }, Running { epoch: u64, pid: u32 }, Restarting { epoch: u64, attempt: u32 }, Switching { from: Option<u64>, to: u64 }, Stopping { epoch: u64 } }` (`#[non_exhaustive]`).
  - `CoreStatus { state: CoreState, changed_at: i64, spec: Option<SpecSummary>, controller: Option<clash_api::Host> }` + `CoreStatus::initial()`; `SpecSummary { kind: CoreKind, config_path: Utf8PathBuf }`; `pub(crate) fn now_ms() -> i64`.
  - `CoreSpec { kind, binary_path: Utf8PathBuf, version: Option<String>, features: Vec<String> }`; `InstanceSpec { core: CoreSpec, config_path, working_dir, pid_file: Option<Utf8PathBuf>, options: InstanceOptions }`; `InstanceOptions { startup_timeout: Duration, probe_interval: Duration, restart_policy: RestartPolicy, backoff: Backoff }` with `Default` per Global Constraints; `ResolvedController { host: clash_api::Host, secret: Option<String> }`; `ControllerMode::Passthrough` (`#[non_exhaustive]` enum, `Managed` arrives in Task 18); `ManagerOptions { controller_mode: ControllerMode, cancel_token: CancellationToken }` with `Default`.

- [ ] **Step 1: Write the failing test (defaults contract) in `spec.rs`'s test module, then the three files**

`src/error.rs`:

```rust
use camino::Utf8PathBuf;

use crate::kind::CoreKind;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("core is already running")]
    AlreadyRunning,
    #[error("core is not running")]
    NotStarted,
    #[error("config file not found: {0}")]
    ConfigNotFound(Utf8PathBuf),
    #[error("core binary not found: {0}")]
    BinaryNotFound(Utf8PathBuf),
    #[error("no external controller configured; the version health probe needs one")]
    ControllerMissing,
    #[error("core kind `{0}` has no launch profile yet")]
    UnsupportedCore(CoreKind),
    #[error("config check failed: {0}")]
    ConfigCheckFailed(String),
    #[error("core did not become healthy before the startup timeout; stderr tail:\n{stderr_tail}")]
    StartupTimeout { stderr_tail: String },
    #[error("core failed to start; stderr tail:\n{stderr_tail}")]
    StartupFailed { stderr_tail: String },
    #[error(transparent)]
    Process(#[from] nyanpasu_utils::process::ProcessError),
    #[error(transparent)]
    Api(#[from] clash_api::Error),
    #[error("failed to process config YAML: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

`src/state.rs`:

```rust
//! Instance and manager state machines and the published status snapshot.

use camino::Utf8PathBuf;

use crate::kind::CoreKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceState {
    /// Spawned; the version health probe has not passed yet.
    Starting,
    Running { pid: u32 },
    /// Crashed; the supervisor is backing off, respawning, or re-probing.
    Restarting { attempt: u32 },
    Stopping,
    Stopped(StopReason),
}

impl InstanceState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The process exited cleanly (code 0); the supervisor does not restart it.
    Finished,
    User,
    Error(String),
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreState {
    Stopped { reason: Option<StopReason> },
    Starting { epoch: u64 },
    Running { epoch: u64, pid: u32 },
    Restarting { epoch: u64, attempt: u32 },
    /// A hard or graceful switch is in flight.
    Switching { from: Option<u64>, to: u64 },
    Stopping { epoch: u64 },
}

#[derive(Debug, Clone)]
pub struct SpecSummary {
    pub kind: CoreKind,
    pub config_path: Utf8PathBuf,
}

/// Snapshot published on the manager's watch channel.
#[derive(Debug, Clone)]
pub struct CoreStatus {
    pub state: CoreState,
    /// Unix milliseconds of the last state transition (feeds IPC `state_changed_at`).
    pub changed_at: i64,
    pub spec: Option<SpecSummary>,
    /// The managed controller endpoint, when `ControllerMode::Managed` is active.
    pub controller: Option<clash_api::Host>,
}

impl CoreStatus {
    pub(crate) fn initial() -> Self {
        Self {
            state: CoreState::Stopped { reason: None },
            changed_at: now_ms(),
            spec: None,
            controller: None,
        }
    }
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
```

`src/spec.rs`:

```rust
//! Immutable launch specifications and manager options.

use std::time::Duration;

use camino::Utf8PathBuf;
use nyanpasu_utils::process::{Backoff, RestartPolicy};
use tokio_util::sync::CancellationToken;

use crate::kind::CoreKind;

#[derive(Debug, Clone)]
pub struct CoreSpec {
    pub kind: CoreKind,
    /// Resolved by the caller (the service keeps `find_binary_path`).
    pub binary_path: Utf8PathBuf,
    /// Display metadata provided by the caller; not interpreted here.
    pub version: Option<String>,
    pub features: Vec<String>,
}

/// Immutable per-epoch launch spec. Changing the config means a new epoch.
#[derive(Debug, Clone)]
pub struct InstanceSpec {
    pub core: CoreSpec,
    pub config_path: Utf8PathBuf,
    pub working_dir: Utf8PathBuf,
    pub pid_file: Option<Utf8PathBuf>,
    pub options: InstanceOptions,
}

#[derive(Debug, Clone)]
pub struct InstanceOptions {
    /// Total limit for the initial start (spawn → version probe success).
    pub startup_timeout: Duration,
    pub probe_interval: Duration,
    pub restart_policy: RestartPolicy,
    pub backoff: Backoff,
}

impl Default for InstanceOptions {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(30),
            probe_interval: Duration::from_millis(250),
            restart_policy: RestartPolicy::OnFailure { max_restarts: 5 },
            backoff: Backoff::exponential(Duration::from_secs(1), Duration::from_secs(30))
                .with_jitter(),
        }
    }
}

/// The probe/control endpoint an instance actually uses.
#[derive(Debug, Clone)]
pub struct ResolvedController {
    pub host: clash_api::Host,
    pub secret: Option<String>,
}

/// How the manager learns and controls the core's external controller.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub enum ControllerMode {
    /// Start the config as-is; extract the probe endpoint from it.
    #[default]
    Passthrough,
}

#[derive(Debug, Clone, Default)]
pub struct ManagerOptions {
    pub controller_mode: ControllerMode,
    pub cancel_token: CancellationToken,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_options_defaults_match_spec() {
        let o = InstanceOptions::default();
        assert_eq!(o.startup_timeout, Duration::from_secs(30));
        assert_eq!(o.probe_interval, Duration::from_millis(250));
        assert_eq!(o.restart_policy, RestartPolicy::OnFailure { max_restarts: 5 });
    }
}
```

`src/lib.rs` module list becomes:

```rust
mod config;
mod error;
mod health;
pub mod instance;
pub mod kind;
pub mod spec;
pub mod state;

pub use clash_api::Host;
pub use error::Error;
pub use kind::CoreKind;
pub use spec::{ControllerMode, CoreSpec, InstanceOptions, InstanceSpec, ManagerOptions, ResolvedController};
pub use state::{CoreState, CoreStatus, InstanceState, SpecSummary, StopReason};
```

(`config`/`health` don't exist yet — create empty placeholder files `src/config.rs` and `src/health.rs` containing only a module doc comment, filled by Tasks 4 and 7.)

- [ ] **Step 2: Run tests**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS (8 tests).

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add error, state, and spec types"
```

### Task 4: `config.rs` introspection + controller resolution

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/config.rs`

**Interfaces:**
- Produces:
  - `pub(crate) struct ConfigInfo { pub controller: Option<RawController>, pub secret: Option<String>, pub has_dns_listen: bool }`
  - `pub(crate) enum RawController { Pipe(String), Unix(String), Http(String) }`
  - `pub(crate) async fn inspect(config_path: &Utf8Path) -> Result<ConfigInfo, Error>` (reads + parses YAML)
  - `pub(crate) fn resolve_controller(info: &ConfigInfo) -> Result<ResolvedController, Error>` (`Err(Error::ControllerMissing)` when absent — the strict policy)
- Consumes: `Error`, `ResolvedController` from Task 3.

- [ ] **Step 1: Write the failing unit tests in `config.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn info_of(yaml: &str) -> ConfigInfo {
        let doc: serde_yaml_ng::Mapping = serde_yaml_ng::from_str(yaml).unwrap();
        inspect_mapping(&doc)
    }

    #[test]
    fn extracts_http_controller_and_secret() {
        let info = info_of("external-controller: 127.0.0.1:9090\nsecret: s3cret\n");
        assert_eq!(info.controller, Some(RawController::Http("127.0.0.1:9090".into())));
        assert_eq!(info.secret.as_deref(), Some("s3cret"));
        assert!(!info.has_dns_listen);
    }

    #[test]
    fn local_transport_takes_priority_over_http() {
        #[cfg(windows)]
        {
            let info = info_of(
                "external-controller: 127.0.0.1:9090\nexternal-controller-pipe: \\\\.\\pipe\\x\n",
            );
            assert_eq!(info.controller, Some(RawController::Pipe(r"\\.\pipe\x".into())));
        }
        #[cfg(unix)]
        {
            let info = info_of(
                "external-controller: 127.0.0.1:9090\nexternal-controller-unix: /run/x.sock\n",
            );
            assert_eq!(info.controller, Some(RawController::Unix("/run/x.sock".into())));
        }
    }

    #[test]
    fn detects_dns_listen() {
        let info = info_of("dns:\n  listen: 0.0.0.0:53\n");
        assert!(info.has_dns_listen);
    }

    #[test]
    fn missing_controller_is_a_strict_error() {
        let info = info_of("mixed-port: 7890\n");
        assert!(matches!(
            resolve_controller(&info),
            Err(crate::Error::ControllerMissing)
        ));
    }

    #[test]
    fn wildcard_bind_addresses_probe_via_loopback() {
        assert_eq!(probe_address("0.0.0.0:9090"), "127.0.0.1:9090");
        assert_eq!(probe_address(":9090"), "127.0.0.1:9090");
        assert_eq!(probe_address("[::]:9090"), "127.0.0.1:9090");
        assert_eq!(probe_address("127.0.0.1:9090"), "127.0.0.1:9090");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager config`
Expected: FAIL — items not found.

- [ ] **Step 3: Implement**

```rust
//! Runtime-config introspection (controller endpoint, secret, DNS listener).

use camino::Utf8Path;
use serde_yaml_ng::{Mapping, Value};

use crate::{error::Error, spec::ResolvedController};

#[derive(Debug)]
pub(crate) struct ConfigInfo {
    pub controller: Option<RawController>,
    pub secret: Option<String>,
    pub has_dns_listen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawController {
    Pipe(String),
    Unix(String),
    Http(String),
}

fn str_value(doc: &Mapping, key: &str) -> Option<String> {
    doc.get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|s| !s.is_empty())
}

pub(crate) async fn inspect(config_path: &Utf8Path) -> Result<ConfigInfo, Error> {
    let raw = tokio::fs::read_to_string(config_path).await?;
    let doc: Mapping = serde_yaml_ng::from_str(&raw)?;
    Ok(inspect_mapping(&doc))
}

fn inspect_mapping(doc: &Mapping) -> ConfigInfo {
    #[cfg(windows)]
    let local = str_value(doc, "external-controller-pipe").map(RawController::Pipe);
    #[cfg(not(windows))]
    let local = str_value(doc, "external-controller-unix").map(RawController::Unix);

    let controller = local.or_else(|| str_value(doc, "external-controller").map(RawController::Http));

    let has_dns_listen = doc
        .get(Value::String("dns".to_owned()))
        .and_then(Value::as_mapping)
        .and_then(|dns| dns.get(Value::String("listen".to_owned())))
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());

    ConfigInfo {
        controller,
        secret: str_value(doc, "secret"),
        has_dns_listen,
    }
}

pub(crate) fn resolve_controller(info: &ConfigInfo) -> Result<ResolvedController, Error> {
    let raw = info.controller.as_ref().ok_or(Error::ControllerMissing)?;
    let host = match raw {
        RawController::Pipe(path) => clash_api::Host::named_pipe(path),
        RawController::Unix(path) => clash_api::Host::unix_socket(path),
        RawController::Http(addr) => clash_api::Host::http(probe_address(addr))?,
    };
    Ok(ResolvedController {
        host,
        secret: info.secret.clone(),
    })
}

/// `0.0.0.0:9090`, `:9090`, and `[::]:9090` are bind addresses — probe loopback.
fn probe_address(addr: &str) -> String {
    match addr.rsplit_once(':') {
        Some(("0.0.0.0" | "::" | "[::]" | "", port)) => format!("127.0.0.1:{port}"),
        _ => addr.to_owned(),
    }
}
```

If `serde_yaml_ng`'s `Mapping::get` wants `&Value` instead of `Value`, adjust `str_value` accordingly (`doc.get(&Value::String(...))`) — keep the tests unchanged.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS (13 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager/src/config.rs
git commit -m "feat(nyanpasu-core-manager): add runtime-config introspection and controller resolution"
```

# M2 — Instance

### Task 5: `fake_core` test helper binary (TCP transport) + test utilities

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/helpers/fake_core.rs` (replace the placeholder)
- Create: `crates/nyanpasu-core-manager/tests/common/mod.rs`
- Create: `crates/nyanpasu-core-manager/tests/fake_core_smoke.rs`

**Interfaces:**
- Produces: the `nyanpasu-fake-core` binary — a scripted mihomo simulator. CLI mirrors mihomo (`[-m] [-t] -d <dir> -f <config>`). Behavior keys under `x-fake-core:` in the config YAML:
  - `ready-delay-ms: u64` — `/version` returns 503 until this delay passes (default 0)
  - `never-ready: true` — `/version` always 503
  - `exit-code: i64` + `stderr-lines: [..]` — print lines to stderr, exit immediately
  - `crash-after-ms: u64` + `crash-times: u64` + `state-file: <path>` — exit(1) after the delay, but only for the first `crash-times` runs (counter persisted in `state-file`)
  - `patch-log: <path>` — append every raw `PATCH /configs` body as one line
  - `reject-patch: true` — `PATCH /configs` returns 500
  - `check-fail: <msg>` — with `-t`: print `time="t" level=error msg="<msg>"` to stdout, exit 1
  - Top-level `mixed-port` (non-zero) is bound on 127.0.0.1 at startup; a PATCH body containing `"mixed-port":N` rebinds it (500 if the bind fails).
  - HTTP transport with a top-level `secret` requires `Authorization: Bearer <secret>` (else 401).
- Produces (test utils in `tests/common/mod.rs`): `fake_core_bin() -> Utf8PathBuf`, `free_port() -> u16`, `fast_options() -> InstanceOptions`, `write_config(dir: &Utf8Path, body: &str) -> Utf8PathBuf`, `mihomo_spec(dir: &Utf8Path, config: Utf8PathBuf) -> InstanceSpec`, `async fn wait_for_state(rx, pred, timeout) -> InstanceState`, `record_states(rx) -> (JoinHandle, Arc<Mutex<Vec<InstanceState>>>)`, `async fn wait_port_refused(port: u16)`.

- [ ] **Step 1: Write `tests/helpers/fake_core.rs`**

```rust
//! Scripted mihomo simulator for nyanpasu-core-manager tests. Not production code.
//! CLI mirrors mihomo: `[-m] [-t] -d <dir> -f <config>`. See the implementation
//! plan for the `x-fake-core` behavior keys.
#![allow(dead_code)] // several Behavior fields are platform-conditional

use std::{
    process::exit,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use serde_yaml_ng::{Mapping, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Behavior {
    external_controller: Option<String>,
    external_controller_pipe: Option<String>,
    external_controller_unix: Option<String>,
    secret: Option<String>,
    mixed_port: u16,
    ready_delay_ms: u64,
    never_ready: bool,
    exit_code: Option<i32>,
    stderr_lines: Vec<String>,
    crash_after_ms: u64,
    crash_times: u64,
    state_file: Option<String>,
    patch_log: Option<String>,
    reject_patch: bool,
    check_fail: Option<String>,
}

fn s(doc: &Mapping, key: &str) -> Option<String> {
    doc.get(Value::String(key.into())).and_then(Value::as_str).map(str::to_owned)
}
fn u(doc: &Mapping, key: &str) -> u64 {
    doc.get(Value::String(key.into())).and_then(Value::as_u64).unwrap_or(0)
}
fn b(doc: &Mapping, key: &str) -> bool {
    doc.get(Value::String(key.into())).and_then(Value::as_bool).unwrap_or(false)
}

fn parse(config: &str) -> Behavior {
    let doc: Mapping = serde_yaml_ng::from_str(config).expect("valid yaml");
    let x = doc
        .get(Value::String("x-fake-core".into()))
        .and_then(Value::as_mapping)
        .cloned()
        .unwrap_or_default();
    Behavior {
        external_controller: s(&doc, "external-controller"),
        external_controller_pipe: s(&doc, "external-controller-pipe"),
        external_controller_unix: s(&doc, "external-controller-unix"),
        secret: s(&doc, "secret"),
        mixed_port: u(&doc, "mixed-port") as u16,
        ready_delay_ms: u(&x, "ready-delay-ms"),
        never_ready: b(&x, "never-ready"),
        exit_code: x
            .get(Value::String("exit-code".into()))
            .and_then(Value::as_i64)
            .map(|c| c as i32),
        stderr_lines: x
            .get(Value::String("stderr-lines".into()))
            .and_then(Value::as_sequence)
            .map(|seq| {
                seq.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        crash_after_ms: u(&x, "crash-after-ms"),
        crash_times: u(&x, "crash-times"),
        state_file: s(&x, "state-file"),
        patch_log: s(&x, "patch-log"),
        reject_patch: b(&x, "reject-patch"),
        check_fail: s(&x, "check-fail"),
    }
}

struct Ctx {
    ready: AtomicBool,
    behavior: Behavior,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check_mode = args.iter().any(|a| a == "-t");
    let config_path = args
        .iter()
        .position(|a| a == "-f")
        .and_then(|i| args.get(i + 1))
        .expect("-f <config> required");
    let config = std::fs::read_to_string(config_path).expect("readable config");
    let behavior = parse(&config);

    if check_mode {
        match &behavior.check_fail {
            Some(msg) => {
                println!("time=\"t\" level=error msg=\"{msg}\"");
                exit(1);
            }
            None => exit(0),
        }
    }

    for line in &behavior.stderr_lines {
        eprintln!("{line}");
    }
    if let Some(code) = behavior.exit_code {
        exit(code);
    }

    // Crash script: only the first `crash-times` runs crash.
    if behavior.crash_after_ms > 0 {
        let state_file = behavior.state_file.clone().expect("crash needs state-file");
        let count: u64 = std::fs::read_to_string(&state_file)
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(0);
        if count < behavior.crash_times {
            std::fs::write(&state_file, (count + 1).to_string()).expect("write state file");
            let delay = behavior.crash_after_ms;
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                exit(1);
            });
        }
    }

    if behavior.mixed_port != 0 {
        let listener = TcpListener::bind(("127.0.0.1", behavior.mixed_port))
            .await
            .expect("bind mixed-port");
        hold_listener(listener);
    }

    let ctx = Arc::new(Ctx {
        ready: AtomicBool::new(false),
        behavior,
    });
    if !ctx.behavior.never_ready {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ctx.behavior.ready_delay_ms)).await;
            ctx.ready.store(true, Ordering::SeqCst);
        });
    }

    let mut served = false;
    if let Some(addr) = ctx.behavior.external_controller.clone() {
        let listener = TcpListener::bind(&addr).await.expect("bind controller");
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { continue };
                let ctx = ctx.clone();
                tokio::spawn(async move { serve_conn(stream, ctx, true).await });
            }
        });
        served = true;
    }
    served |= serve_local_transports(&ctx);
    if !served {
        eprintln!("fake-core: no controller configured");
    }

    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

/// Accept-and-hold so the port stays bound (simulates a proxy listener).
fn hold_listener(listener: TcpListener) {
    tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
}

/// Named-pipe / unix-socket transports land in a later task (M4).
fn serve_local_transports(_ctx: &Arc<Ctx>) -> bool {
    false
}

async fn serve_conn<S>(mut stream: S, ctx: Arc<Ctx>, http_transport: bool)
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buf = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 1024];
        let Ok(n) = stream.read(&mut chunk).await else { return };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 64 * 1024 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();

    let mut content_length = 0usize;
    let mut authorization = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else { continue };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => content_length = value.trim().parse().unwrap_or(0),
            "authorization" => authorization = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let mut chunk = [0u8; 1024];
        let Ok(n) = stream.read(&mut chunk).await else { return };
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&body).into_owned();

    if http_transport
        && let Some(secret) = &ctx.behavior.secret
        && authorization.as_deref() != Some(&format!("Bearer {secret}"))
    {
        respond(&mut stream, 401, r#"{"message":"Unauthorized"}"#).await;
        return;
    }

    match (method.as_str(), path.as_str()) {
        ("GET", "/version") => {
            if ctx.ready.load(Ordering::SeqCst) {
                respond(&mut stream, 200, r#"{"meta":true,"version":"fake-core"}"#).await;
            } else {
                respond(&mut stream, 503, r#"{"message":"starting"}"#).await;
            }
        }
        ("PATCH", "/configs") => {
            if ctx.behavior.reject_patch {
                respond(&mut stream, 500, r#"{"message":"patch rejected"}"#).await;
                return;
            }
            if let Some(log) = &ctx.behavior.patch_log {
                let mut existing = std::fs::read_to_string(log).unwrap_or_default();
                existing.push_str(&body);
                existing.push('\n');
                let _ = std::fs::write(log, existing);
            }
            if let Some(port) = extract_mixed_port(&body) {
                match TcpListener::bind(("127.0.0.1", port)).await {
                    Ok(listener) => hold_listener(listener),
                    Err(_) => {
                        respond(&mut stream, 500, r#"{"message":"bind failed"}"#).await;
                        return;
                    }
                }
            }
            respond(&mut stream, 204, "").await;
        }
        _ => respond(&mut stream, 404, r#"{"message":"not found"}"#).await,
    }
}

fn extract_mixed_port(body: &str) -> Option<u16> {
    let idx = body.find("\"mixed-port\":")?;
    let digits: String = body[idx + 13..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

async fn respond<S: AsyncWriteExt + Unpin>(stream: &mut S, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        401 => "Unauthorized",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}
```

- [ ] **Step 2: Write `tests/common/mod.rs`**

```rust
//! Shared utilities for nyanpasu-core-manager integration tests.
#![allow(dead_code)]

use std::{sync::Arc, time::Duration};

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    CoreKind, CoreSpec, InstanceOptions, InstanceSpec, state::InstanceState,
};
use nyanpasu_utils::process::{Backoff, RestartPolicy};
use parking_lot::Mutex;
use tokio::sync::watch;

pub fn fake_core_bin() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_BIN_EXE_nyanpasu-fake-core"))
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// Small budgets so failure paths finish in test time.
pub fn fast_options() -> InstanceOptions {
    InstanceOptions {
        startup_timeout: Duration::from_secs(5),
        probe_interval: Duration::from_millis(50),
        restart_policy: RestartPolicy::OnFailure { max_restarts: 2 },
        backoff: Backoff::exponential(Duration::from_millis(50), Duration::from_millis(200)),
    }
}

pub fn write_config(dir: &Utf8Path, body: &str) -> Utf8PathBuf {
    let path = dir.join("config.yaml");
    std::fs::write(&path, body).expect("write config");
    path
}

pub fn mihomo_spec(dir: &Utf8Path, config_path: Utf8PathBuf) -> InstanceSpec {
    InstanceSpec {
        core: CoreSpec {
            kind: CoreKind::Mihomo,
            binary_path: fake_core_bin(),
            version: None,
            features: Vec::new(),
        },
        config_path,
        working_dir: dir.to_owned(),
        pid_file: None,
        options: fast_options(),
    }
}

pub fn utf8_tempdir() -> (tempfile::TempDir, Utf8PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf8 tempdir");
    (dir, path)
}

pub async fn wait_for_state(
    rx: &mut watch::Receiver<InstanceState>,
    pred: impl Fn(&InstanceState) -> bool,
    timeout: Duration,
) -> InstanceState {
    tokio::time::timeout(timeout, async {
        loop {
            let current = rx.borrow_and_update().clone();
            if pred(&current) {
                return current;
            }
            if rx.changed().await.is_err() {
                panic!("state channel closed while waiting");
            }
        }
    })
    .await
    .expect("timed out waiting for state")
}

/// Records every state transition for later sequence assertions.
pub fn record_states(
    mut rx: watch::Receiver<InstanceState>,
) -> (tokio::task::JoinHandle<()>, Arc<Mutex<Vec<InstanceState>>>) {
    let log = Arc::new(Mutex::new(vec![rx.borrow().clone()]));
    let log_ = log.clone();
    let handle = tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            log_.lock().push(rx.borrow().clone());
        }
    });
    (handle, log)
}

/// Asserts the process behind `port` is gone by polling until connect is refused.
pub async fn wait_port_refused(port: u16) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("port was never released");
}
```

- [ ] **Step 3: Write the smoke test `tests/fake_core_smoke.rs`**

```rust
mod common;

use std::time::Duration;

use clash_api::{Client, ConfigPatch};

#[tokio::test]
async fn fake_core_serves_version_and_records_patches() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let patch_log = dir.join("patch.log");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  ready-delay-ms: 200\n  patch-log: {patch_log}\n"
        ),
    );

    let mut child = tokio::process::Command::new(common::fake_core_bin())
        .args(["-m", "-d", dir.as_str(), "-f", config.as_str()])
        .kill_on_drop(true)
        .spawn()
        .expect("spawn fake core");

    let client = Client::new_http(format!("127.0.0.1:{port}")).unwrap();
    // Not ready yet → probe fails; ready after the delay → succeeds.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(client.version().await.is_err(), "must be 503 before ready");
    tokio::time::timeout(Duration::from_secs(5), async {
        while client.version().await.is_err() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("fake core never became ready");

    client
        .patch_config(&ConfigPatch { mixed_port: Some(0), ..Default::default() })
        .await
        .expect("patch accepted");
    let log = std::fs::read_to_string(&patch_log).expect("patch log written");
    assert!(log.contains("\"mixed-port\":0"), "log was: {log}");

    child.kill().await.ok();
}
```

- [ ] **Step 4: Run**

Run: `cargo test -p nyanpasu-core-manager --test fake_core_smoke`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests
git commit -m "test(nyanpasu-core-manager): add the fake-core simulator and shared test utilities"
```

### Task 6: `check_config`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/kind.rs`
- Create: `crates/nyanpasu-core-manager/tests/check_config.rs`

**Interfaces:**
- Produces: `pub async fn check_config(spec: &InstanceSpec) -> Result<(), Error>` in `kind.rs` (one-shot `-t` run via `nyanpasu_utils::process::Command::output()`, `SAFE_PATHS` env set, mihomo output condensed via `parse_check_output`, ClashRs output passed through as `stdout\nstderr`).

- [ ] **Step 1: Write the failing test `tests/check_config.rs`**

```rust
mod common;

use nyanpasu_core_manager::{Error, kind::check_config};

#[tokio::test]
async fn check_config_passes_and_fails() {
    let (_guard, dir) = common::utf8_tempdir();
    let ok_config = common::write_config(&dir, "mixed-port: 7890\n");
    let spec = common::mihomo_spec(&dir, ok_config);
    check_config(&spec).await.expect("valid config passes");

    let bad_config = dir.join("bad.yaml");
    std::fs::write(&bad_config, "x-fake-core:\n  check-fail: port already in use\n").unwrap();
    let mut bad_spec = common::mihomo_spec(&dir, bad_config);
    bad_spec.config_path = dir.join("bad.yaml");
    let err = check_config(&bad_spec).await.expect_err("must fail");
    match err {
        Error::ConfigCheckFailed(msg) => assert_eq!(msg, "port already in use"),
        other => panic!("unexpected error: {other}"),
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager --test check_config`
Expected: FAIL — `check_config` not found.

- [ ] **Step 3: Implement in `kind.rs`**

```rust
/// One-shot `-t` config validation, replacing the legacy `check_config_`.
/// A non-zero exit becomes [`Error::ConfigCheckFailed`] with a condensed message.
pub async fn check_config(spec: &crate::spec::InstanceSpec) -> Result<(), Error> {
    if matches!(spec.core.kind, CoreKind::Meow) {
        return Err(Error::UnsupportedCore(spec.core.kind));
    }
    let config_dir = spec
        .config_path
        .parent()
        .ok_or_else(|| Error::ConfigNotFound(spec.config_path.clone()))?;
    let output = nyanpasu_utils::process::Command::new(spec.core.binary_path.as_str())
        .args(CoreKind::check_args(&spec.working_dir, &spec.config_path))
        .env(
            MIHOMO_SAFE_PATHS_ENV_NAME,
            mihomo_safe_paths(&spec.working_dir, config_dir),
        )
        .output()
        .await?;
    if output.success() {
        return Ok(());
    }
    let message = match spec.core.kind {
        CoreKind::ClashRs => format!("{}\n{}", output.stdout, output.stderr),
        _ => parse_check_output(output.stdout.trim().to_owned()),
    };
    Err(Error::ConfigCheckFailed(message))
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager --test check_config`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add one-shot config checking"
```

### Task 7: `health.rs`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/health.rs`
- Create: `crates/nyanpasu-core-manager/tests/health_probe.rs`

**Interfaces:**
- Produces: `pub(crate) fn build_client(controller: &ResolvedController) -> Result<clash_api::Client, Error>` (1s per-request timeout, `NoRetry` — the probe loop owns retrying), `pub(crate) struct HealthCheck` with `fn new(&ResolvedController) -> Result<Self, Error>` and `async fn probe_once(&self) -> bool`.
- Note: `pub(crate)` items are not reachable from integration tests — mark the module `pub` in `lib.rs` under `#[doc(hidden)]` is NOT wanted; instead expose a tiny test window: `HealthCheck` stays crate-private and the integration test exercises it indirectly through `Instance` in Task 8. For THIS task, test `build_client`'s observable behavior through a `#[cfg(test)]` unit test that spins a real TCP listener inside the crate.

- [ ] **Step 1: Write `src/health.rs` with its unit test**

```rust
//! Version-probe health checking against the core's external controller.

use std::time::Duration;

use crate::{error::Error, spec::ResolvedController};

/// Builds a probe/control client: 1s per-request timeout so one hung request
/// can never eat the whole startup deadline; retrying is the caller's loop.
pub(crate) fn build_client(controller: &ResolvedController) -> Result<clash_api::Client, Error> {
    let mut builder = clash_api::Client::builder(controller.host.clone())
        .configure_reqwest(|b| b.timeout(Duration::from_secs(1)));
    if let Some(secret) = &controller.secret {
        builder = builder.secret(secret.as_str());
    }
    Ok(builder.build()?)
}

pub(crate) struct HealthCheck {
    client: clash_api::Client,
}

impl HealthCheck {
    pub(crate) fn new(controller: &ResolvedController) -> Result<Self, Error> {
        Ok(Self { client: build_client(controller)? })
    }

    /// One probe attempt: healthy iff `GET /version` succeeds.
    pub(crate) async fn probe_once(&self) -> bool {
        self.client.version().await.is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(port: u16) -> ResolvedController {
        ResolvedController {
            host: clash_api::Host::http(format!("127.0.0.1:{port}")).unwrap(),
            secret: None,
        }
    }

    #[tokio::test]
    async fn probe_fails_against_closed_port_and_succeeds_against_version_server() {
        // Closed port → probe false.
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let probe = HealthCheck::new(&controller(port)).unwrap();
        assert!(!probe.probe_once().await);

        // Minimal /version responder → probe true.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { continue };
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf).await;
                    let body = r#"{"meta":true,"version":"t"}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                });
            }
        });
        let probe = HealthCheck::new(&controller(port)).unwrap();
        assert!(probe.probe_once().await);
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p nyanpasu-core-manager health`
Expected: PASS. Delete the now-unneeded empty `tests/health_probe.rs` plan entry — the unit test above covers this task (do not create the file).

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/src/health.rs
git commit -m "feat(nyanpasu-core-manager): add the version health probe"
```

### Task 8: `Instance` — spawn, monitor task, happy path to `Running`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/instance.rs` (full rewrite)
- Create: `crates/nyanpasu-core-manager/tests/instance_lifecycle.rs`

**Interfaces:**
- Produces (used by `CoreManager` and all M2 tests):
  - `pub struct Instance` with `pub async fn spawn(spec: InstanceSpec, epoch: u64, controller: ResolvedController, parent: CancellationToken) -> Result<Instance, Error>`, `pub fn state(&self) -> watch::Receiver<InstanceState>`, `pub fn epoch(&self) -> u64`, `pub fn spec(&self) -> &InstanceSpec`, `pub fn controller(&self) -> &ResolvedController`, `pub fn pid(&self) -> Option<u32>`, `pub async fn wait_ready(&self) -> Result<(), Error>`, `pub async fn stop(self) -> Result<(), Error>`.
- Consumes: `HealthCheck` (Task 7), `run_args`/`mihomo_safe_paths`/`error_summary` (Tasks 1–2), state/spec/error types (Task 3), `Supervisor` from `nyanpasu_utils::process`.

- [ ] **Step 1: Write the failing test in `tests/instance_lifecycle.rs`**

```rust
mod common;

use nyanpasu_core_manager::{
    instance::Instance,
    spec::ResolvedController,
    state::InstanceState,
};
use tokio_util::sync::CancellationToken;

fn http_controller(port: u16) -> ResolvedController {
    ResolvedController {
        host: clash_api::Host::http(format!("127.0.0.1:{port}")).unwrap(),
        secret: None,
    }
}

#[tokio::test]
async fn start_confirms_via_version_probe() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nx-fake-core:\n  ready-delay-ms: 300\n"),
    );
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    let (recorder, log) = common::record_states(instance.state());

    instance.wait_ready().await.expect("becomes healthy");
    assert!(matches!(
        *instance.state().borrow(),
        InstanceState::Running { pid } if pid > 0
    ));
    assert_eq!(instance.epoch(), 1);

    instance.stop().await.expect("stop");
    recorder.abort();
    let states = log.lock().clone();
    // Starting must precede Running: the probe gates the Running transition.
    let starting = states.iter().position(|s| matches!(s, InstanceState::Starting));
    let running = states.iter().position(|s| matches!(s, InstanceState::Running { .. }));
    assert!(starting.unwrap() < running.unwrap(), "sequence was {states:?}");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager --test instance_lifecycle`
Expected: FAIL — `Instance::spawn` not found.

- [ ] **Step 3: Rewrite `src/instance.rs`**

```rust
//! Single-epoch core instance: process supervision + health-probed state machine.

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use nyanpasu_utils::process::{
    Command, ProcessError, ProcessEvent, Supervisor, SupervisorEvent, TerminatedPayload,
};
use tokio::{
    sync::{mpsc, watch},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use crate::{
    error::Error,
    health::HealthCheck,
    kind::{self, MIHOMO_SAFE_PATHS_ENV_NAME},
    spec::{InstanceOptions, InstanceSpec, ResolvedController},
    state::{InstanceState, StopReason},
};

const STDERR_TAIL_LINES: usize = 32;

/// One epoch of a running core. The spec is immutable; a config change means a
/// new `Instance` with a new epoch (created by `CoreManager`).
pub struct Instance {
    epoch: u64,
    spec: Arc<InstanceSpec>,
    controller: ResolvedController,
    state_rx: watch::Receiver<InstanceState>,
    shared: Arc<Shared>,
}

struct Shared {
    state_tx: watch::Sender<InstanceState>,
    user_stop: AtomicBool,
    probe_timeout: AtomicBool,
    stderr_tail: parking_lot::Mutex<VecDeque<String>>,
    cancel: CancellationToken,
    supervisor: tokio::sync::Mutex<Option<Supervisor>>,
    monitor: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl Shared {
    fn publish(&self, state: InstanceState) {
        let _ = self.state_tx.send(state);
    }

    fn tail(&self) -> String {
        let buf = self.stderr_tail.lock();
        buf.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

impl Instance {
    pub async fn spawn(
        spec: InstanceSpec,
        epoch: u64,
        controller: ResolvedController,
        parent: CancellationToken,
    ) -> Result<Instance, Error> {
        if tokio::fs::metadata(&spec.config_path).await.is_err() {
            return Err(Error::ConfigNotFound(spec.config_path.clone()));
        }
        if tokio::fs::metadata(&spec.core.binary_path).await.is_err() {
            return Err(Error::BinaryNotFound(spec.core.binary_path.clone()));
        }
        // Rejects kinds without a launch profile (`Meow`) before spawning.
        spec.core.kind.run_args(&spec.working_dir, &spec.config_path)?;

        let spec = Arc::new(spec);
        let (state_tx, state_rx) = watch::channel(InstanceState::Starting);
        let cancel = parent.child_token();
        let shared = Arc::new(Shared {
            state_tx,
            user_stop: AtomicBool::new(false),
            probe_timeout: AtomicBool::new(false),
            stderr_tail: parking_lot::Mutex::new(VecDeque::with_capacity(STDERR_TAIL_LINES)),
            cancel: cancel.clone(),
            supervisor: tokio::sync::Mutex::new(None),
            monitor: tokio::sync::Mutex::new(None),
        });

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let supervisor = Supervisor::builder({
            let spec = spec.clone();
            move || build_command(&spec)
        })
        .restart_policy(spec.options.restart_policy)
        .backoff(spec.options.backoff)
        .cancel_token(cancel.clone())
        .on_event(move |event| {
            let _ = event_tx.send(event);
        })
        .on_process_event({
            let shared = shared.clone();
            move |event| match event {
                ProcessEvent::Stdout(line) => tracing::info!(target: "core", "{line}"),
                ProcessEvent::Stderr(line) => {
                    tracing::warn!(target: "core", "{line}");
                    let mut tail = shared.stderr_tail.lock();
                    if tail.len() == STDERR_TAIL_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
                ProcessEvent::Error(error) => {
                    tracing::warn!(target: "core", "output pump: {error}")
                }
                _ => {}
            }
        })
        .spawn()
        .await?;
        *shared.supervisor.lock().await = Some(supervisor);

        let monitor = tokio::spawn(monitor_loop(
            event_rx,
            shared.clone(),
            spec.options.clone(),
            controller.clone(),
        ));
        *shared.monitor.lock().await = Some(monitor);

        Ok(Instance {
            epoch,
            spec,
            controller,
            state_rx,
            shared,
        })
    }

    pub fn state(&self) -> watch::Receiver<InstanceState> {
        self.state_rx.clone()
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn spec(&self) -> &InstanceSpec {
        &self.spec
    }

    pub fn controller(&self) -> &ResolvedController {
        &self.controller
    }

    pub fn pid(&self) -> Option<u32> {
        match &*self.state_rx.borrow() {
            InstanceState::Running { pid } => Some(*pid),
            _ => None,
        }
    }

    /// Resolves once the initial start is confirmed (`Running`) or failed
    /// (`Stopped`). The startup timeout is enforced by the monitor task.
    pub async fn wait_ready(&self) -> Result<(), Error> {
        let mut rx = self.state_rx.clone();
        loop {
            let state = rx.borrow_and_update().clone();
            match state {
                InstanceState::Running { .. } => return Ok(()),
                InstanceState::Stopped(reason) => {
                    let text = match reason {
                        StopReason::Error(text) => text,
                        other => format!("stopped before ready: {other:?}"),
                    };
                    let stderr_tail = kind::error_summary(self.spec.core.kind, &text);
                    return Err(if self.shared.probe_timeout.load(Ordering::SeqCst) {
                        Error::StartupTimeout { stderr_tail }
                    } else {
                        Error::StartupFailed { stderr_tail }
                    });
                }
                _ => {}
            }
            if rx.changed().await.is_err() {
                return Err(Error::StartupFailed {
                    stderr_tail: self.shared.tail(),
                });
            }
        }
    }

    /// Stops the core (graceful, then tree kill) and waits until the terminal
    /// state is published.
    pub async fn stop(self) -> Result<(), Error> {
        if self.state_rx.borrow().is_terminal() {
            return Ok(());
        }
        self.shared.user_stop.store(true, Ordering::SeqCst);
        self.shared.publish(InstanceState::Stopping);
        let supervisor = self.shared.supervisor.lock().await.take();
        if let Some(supervisor) = supervisor {
            match supervisor.stop().await {
                Ok(()) | Err(ProcessError::AlreadyExited) => {}
                Err(error) => return Err(Error::Process(error)),
            }
        }
        if let Some(monitor) = self.shared.monitor.lock().await.take() {
            let _ = monitor.await;
        }
        Ok(())
    }
}

fn build_command(spec: &InstanceSpec) -> Command {
    let args = spec
        .core
        .kind
        .run_args(&spec.working_dir, &spec.config_path)
        .expect("kind validated in Instance::spawn");
    let config_dir = spec
        .config_path
        .parent()
        .unwrap_or(spec.config_path.as_path());
    let mut command = Command::new(spec.core.binary_path.as_str())
        .args(args)
        .env(
            MIHOMO_SAFE_PATHS_ENV_NAME,
            kind::mihomo_safe_paths(&spec.working_dir, config_dir),
        )
        .current_dir(spec.working_dir.as_str());
    if let Some(pid_file) = &spec.pid_file {
        command = command.pid_file(pid_file.as_str());
    }
    command
}

struct Probe {
    pid: u32,
    deadline: Instant,
}

async fn monitor_loop(
    mut events: mpsc::UnboundedReceiver<SupervisorEvent>,
    shared: Arc<Shared>,
    options: InstanceOptions,
    controller: ResolvedController,
) {
    let health = match HealthCheck::new(&controller) {
        Ok(health) => health,
        Err(error) => {
            shared.cancel.cancel();
            shared.publish(InstanceState::Stopped(StopReason::Error(format!(
                "failed to build the health-probe client: {error}"
            ))));
            return;
        }
    };

    let initial_deadline = Instant::now() + options.startup_timeout;
    let mut ever_ready = false;
    let mut timeout_fired = false;
    let mut probe: Option<Probe> = None;
    let mut last_exit: Option<TerminatedPayload> = None;

    loop {
        tokio::select! {
            maybe = events.recv() => match maybe {
                Some(SupervisorEvent::Started { pid }) => {
                    probe = Some(Probe {
                        pid,
                        deadline: Instant::now() + options.startup_timeout,
                    });
                }
                Some(SupervisorEvent::Restarting { attempt, .. }) => {
                    probe = None;
                    shared.publish(InstanceState::Restarting { attempt });
                }
                Some(SupervisorEvent::Exited(payload)) => last_exit = Some(payload),
                Some(SupervisorEvent::GaveUp) => {
                    shared.publish(InstanceState::Stopped(StopReason::Error(format!(
                        "core kept crashing; restart budget exhausted\n{}",
                        shared.tail()
                    ))));
                    return;
                }
                Some(SupervisorEvent::Stopped) => {
                    publish_terminal(&shared, last_exit.as_ref());
                    return;
                }
                Some(_) => {} // `Ready` (alive-after) only resets the restart budget
                None => {
                    publish_terminal(&shared, last_exit.as_ref());
                    return;
                }
            },
            _ = tokio::time::sleep_until(initial_deadline), if !ever_ready && !timeout_fired => {
                // Total limit for the initial start, crash-retries included.
                timeout_fired = true;
                probe = None;
                shared.probe_timeout.store(true, Ordering::SeqCst);
                shared.cancel.cancel(); // the supervisor kills the tree, then emits Stopped
            }
            _ = tokio::time::sleep(options.probe_interval), if probe.is_some() => {
                let deadline = probe.as_ref().expect("guarded").deadline;
                if health.probe_once().await {
                    let pid = probe.take().expect("guarded").pid;
                    ever_ready = true;
                    shared.publish(InstanceState::Running { pid });
                } else if ever_ready && Instant::now() >= deadline {
                    // A post-crash respawn never became healthy again.
                    probe = None;
                    shared.probe_timeout.store(true, Ordering::SeqCst);
                    shared.cancel.cancel();
                }
            }
        }
    }
}

fn publish_terminal(shared: &Shared, last_exit: Option<&TerminatedPayload>) {
    let reason = if shared.user_stop.load(Ordering::SeqCst) {
        StopReason::User
    } else if shared.probe_timeout.load(Ordering::SeqCst) {
        StopReason::Error(format!("health probe timed out\n{}", shared.tail()))
    } else if last_exit.is_some_and(|payload| payload.code == Some(0)) {
        StopReason::Finished
    } else {
        StopReason::Error(format!(
            "core exited unexpectedly ({last_exit:?})\n{}",
            shared.tail()
        ))
    };
    let _ = shared.state_tx.send(InstanceState::Stopped(reason));
}
```

Also add to `lib.rs` re-exports: `pub use instance::Instance;`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager --test instance_lifecycle`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add the health-probed single-epoch instance"
```

### Task 9: startup timeout kills the process tree

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/instance_lifecycle.rs`

**Interfaces:**
- Consumes: `Instance` (Task 8), fake-core `never-ready`, `common::wait_port_refused`.

- [ ] **Step 1: Write the failing (or passing — this validates Task 8's timeout path) test**

```rust
#[tokio::test]
async fn startup_timeout_kills_the_core() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nx-fake-core:\n  never-ready: true\n"),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = std::time::Duration::from_secs(1);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    let err = instance.wait_ready().await.expect_err("must time out");
    assert!(
        matches!(err, nyanpasu_core_manager::Error::StartupTimeout { .. }),
        "got {err}"
    );
    // The tree is killed: the fake core's controller port must be released.
    common::wait_port_refused(port).await;
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p nyanpasu-core-manager --test instance_lifecycle startup_timeout`
Expected: PASS (Task 8 implemented the path; if it fails, fix `monitor_loop` — the `initial_deadline` arm must fire while the probe is still pending and `wait_ready` must map `probe_timeout` to `StartupTimeout`).

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests/instance_lifecycle.rs
git commit -m "test(nyanpasu-core-manager): cover the startup-timeout kill path"
```

### Task 10: immediate-exit failure surfaces the stderr tail

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/instance_lifecycle.rs`

**Interfaces:**
- Consumes: fake-core `exit-code` + `stderr-lines`; `Error::StartupFailed` mapping via `error_summary`.

- [ ] **Step 1: Write the test**

```rust
#[tokio::test]
async fn immediate_exit_reports_stderr_tail() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  exit-code: 1\n  stderr-lines:\n    - \"boot marker failure\"\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    // One retry, tiny backoff: GaveUp arrives well inside the startup deadline.
    spec.options.restart_policy =
        nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 1 };

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn succeeds; the failure is the exit");
    let err = instance.wait_ready().await.expect_err("must fail");
    match err {
        nyanpasu_core_manager::Error::StartupFailed { stderr_tail } => {
            assert!(stderr_tail.contains("boot marker failure"), "tail: {stderr_tail}")
        }
        other => panic!("unexpected error: {other}"),
    }
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p nyanpasu-core-manager --test instance_lifecycle immediate_exit`
Expected: PASS (validates Task 8's `GaveUp` → `Stopped(Error(tail))` → `StartupFailed` chain).

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests/instance_lifecycle.rs
git commit -m "test(nyanpasu-core-manager): cover immediate-exit startup failure"
```

### Task 11: crash recovery and restart-budget exhaustion

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/instance_lifecycle.rs`

**Interfaces:**
- Consumes: fake-core `crash-after-ms`/`crash-times`/`state-file`; `InstanceState::Restarting`; supervisor budget semantics.

- [ ] **Step 1: Write the two tests**

```rust
#[tokio::test]
async fn crash_recovers_through_restart_and_reprobe() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 400\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    let (recorder, log) = common::record_states(instance.state());
    instance.wait_ready().await.expect("initially healthy");

    // First run crashes at ~400ms; the supervisor restarts; the re-probe
    // confirms the second (healthy) run.
    let mut rx = instance.state();
    common::wait_for_state(
        &mut rx,
        |s| matches!(s, InstanceState::Restarting { .. }),
        std::time::Duration::from_secs(5),
    )
    .await;
    common::wait_for_state(
        &mut rx,
        |s| matches!(s, InstanceState::Running { .. }),
        std::time::Duration::from_secs(10),
    )
    .await;

    instance.stop().await.expect("stop");
    recorder.abort();
    let states = log.lock().clone();
    let running_count = states
        .iter()
        .filter(|s| matches!(s, InstanceState::Running { .. }))
        .count();
    assert!(running_count >= 2, "sequence was {states:?}");
}

#[tokio::test]
async fn crash_loop_exhausts_the_budget() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 300\n  crash-times: 99\n  state-file: {state_file}\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.restart_policy =
        nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 1 };

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    instance.wait_ready().await.expect("first run is briefly healthy");

    let mut rx = instance.state();
    let terminal = common::wait_for_state(
        &mut rx,
        |s| s.is_terminal(),
        std::time::Duration::from_secs(15),
    )
    .await;
    assert!(
        matches!(
            &terminal,
            InstanceState::Stopped(nyanpasu_core_manager::StopReason::Error(msg))
                if msg.contains("restart budget exhausted")
        ),
        "terminal was {terminal:?}"
    );
}
```

- [ ] **Step 2: Run**

Run: `cargo test -p nyanpasu-core-manager --test instance_lifecycle crash`
Expected: PASS (2 tests). These validate the `Restarting` publication and the `GaveUp` terminal path after a confirmed-healthy run.

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests/instance_lifecycle.rs
git commit -m "test(nyanpasu-core-manager): cover crash recovery and budget exhaustion"
```

### Task 12: user stop

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/instance_lifecycle.rs`

- [ ] **Step 1: Write the test**

```rust
#[tokio::test]
async fn user_stop_is_terminal_and_releases_the_port() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config =
        common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    instance.wait_ready().await.expect("healthy");
    let mut rx = instance.state();
    instance.stop().await.expect("stop");

    let terminal = common::wait_for_state(
        &mut rx,
        |s| s.is_terminal(),
        std::time::Duration::from_secs(5),
    )
    .await;
    assert!(
        matches!(
            terminal,
            InstanceState::Stopped(nyanpasu_core_manager::StopReason::User)
        ),
        "terminal was {terminal:?}"
    );
    common::wait_port_refused(port).await;
}
```

- [ ] **Step 2: Run the whole M2 suite**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS — all unit tests plus `check_config`, `fake_core_smoke`, and 6 `instance_lifecycle` tests.

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests/instance_lifecycle.rs
git commit -m "test(nyanpasu-core-manager): cover user stop semantics"
```

# M3 — CoreManager orchestration with hard switching

### Task 13: `CoreManager` — start/stop, epoch, watch aggregation (Passthrough)

**Files:**
- Create: `crates/nyanpasu-core-manager/src/manager.rs`
- Modify: `crates/nyanpasu-core-manager/src/lib.rs`
- Create: `crates/nyanpasu-core-manager/tests/manager_orchestration.rs`

**Interfaces:**
- Produces: `pub struct CoreManager` with `pub fn new(options: ManagerOptions) -> Self`, `pub fn subscribe(&self) -> watch::Receiver<CoreStatus>`, `pub fn status(&self) -> CoreStatus`, `pub async fn start(&self, spec: InstanceSpec) -> Result<(), Error>`, `pub async fn stop(&self) -> Result<(), Error>`, `pub async fn shutdown(&self) -> Result<(), Error>`, and the spec §6.1 convenience `pub async fn check_config(&self, spec: &InstanceSpec) -> Result<(), Error>` delegating to `kind::check_config`.
- Internal shape later tasks build on: `struct Inner { options, ctrl: tokio::sync::Mutex<Ctrl>, status_tx: watch::Sender<CoreStatus>, epoch: AtomicU64 }`, `struct Ctrl { current: Option<Active>, last_spec: Option<InstanceSpec> }`, `struct Active { instance: Instance, forwarder: JoinHandle<()> }`, `fn next_epoch(&self) -> u64`, `async fn prepare(&self, spec: &InstanceSpec, epoch: u64) -> Result<(InstanceSpec, ResolvedController, Option<Host>), Error>`, `async fn start_locked(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error>`, `fn spawn_forwarder(inner, state_rx, epoch) -> JoinHandle<()>`.
- Consumes: `Instance` (Task 8), `config::{inspect, resolve_controller}` (Task 4).

- [ ] **Step 1: Write the failing tests in `tests/manager_orchestration.rs`**

```rust
mod common;

use std::time::Duration;

use nyanpasu_core_manager::{
    CoreState, Error, ManagerOptions, manager::CoreManager,
};

fn manager() -> CoreManager {
    CoreManager::new(ManagerOptions::default())
}

async fn wait_core_state(
    rx: &mut tokio::sync::watch::Receiver<nyanpasu_core_manager::CoreStatus>,
    pred: impl Fn(&CoreState) -> bool,
    timeout: Duration,
) -> CoreState {
    tokio::time::timeout(timeout, async {
        loop {
            let current = rx.borrow_and_update().state.clone();
            if pred(&current) {
                return current;
            }
            rx.changed().await.expect("status channel open");
        }
    })
    .await
    .expect("timed out waiting for core state")
}

#[tokio::test]
async fn start_publishes_running_and_rejects_double_start() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config =
        common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let manager = manager();
    let mut rx = manager.subscribe();
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped { reason: None }
    ));

    manager.start(spec.clone()).await.expect("start");
    let state = wait_core_state(
        &mut rx,
        |s| matches!(s, CoreState::Running { .. }),
        Duration::from_secs(10),
    )
    .await;
    let CoreState::Running { epoch, pid } = state else { unreachable!() };
    assert!(epoch >= 1 && pid > 0);
    let status = manager.status();
    assert_eq!(
        status.spec.as_ref().map(|s| s.config_path.clone()),
        Some(spec.config_path.clone())
    );
    assert!(status.changed_at > 0);

    let err = manager.start(spec).await.expect_err("double start");
    assert!(matches!(err, Error::AlreadyRunning), "got {err}");

    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn stop_requires_a_running_core() {
    let manager = manager();
    assert!(matches!(manager.stop().await, Err(Error::NotStarted)));
}

#[tokio::test]
async fn failed_start_reports_error_and_publishes_stopped() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nx-fake-core:\n  never-ready: true\n"),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_secs(1);

    let manager = manager();
    let err = manager.start(spec).await.expect_err("must fail");
    assert!(matches!(err, Error::StartupTimeout { .. }), "got {err}");
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped { reason: Some(_) }
    ));
}

#[tokio::test]
async fn missing_controller_is_rejected_strictly() {
    let (_guard, dir) = common::utf8_tempdir();
    let config = common::write_config(&dir, "mixed-port: 7890\n");
    let spec = common::mihomo_spec(&dir, config);
    let manager = manager();
    assert!(matches!(
        manager.start(spec).await,
        Err(Error::ControllerMissing)
    ));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager --test manager_orchestration`
Expected: FAIL — `manager` module not found.

- [ ] **Step 3: Write `src/manager.rs`**

```rust
//! Cross-epoch orchestration: start/stop/switch and status publication.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use tokio::sync::watch;

use crate::{
    config,
    error::Error,
    instance::Instance,
    spec::{ControllerMode, InstanceSpec, ManagerOptions, ResolvedController},
    state::{CoreState, CoreStatus, InstanceState, SpecSummary, StopReason, now_ms},
};

pub struct CoreManager {
    inner: Arc<Inner>,
}

struct Inner {
    options: ManagerOptions,
    ctrl: tokio::sync::Mutex<Ctrl>,
    status_tx: watch::Sender<CoreStatus>,
    epoch: AtomicU64,
}

#[derive(Default)]
struct Ctrl {
    current: Option<Active>,
    last_spec: Option<InstanceSpec>,
}

struct Active {
    instance: Instance,
    forwarder: tokio::task::JoinHandle<()>,
}

impl Inner {
    fn publish_state(&self, state: CoreState) {
        self.status_tx.send_modify(|status| {
            status.state = state;
            status.changed_at = now_ms();
        });
    }

    fn publish_context(&self, spec: Option<SpecSummary>, controller: Option<clash_api::Host>) {
        self.status_tx.send_modify(|status| {
            status.spec = spec;
            status.controller = controller;
        });
    }
}

impl CoreManager {
    pub fn new(options: ManagerOptions) -> Self {
        let (status_tx, _) = watch::channel(CoreStatus::initial());
        Self {
            inner: Arc::new(Inner {
                options,
                ctrl: tokio::sync::Mutex::default(),
                status_tx,
                epoch: AtomicU64::new(0),
            }),
        }
    }

    pub fn subscribe(&self) -> watch::Receiver<CoreStatus> {
        self.inner.status_tx.subscribe()
    }

    pub fn status(&self) -> CoreStatus {
        self.inner.status_tx.borrow().clone()
    }

    fn next_epoch(&self) -> u64 {
        self.inner.epoch.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Mode-dependent launch preparation: the effective spec (Managed mode may
    /// swap the config path for a derived one), the probe controller, and the
    /// publicly advertised controller endpoint (Managed only).
    async fn prepare(
        &self,
        spec: &InstanceSpec,
        _epoch: u64,
    ) -> Result<(InstanceSpec, ResolvedController, Option<clash_api::Host>), Error> {
        match &self.inner.options.controller_mode {
            ControllerMode::Passthrough => {
                let info = config::inspect(&spec.config_path).await?;
                let controller = config::resolve_controller(&info)?;
                Ok((spec.clone(), controller, None))
            }
            #[allow(unreachable_patterns)] // `Managed` arrives in M4
            _ => Err(Error::ControllerMissing),
        }
    }

    pub async fn start(&self, spec: InstanceSpec) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        let running = ctrl
            .current
            .as_ref()
            .is_some_and(|active| !active.instance.state().borrow().is_terminal());
        if running {
            return Err(Error::AlreadyRunning);
        }
        if let Some(stale) = ctrl.current.take() {
            stale.forwarder.abort();
        }
        self.start_locked(&mut ctrl, spec).await
    }

    async fn start_locked(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error> {
        let epoch = self.next_epoch();
        let (effective_spec, controller, advertised) = self.prepare(&spec, epoch).await?;
        self.inner.publish_context(
            Some(SpecSummary {
                kind: spec.core.kind,
                config_path: spec.config_path.clone(),
            }),
            advertised,
        );
        self.inner.publish_state(CoreState::Starting { epoch });

        let instance = match Instance::spawn(
            effective_spec,
            epoch,
            controller,
            self.inner.options.cancel_token.clone(),
        )
        .await
        {
            Ok(instance) => instance,
            Err(error) => {
                self.inner.publish_state(CoreState::Stopped {
                    reason: Some(StopReason::Error(error.to_string())),
                });
                return Err(error);
            }
        };

        match instance.wait_ready().await {
            Ok(()) => {
                let pid = instance.pid().unwrap_or_default();
                self.inner.publish_state(CoreState::Running { epoch, pid });
                let forwarder = spawn_forwarder(self.inner.clone(), instance.state(), epoch);
                ctrl.current = Some(Active { instance, forwarder });
                ctrl.last_spec = Some(spec);
                Ok(())
            }
            Err(error) => {
                self.inner.publish_state(CoreState::Stopped {
                    reason: Some(StopReason::Error(error.to_string())),
                });
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        let Some(active) = ctrl.current.take() else {
            return Err(Error::NotStarted);
        };
        active.forwarder.abort();
        if active.instance.state().borrow().is_terminal() {
            return Err(Error::NotStarted);
        }
        let epoch = active.instance.epoch();
        self.inner.publish_state(CoreState::Stopping { epoch });
        active.instance.stop().await?;
        self.inner.publish_state(CoreState::Stopped {
            reason: Some(StopReason::User),
        });
        Ok(())
    }

    /// One-shot `-t` validation of a spec's config (spec §6.1 convenience).
    pub async fn check_config(&self, spec: &InstanceSpec) -> Result<(), Error> {
        crate::kind::check_config(spec).await
    }

    /// Service-shutdown teardown: stop whatever is running, tolerate nothing running.
    pub async fn shutdown(&self) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        if let Some(active) = ctrl.current.take() {
            active.forwarder.abort();
            if !active.instance.state().borrow().is_terminal() {
                let epoch = active.instance.epoch();
                self.inner.publish_state(CoreState::Stopping { epoch });
                active.instance.stop().await?;
            }
            self.inner.publish_state(CoreState::Stopped {
                reason: Some(StopReason::User),
            });
        }
        Ok(())
    }
}

/// Steady-state bridge: instance transitions → manager status. Installed only
/// once a start/switch confirmed `Running`; aborted before any control action.
fn spawn_forwarder(
    inner: Arc<Inner>,
    mut state_rx: watch::Receiver<InstanceState>,
    epoch: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if state_rx.changed().await.is_err() {
                break;
            }
            let state = state_rx.borrow_and_update().clone();
            let core_state = match state {
                InstanceState::Starting => CoreState::Starting { epoch },
                InstanceState::Running { pid } => CoreState::Running { epoch, pid },
                InstanceState::Restarting { attempt } => {
                    CoreState::Restarting { epoch, attempt }
                }
                InstanceState::Stopping => CoreState::Stopping { epoch },
                InstanceState::Stopped(reason) => {
                    inner.publish_state(CoreState::Stopped { reason: Some(reason) });
                    break;
                }
            };
            inner.publish_state(core_state);
        }
    })
}
```

`lib.rs`: add `pub mod manager;` and `pub use manager::CoreManager;`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager --test manager_orchestration`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add CoreManager start/stop with status publication"
```

### Task 14: `restart` and hard `switch`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/manager.rs`
- Modify: `crates/nyanpasu-core-manager/tests/manager_orchestration.rs`

**Interfaces:**
- Produces (M3 signatures; Task 19 migrates them to `Result<SwitchOutcome, Error>`): `pub async fn restart(&self) -> Result<(), Error>` (= switch to `last_spec`; `Err(NotStarted)` if never started), `pub async fn switch(&self, spec: InstanceSpec) -> Result<(), Error>` (hard switch; behaves as `start` when nothing is running). Internal: `async fn hard_switch(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error>` publishing `CoreState::Switching { from, to }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn hard_switch_replaces_the_core_and_bumps_the_epoch() {
    let (_guard, dir) = common::utf8_tempdir();
    let port_a = common::free_port();
    let port_b = common::free_port();
    let config_a =
        common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port_a}\n"));
    let config_b = dir.join("config-b.yaml");
    std::fs::write(&config_b, format!("external-controller: 127.0.0.1:{port_b}\n")).unwrap();

    let manager = manager();
    let mut rx = manager.subscribe();
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start");
    let CoreState::Running { epoch: first, .. } = manager.status().state else {
        panic!("not running")
    };

    let mut spec_b = common::mihomo_spec(&dir, config_b);
    spec_b.config_path = dir.join("config-b.yaml");
    manager.switch(spec_b).await.expect("switch");

    let state = wait_core_state(
        &mut rx,
        |s| matches!(s, CoreState::Running { .. }),
        Duration::from_secs(10),
    )
    .await;
    let CoreState::Running { epoch: second, .. } = state else { unreachable!() };
    assert!(second > first, "epoch must increase: {first} -> {second}");
    common::wait_port_refused(port_a).await; // old core is dead

    // The Switching window was published.
    // (record from a fresh subscription is racy; assert via history captured below)
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn restart_uses_the_last_spec_and_survives_stop() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config =
        common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));

    let manager = manager();
    assert!(matches!(manager.restart().await, Err(Error::NotStarted)));

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start");
    manager.stop().await.expect("stop");
    // Legacy parity: restart after stop starts the remembered spec again.
    manager.restart().await.expect("restart after stop");
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn switch_publishes_a_switching_window() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config =
        common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let manager = manager();
    manager.start(spec.clone()).await.expect("start");

    let mut rx = manager.subscribe();
    let seen = std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
    let seen_ = seen.clone();
    let recorder = tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            seen_.lock().push(rx.borrow_and_update().state.clone());
        }
    });

    manager.restart().await.expect("restart");
    recorder.abort();
    let states = seen.lock().clone();
    assert!(
        states.iter().any(|s| matches!(s, CoreState::Switching { .. })),
        "sequence was {states:?}"
    );
    manager.shutdown().await.expect("shutdown");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager --test manager_orchestration switch`
Expected: FAIL — `switch`/`restart` not found.

- [ ] **Step 3: Implement in `manager.rs`**

```rust
    pub async fn restart(&self) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        let spec = ctrl.last_spec.clone().ok_or(Error::NotStarted)?;
        self.switch_locked(&mut ctrl, spec).await
    }

    pub async fn switch(&self, spec: InstanceSpec) -> Result<(), Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        self.switch_locked(&mut ctrl, spec).await
    }

    async fn switch_locked(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error> {
        let running = ctrl
            .current
            .as_ref()
            .is_some_and(|active| !active.instance.state().borrow().is_terminal());
        if !running {
            if let Some(stale) = ctrl.current.take() {
                stale.forwarder.abort();
            }
            return self.start_locked(ctrl, spec).await;
        }
        self.hard_switch(ctrl, spec).await
    }

    async fn hard_switch(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error> {
        let active = ctrl.current.take().expect("running checked by caller");
        active.forwarder.abort();
        let from = active.instance.epoch();
        // Safe peek: `epoch` only advances under the ctrl lock we hold.
        let to = self.inner.epoch.load(Ordering::Relaxed) + 1;
        self.inner
            .publish_state(CoreState::Switching { from: Some(from), to });
        active.instance.stop().await?;
        self.start_locked(ctrl, spec).await
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager --test manager_orchestration`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add restart and hard switching"
```

### Task 15: equivalence-sequence test (legacy parity gate)

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/manager_orchestration.rs`

**Interfaces:**
- Consumes: everything from M2/M3. This is the spec §8 equivalence check: the externally visible sequence for start → crash-recover → stop matches the legacy `CoreInstance` contract (start confirmed only when healthy; crash surfaces a non-running phase; user stop is terminal with no restart).

- [ ] **Step 1: Write the test**

```rust
#[tokio::test]
async fn lifecycle_sequence_matches_legacy_contract() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 400\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );

    let manager = manager();
    let mut rx = manager.subscribe();
    let seen = std::sync::Arc::new(parking_lot::Mutex::new(vec![rx.borrow().state.clone()]));
    let seen_ = seen.clone();
    let recorder = tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            seen_.lock().push(rx.borrow_and_update().state.clone());
        }
    });

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start");
    // Crash at ~400ms, recovery, then user stop.
    tokio::time::sleep(Duration::from_secs(3)).await;
    manager.stop().await.expect("stop");
    recorder.abort();

    let states = seen.lock().clone();
    let position = |pred: &dyn Fn(&CoreState) -> bool| states.iter().position(|s| pred(s));
    let starting = position(&|s| matches!(s, CoreState::Starting { .. })).expect("Starting");
    let running = position(&|s| matches!(s, CoreState::Running { .. })).expect("Running");
    let restarting =
        position(&|s| matches!(s, CoreState::Restarting { .. })).expect("Restarting");
    let stopped = states
        .iter()
        .rposition(|s| matches!(s, CoreState::Stopped { reason: Some(StopReason::User) }))
        .expect("terminal user stop");
    assert!(starting < running && running < restarting && restarting < stopped,
        "sequence was {states:?}");
    let running_after_restart = states[restarting..]
        .iter()
        .any(|s| matches!(s, CoreState::Running { .. }));
    assert!(running_after_restart, "recovery must re-confirm Running: {states:?}");
}
```

Add `use nyanpasu_core_manager::StopReason;` to the test file imports.

- [ ] **Step 2: Run the whole suite**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS.

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests/manager_orchestration.rs
git commit -m "test(nyanpasu-core-manager): add the legacy lifecycle equivalence gate"
```

# M4 — Managed mode and graceful switching

### Task 16: fake_core local transports (named pipe / unix socket)

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/helpers/fake_core.rs`
- Create: `crates/nyanpasu-core-manager/tests/local_transport_smoke.rs`

**Interfaces:**
- Produces: fake_core serves the same HTTP surface over `external-controller-pipe` (Windows) and `external-controller-unix` (Unix). Local transports skip the secret check (matching clash-api, which only sends `Authorization` on HTTP hosts).

- [ ] **Step 1: Replace the `serve_local_transports` stub**

```rust
fn serve_local_transports(ctx: &Arc<Ctx>) -> bool {
    let mut served = false;
    #[cfg(windows)]
    if let Some(path) = ctx.behavior.external_controller_pipe.clone() {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            use tokio::net::windows::named_pipe::ServerOptions;
            let mut server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&path)
                .expect("create pipe");
            loop {
                if server.connect().await.is_err() {
                    continue;
                }
                let conn = server;
                server = ServerOptions::new().create(&path).expect("recreate pipe");
                let ctx = ctx.clone();
                tokio::spawn(async move { serve_conn(conn, ctx, false).await });
            }
        });
        served = true;
    }
    #[cfg(unix)]
    if let Some(path) = ctx.behavior.external_controller_unix.clone() {
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind unix socket");
        listener.set_nonblocking(true).expect("nonblocking");
        let listener = tokio::net::UnixListener::from_std(listener).expect("tokio listener");
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else { continue };
                let ctx = ctx.clone();
                tokio::spawn(async move { serve_conn(stream, ctx, false).await });
            }
        });
        served = true;
    }
    let _ = ctx;
    served
}
```

(The `#![allow(dead_code)]` covering platform-conditional `Behavior` fields is already in place from Task 5.)

- [ ] **Step 2: Write `tests/local_transport_smoke.rs`**

```rust
mod common;

use std::time::Duration;

use clash_api::Client;

async fn wait_version(client: &Client) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while client.version().await.is_err() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("fake core never became ready on the local transport");
}

#[cfg(windows)]
#[tokio::test]
async fn fake_core_serves_over_named_pipe() {
    let (_guard, dir) = common::utf8_tempdir();
    let pipe = format!(r"\\.\pipe\nyanpasu-fake-{}", std::process::id());
    let config = common::write_config(
        &dir,
        &format!("external-controller-pipe: {pipe}\n"),
    );
    let mut child = tokio::process::Command::new(common::fake_core_bin())
        .args(["-m", "-d", dir.as_str(), "-f", config.as_str()])
        .kill_on_drop(true)
        .spawn()
        .expect("spawn");
    let client = Client::new_named_pipe(&pipe).unwrap();
    wait_version(&client).await;
    child.kill().await.ok();
}

#[cfg(unix)]
#[tokio::test]
async fn fake_core_serves_over_unix_socket() {
    let (_guard, dir) = common::utf8_tempdir();
    let socket = dir.join("fake.sock");
    let config = common::write_config(
        &dir,
        &format!("external-controller-unix: {socket}\n"),
    );
    let mut child = tokio::process::Command::new(common::fake_core_bin())
        .args(["-m", "-d", dir.as_str(), "-f", config.as_str()])
        .kill_on_drop(true)
        .spawn()
        .expect("spawn");
    let client = Client::new_unix_socket(socket.as_str()).unwrap();
    wait_version(&client).await;
    child.kill().await.ok();
}
```

- [ ] **Step 3: Run**

Run: `cargo test -p nyanpasu-core-manager --test local_transport_smoke`
Expected: PASS (1 test on the current platform).

- [ ] **Step 4: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests
git commit -m "test(nyanpasu-core-manager): serve the fake core over local transports"
```

### Task 17: config derivation (`ControllerOnly` / `ZeroListeners`) + `RestorePlan`

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/config.rs`

**Interfaces:**
- Produces:
  - `pub(crate) enum DeriveMode { ControllerOnly, ZeroListeners }`
  - `pub(crate) struct RestorePlan { port, socks_port, redir_port, tproxy_port, mixed_port: Option<i64>, tun_enabled: bool }` with `fn to_patch(&self) -> clash_api::ConfigPatch` and `fn is_empty(&self) -> bool`
  - `pub(crate) struct DerivedConfig { pub path: Utf8PathBuf, pub controller: ResolvedController, pub restore: RestorePlan }`
  - `pub(crate) async fn derive(config_path: &Utf8Path, derived_dir: &Utf8Path, template: Option<&str>, epoch: u64, mode: DeriveMode) -> Result<DerivedConfig, Error>`
  - `pub(crate) fn managed_endpoint_path(derived_dir: &Utf8Path, template: Option<&str>, epoch: u64) -> String` (template `{epoch}` substitution; defaults: Windows `\\.\pipe\nyanpasu\core-{epoch}`, Unix `<derived_dir>/core-{epoch}.sock`)

- [ ] **Step 1: Write the failing unit tests (append to `config.rs` tests)**

```rust
    #[tokio::test]
    async fn derive_zeroes_listeners_and_records_the_restore_plan() {
        let dir = tempfile::tempdir().unwrap();
        let dir = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let src = dir.join("config.yaml");
        std::fs::write(
            &src,
            "mixed-port: 7890\nsocks-port: 0\nexternal-controller: 127.0.0.1:9090\nsecret: sc\nmode: rule\ntun:\n  enable: true\n  stack: system\n",
        )
        .unwrap();

        let derived = derive(&src, &dir, None, 7, DeriveMode::ZeroListeners)
            .await
            .unwrap();
        assert_eq!(derived.restore.mixed_port, Some(7890));
        assert_eq!(derived.restore.socks_port, None, "0 means disabled — not restored");
        assert!(derived.restore.tun_enabled);
        assert_eq!(derived.controller.secret.as_deref(), Some("sc"));

        let out: serde_yaml_ng::Mapping =
            serde_yaml_ng::from_str(&std::fs::read_to_string(&derived.path).unwrap()).unwrap();
        let get = |k: &str| out.get(Value::String(k.to_owned())).cloned();
        assert_eq!(get("mixed-port"), Some(Value::from(0)));
        assert_eq!(get("mode"), Some(Value::from("rule")), "unrelated keys survive");
        assert_eq!(get("external-controller"), None, "HTTP controller stripped");
        let tun = get("tun").unwrap();
        assert_eq!(
            tun.as_mapping().unwrap().get(Value::String("enable".into())),
            Some(&Value::from(false))
        );
        #[cfg(windows)]
        assert!(get("external-controller-pipe").is_some());
        #[cfg(not(windows))]
        assert!(get("external-controller-unix").is_some());
    }

    #[tokio::test]
    async fn derive_controller_only_keeps_listeners() {
        let dir = tempfile::tempdir().unwrap();
        let dir = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
        let src = dir.join("config.yaml");
        std::fs::write(&src, "mixed-port: 7890\n").unwrap();
        let derived = derive(&src, &dir, None, 3, DeriveMode::ControllerOnly)
            .await
            .unwrap();
        assert!(derived.restore.is_empty());
        let out: serde_yaml_ng::Mapping =
            serde_yaml_ng::from_str(&std::fs::read_to_string(&derived.path).unwrap()).unwrap();
        assert_eq!(
            out.get(Value::String("mixed-port".into())),
            Some(&Value::from(7890))
        );
    }

    #[test]
    fn endpoint_template_substitutes_epoch() {
        let dir = camino::Utf8Path::new("/tmp/x");
        assert_eq!(
            managed_endpoint_path(dir, Some(r"\\.\pipe\ny-{epoch}"), 42),
            r"\\.\pipe\ny-42"
        );
        let default_path = managed_endpoint_path(dir, None, 42);
        assert!(default_path.contains("42"), "got {default_path}");
    }

    #[test]
    fn restore_plan_maps_to_config_patch() {
        let plan = RestorePlan {
            mixed_port: Some(7890),
            tun_enabled: true,
            ..Default::default()
        };
        let patch = plan.to_patch();
        assert_eq!(patch.mixed_port, Some(7890));
        assert_eq!(patch.tun.as_ref().map(|t| t.enable), Some(true));
        assert_eq!(patch.port, None);
    }
```

Add `tempfile` usage: it is already a dev-dependency. If `Mapping::get` needs `&Value`, adapt call sites, not tests' intent.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager config`
Expected: FAIL — `derive` not found.

- [ ] **Step 3: Implement in `config.rs`**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeriveMode {
    /// Rewrite only the controller keys (Managed-mode normal start).
    ControllerOnly,
    /// Also zero every listener so the new core can start beside the old one.
    ZeroListeners,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RestorePlan {
    pub port: Option<i64>,
    pub socks_port: Option<i64>,
    pub redir_port: Option<i64>,
    pub tproxy_port: Option<i64>,
    pub mixed_port: Option<i64>,
    pub tun_enabled: bool,
}

impl RestorePlan {
    pub(crate) fn to_patch(&self) -> clash_api::ConfigPatch {
        clash_api::ConfigPatch {
            port: self.port,
            socks_port: self.socks_port,
            redir_port: self.redir_port,
            tproxy_port: self.tproxy_port,
            mixed_port: self.mixed_port,
            tun: self.tun_enabled.then(|| clash_api::TunPatch {
                enable: true,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

pub(crate) struct DerivedConfig {
    pub path: camino::Utf8PathBuf,
    pub controller: ResolvedController,
    pub restore: RestorePlan,
}

pub(crate) fn managed_endpoint_path(
    derived_dir: &Utf8Path,
    template: Option<&str>,
    epoch: u64,
) -> String {
    if let Some(template) = template {
        return template.replace("{epoch}", &epoch.to_string());
    }
    #[cfg(windows)]
    {
        let _ = derived_dir;
        format!(r"\\.\pipe\nyanpasu\core-{epoch}")
    }
    #[cfg(not(windows))]
    {
        derived_dir.join(format!("core-{epoch}.sock")).to_string()
    }
}

fn zero_listener(doc: &mut Mapping, key: &str, slot: &mut Option<i64>) {
    let key = Value::String(key.to_owned());
    if let Some(value) = doc.get(&key).and_then(Value::as_i64).filter(|v| *v != 0) {
        *slot = Some(value);
        doc.insert(key, Value::from(0));
    }
}

/// Writes `derived_dir/epoch-{epoch}.yaml` — the caller's config with the
/// controller swapped for the managed epoch endpoint (and, for
/// [`DeriveMode::ZeroListeners`], every listener disabled and recorded).
pub(crate) async fn derive(
    config_path: &Utf8Path,
    derived_dir: &Utf8Path,
    template: Option<&str>,
    epoch: u64,
    mode: DeriveMode,
) -> Result<DerivedConfig, Error> {
    let raw = tokio::fs::read_to_string(config_path).await?;
    let mut doc: Mapping = serde_yaml_ng::from_str(&raw)?;

    let mut restore = RestorePlan::default();
    if mode == DeriveMode::ZeroListeners {
        zero_listener(&mut doc, "port", &mut restore.port);
        zero_listener(&mut doc, "socks-port", &mut restore.socks_port);
        zero_listener(&mut doc, "redir-port", &mut restore.redir_port);
        zero_listener(&mut doc, "tproxy-port", &mut restore.tproxy_port);
        zero_listener(&mut doc, "mixed-port", &mut restore.mixed_port);
        if let Some(tun) = doc
            .get_mut(Value::String("tun".to_owned()))
            .and_then(Value::as_mapping_mut)
        {
            let enable = Value::String("enable".to_owned());
            if tun.get(&enable).and_then(Value::as_bool) == Some(true) {
                restore.tun_enabled = true;
                tun.insert(enable, Value::from(false));
            }
        }
    }

    doc.remove(Value::String("external-controller".to_owned()));
    doc.remove(Value::String("external-controller-pipe".to_owned()));
    doc.remove(Value::String("external-controller-unix".to_owned()));
    let endpoint = managed_endpoint_path(derived_dir, template, epoch);
    #[cfg(windows)]
    doc.insert(
        Value::String("external-controller-pipe".to_owned()),
        Value::String(endpoint.clone()),
    );
    #[cfg(not(windows))]
    doc.insert(
        Value::String("external-controller-unix".to_owned()),
        Value::String(endpoint.clone()),
    );
    let secret = str_value(&doc, "secret");

    tokio::fs::create_dir_all(derived_dir).await?;
    let path = derived_dir.join(format!("epoch-{epoch}.yaml"));
    tokio::fs::write(&path, serde_yaml_ng::to_string(&doc)?).await?;

    #[cfg(windows)]
    let host = clash_api::Host::named_pipe(&endpoint);
    #[cfg(not(windows))]
    let host = clash_api::Host::unix_socket(&endpoint);
    Ok(DerivedConfig {
        path,
        controller: ResolvedController { host, secret },
        restore,
    })
}
```

Adjust `Mapping::remove`/`get`/`insert` argument forms (`Value` vs `&Value`) to whatever `serde_yaml_ng` compiles — keep behavior identical.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nyanpasu-core-manager config`
Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/nyanpasu-core-manager/src/config.rs
git commit -m "feat(nyanpasu-core-manager): add derived-config generation with a listener restore plan"
```

### Task 18: `ControllerMode::Managed` — managed starts, sweep, advertised endpoint

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/spec.rs`
- Modify: `crates/nyanpasu-core-manager/src/manager.rs`
- Create: `crates/nyanpasu-core-manager/tests/graceful_switch.rs`

**Interfaces:**
- Produces: `ControllerMode::Managed { derived_dir: Utf8PathBuf, controller_template: Option<String> }`; managed `prepare()` (derives `ControllerOnly` config, advertises the endpoint in `CoreStatus.controller`); startup sweep of `derived_dir` (removes stale `epoch-*.yaml` / `core-*.sock`); `Active.derived_path: Option<Utf8PathBuf>` with cleanup after stop/shutdown/hard-switch.
- Consumes: `config::{derive, DeriveMode}` (Task 17), fake_core local transports (Task 16).

- [ ] **Step 1: Add the `Managed` variant to `spec.rs`**

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub enum ControllerMode {
    /// Start the config as-is; extract the probe endpoint from it.
    #[default]
    Passthrough,
    /// Rewrite the config to a manager-owned, epoch-parameterized local
    /// transport endpoint. Prerequisite for graceful switching.
    Managed {
        /// Where derived configs (and default unix sockets) live.
        derived_dir: camino::Utf8PathBuf,
        /// Endpoint template containing `{epoch}`; platform default when `None`.
        controller_template: Option<String>,
    },
}
```

- [ ] **Step 2: Write the failing tests in `tests/graceful_switch.rs`**

```rust
mod common;

use std::time::Duration;

use nyanpasu_core_manager::{
    ControllerMode, CoreState, ManagerOptions, manager::CoreManager,
};

fn unique_template() -> Option<String> {
    #[cfg(windows)]
    {
        Some(format!(r"\\.\pipe\nyanpasu-test-{}-{{epoch}}", std::process::id()))
    }
    #[cfg(not(windows))]
    {
        None // unix default derives the socket under derived_dir (already unique)
    }
}

fn managed_manager(derived_dir: camino::Utf8PathBuf) -> CoreManager {
    CoreManager::new(ManagerOptions {
        controller_mode: ControllerMode::Managed {
            derived_dir,
            controller_template: unique_template(),
        },
        ..Default::default()
    })
}

#[tokio::test]
async fn managed_start_injects_the_epoch_endpoint_and_advertises_it() {
    let (_guard, dir) = common::utf8_tempdir();
    let derived_dir = dir.join("derived");
    // Stale artifacts from a "previous run" must be swept by CoreManager::new.
    std::fs::create_dir_all(&derived_dir).unwrap();
    std::fs::write(derived_dir.join("epoch-99.yaml"), "stale").unwrap();

    // No external-controller in the user config — Managed mode injects one.
    let config = common::write_config(&dir, "mixed-port: 0\n");
    let manager = managed_manager(derived_dir.clone());
    assert!(
        !derived_dir.join("epoch-99.yaml").exists(),
        "stale derived config swept on construction"
    );

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("managed start");
    let status = manager.status();
    assert!(matches!(status.state, CoreState::Running { .. }));
    let controller = status.controller.expect("advertised managed endpoint");
    let endpoint = format!("{controller:?}");
    assert!(endpoint.contains('1'), "endpoint should embed the epoch: {endpoint}");
    assert!(derived_dir.join("epoch-1.yaml").exists());

    manager.shutdown().await.expect("shutdown");
    assert!(
        !derived_dir.join("epoch-1.yaml").exists(),
        "derived config removed after shutdown"
    );
    let _ = Duration::ZERO;
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager --test graceful_switch`
Expected: FAIL — `Managed` variant not found (then, after adding it, sweep/advertise assertions fail until manager.rs is updated).

- [ ] **Step 4: Implement in `manager.rs`**

In `CoreManager::new`, sweep before constructing `Inner`:

```rust
    pub fn new(options: ManagerOptions) -> Self {
        if let ControllerMode::Managed { derived_dir, .. } = &options.controller_mode {
            sweep_derived_dir(derived_dir);
        }
        // ... unchanged ...
    }
```

```rust
/// Removes runtime artifacts left behind by a previous manager process.
fn sweep_derived_dir(derived_dir: &camino::Utf8Path) {
    let Ok(entries) = std::fs::read_dir(derived_dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let stale = (name.starts_with("epoch-") && name.ends_with(".yaml"))
            || (name.starts_with("core-") && name.ends_with(".sock"));
        if stale && let Err(error) = std::fs::remove_file(entry.path()) {
            tracing::warn!("failed to sweep stale derived artifact {name}: {error}");
        }
    }
}
```

Replace the `prepare` fallback arm with the real Managed arm, and thread the derived path (change the return tuple):

```rust
    async fn prepare(
        &self,
        spec: &InstanceSpec,
        epoch: u64,
    ) -> Result<
        (
            InstanceSpec,
            ResolvedController,
            Option<clash_api::Host>,
            Option<camino::Utf8PathBuf>,
        ),
        Error,
    > {
        match &self.inner.options.controller_mode {
            ControllerMode::Passthrough => {
                let info = config::inspect(&spec.config_path).await?;
                let controller = config::resolve_controller(&info)?;
                Ok((spec.clone(), controller, None, None))
            }
            ControllerMode::Managed { derived_dir, controller_template } => {
                let derived = config::derive(
                    &spec.config_path,
                    derived_dir,
                    controller_template.as_deref(),
                    epoch,
                    config::DeriveMode::ControllerOnly,
                )
                .await?;
                let mut effective = spec.clone();
                effective.config_path = derived.path.clone();
                let advertised = Some(derived.controller.host.clone());
                Ok((effective, derived.controller, advertised, Some(derived.path)))
            }
        }
    }
```

`Active` gains the derived path, and every teardown site cleans it up:

```rust
struct Active {
    instance: Instance,
    forwarder: tokio::task::JoinHandle<()>,
    derived_path: Option<camino::Utf8PathBuf>,
}

async fn cleanup_derived(path: Option<camino::Utf8PathBuf>) {
    if let Some(path) = path {
        let _ = tokio::fs::remove_file(&path).await;
    }
}
```

- `start_locked`: destructure the new tuple, store `derived_path` in `Active`; on the `wait_ready` error path call `cleanup_derived(derived_path).await` after publishing `Stopped`.
- `stop` / `shutdown` / `hard_switch`: after `active.instance.stop().await?`, call `cleanup_derived(active.derived_path).await`.

- [ ] **Step 5: Run**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS — including the new `graceful_switch` test and all prior suites (Passthrough behavior unchanged).

- [ ] **Step 6: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add managed controller mode with derived configs"
```

### Task 19: switch decision matrix + graceful switch flow

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/manager.rs`
- Modify: `crates/nyanpasu-core-manager/src/lib.rs`
- Modify: `crates/nyanpasu-core-manager/tests/graceful_switch.rs`

**Interfaces:**
- Produces:
  - `pub enum DegradeReason { NotRunning, PassthroughMode, UnsupportedKind, DnsListen, PatchFailed }`, `pub enum SwitchOutcome { Graceful, Hard { reason: DegradeReason } }` (both `Copy`, re-exported from `lib.rs`).
  - Signature migration: `restart`/`switch` now return `Result<SwitchOutcome, Error>` (M3 tests keep compiling — they ignore the `Ok` payload).
  - `fn decide(managed: bool, kind: CoreKind, has_dns_listen: bool) -> Option<DegradeReason>` (pure; `None` = graceful-eligible).
  - `async fn graceful_switch(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<SwitchOutcome, Error>` implementing spec §6.2 (overlap start → stop old → PATCH restore 3×500ms → fallback hard restart on the same epoch).
  - `start_locked` split: `async fn start_locked_with_epoch(&self, ctrl, spec, epoch_override: Option<u64>) -> Result<(), Error>`.

- [ ] **Step 1: Write the failing matrix unit test (in `manager.rs`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::CoreKind;

    #[test]
    fn switch_matrix_matches_the_spec() {
        assert_eq!(
            decide(false, CoreKind::Mihomo, false),
            Some(DegradeReason::PassthroughMode)
        );
        assert_eq!(
            decide(true, CoreKind::ClashRs, false),
            Some(DegradeReason::UnsupportedKind)
        );
        assert_eq!(
            decide(true, CoreKind::ClashPremium, false),
            Some(DegradeReason::UnsupportedKind)
        );
        assert_eq!(
            decide(true, CoreKind::Mihomo, true),
            Some(DegradeReason::DnsListen)
        );
        assert_eq!(decide(true, CoreKind::Mihomo, false), None);
    }
}
```

- [ ] **Step 2: Write the failing graceful-flow integration test (append to `tests/graceful_switch.rs`)**

```rust
use nyanpasu_core_manager::SwitchOutcome;
use parking_lot::Mutex;
use std::sync::Arc;

#[tokio::test]
async fn graceful_switch_overlaps_and_restores_listeners() {
    let (_guard, dir) = common::utf8_tempdir();
    let derived_dir = dir.join("derived");
    let mixed = common::free_port();
    let patch_log_b = dir.join("patch-b.log");

    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(
        &config_b_path,
        format!("mixed-port: {mixed}\nx-fake-core:\n  patch-log: {patch_log_b}\n"),
    )
    .unwrap();

    let manager = managed_manager(derived_dir);
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("A holds the mixed port");

    let mut rx = manager.subscribe();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_ = seen.clone();
    let recorder = tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            seen_.lock().push(rx.borrow_and_update().state.clone());
        }
    });

    let mut spec_b = common::mihomo_spec(&dir, config_b_path.clone());
    spec_b.config_path = config_b_path;
    let outcome = manager.switch(spec_b).await.expect("switch");
    assert_eq!(outcome, SwitchOutcome::Graceful);
    recorder.abort();

    // The user-visible overlap guarantee: never Stopped during the switch.
    let states = seen.lock().clone();
    assert!(
        states.iter().any(|s| matches!(s, CoreState::Switching { .. })),
        "sequence was {states:?}"
    );
    assert!(
        !states
            .iter()
            .any(|s| matches!(s, CoreState::Stopped { .. })),
        "graceful switch must not publish Stopped: {states:?}"
    );

    // The new core received the original listener values via PATCH.
    let log = std::fs::read_to_string(&patch_log_b).expect("patch log");
    assert!(log.contains(&format!("\"mixed-port\":{mixed}")), "log: {log}");
    // And rebound the port after the old core released it.
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("B serves the mixed port after the switch");

    let CoreState::Running { epoch, .. } = manager.status().state else {
        panic!("not running after switch")
    };
    assert_eq!(epoch, 2);
    manager.shutdown().await.expect("shutdown");
}
```

Note for the implementer: the fake core started from `config-b` boots with `mixed-port: 0` because the manager's *derived* config zeroed it — binding conflicts with A are structurally impossible; if derivation regressed, B would panic on the occupied port and the switch would fail.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p nyanpasu-core-manager --test graceful_switch`
Expected: FAIL — `SwitchOutcome` unresolved / switch returns `()`.

- [ ] **Step 4: Implement in `manager.rs`**

Types and matrix:

```rust
/// Why a switch was executed as a hard stop→start instead of gracefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DegradeReason {
    NotRunning,
    PassthroughMode,
    UnsupportedKind,
    DnsListen,
    /// Graceful overlap succeeded but the listener-restore PATCH kept failing;
    /// converged via a hard restart on the full config.
    PatchFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchOutcome {
    Graceful,
    Hard { reason: DegradeReason },
}

/// Spec §6.3 degradation matrix. `None` means graceful-eligible.
fn decide(managed: bool, kind: CoreKind, has_dns_listen: bool) -> Option<DegradeReason> {
    if !managed {
        return Some(DegradeReason::PassthroughMode);
    }
    if !matches!(kind, CoreKind::Mihomo) {
        return Some(DegradeReason::UnsupportedKind);
    }
    if has_dns_listen {
        return Some(DegradeReason::DnsListen);
    }
    None
}
```

(add `use crate::kind::CoreKind;` and `use std::time::Duration;` to the imports)

Signature migration and routing:

```rust
    pub async fn restart(&self) -> Result<SwitchOutcome, Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        let spec = ctrl.last_spec.clone().ok_or(Error::NotStarted)?;
        self.switch_locked(&mut ctrl, spec).await
    }

    pub async fn switch(&self, spec: InstanceSpec) -> Result<SwitchOutcome, Error> {
        let mut ctrl = self.inner.ctrl.lock().await;
        self.switch_locked(&mut ctrl, spec).await
    }

    async fn switch_locked(
        &self,
        ctrl: &mut Ctrl,
        spec: InstanceSpec,
    ) -> Result<SwitchOutcome, Error> {
        let running = ctrl
            .current
            .as_ref()
            .is_some_and(|active| !active.instance.state().borrow().is_terminal());
        if !running {
            if let Some(stale) = ctrl.current.take() {
                stale.forwarder.abort();
            }
            self.start_locked(ctrl, spec).await?;
            return Ok(SwitchOutcome::Hard { reason: DegradeReason::NotRunning });
        }
        let managed = matches!(
            self.inner.options.controller_mode,
            ControllerMode::Managed { .. }
        );
        let info = config::inspect(&spec.config_path).await?;
        match decide(managed, spec.core.kind, info.has_dns_listen) {
            Some(reason) => {
                self.hard_switch(ctrl, spec).await?;
                Ok(SwitchOutcome::Hard { reason })
            }
            None => self.graceful_switch(ctrl, spec).await,
        }
    }
```

Epoch-override split of `start_locked` (hard-switch and fallback reuse):

```rust
    async fn start_locked(&self, ctrl: &mut Ctrl, spec: InstanceSpec) -> Result<(), Error> {
        self.start_locked_with_epoch(ctrl, spec, None).await
    }

    async fn start_locked_with_epoch(
        &self,
        ctrl: &mut Ctrl,
        spec: InstanceSpec,
        epoch_override: Option<u64>,
    ) -> Result<(), Error> {
        let epoch = epoch_override.unwrap_or_else(|| self.next_epoch());
        // ... body unchanged from Task 18 (prepare/publish/spawn/wait_ready) ...
    }
```

The graceful flow (spec §6.2):

```rust
    async fn graceful_switch(
        &self,
        ctrl: &mut Ctrl,
        spec: InstanceSpec,
    ) -> Result<SwitchOutcome, Error> {
        let ControllerMode::Managed { derived_dir, controller_template } =
            self.inner.options.controller_mode.clone()
        else {
            unreachable!("decide() only selects graceful in Managed mode");
        };
        let old_epoch = ctrl.current.as_ref().map(|a| a.instance.epoch());
        let old_pid = ctrl
            .current
            .as_ref()
            .and_then(|a| a.instance.pid())
            .unwrap_or_default();
        let epoch = self.next_epoch();
        self.inner
            .publish_state(CoreState::Switching { from: old_epoch, to: epoch });

        // 1. Derive B' (listeners zeroed, epoch endpoint injected) and start it
        //    while the old core keeps serving.
        let derived = config::derive(
            &spec.config_path,
            &derived_dir,
            controller_template.as_deref(),
            epoch,
            config::DeriveMode::ZeroListeners,
        )
        .await?;
        let mut effective = spec.clone();
        effective.config_path = derived.path.clone();
        let started = async {
            let instance = Instance::spawn(
                effective,
                epoch,
                derived.controller.clone(),
                self.inner.options.cancel_token.clone(),
            )
            .await?;
            instance.wait_ready().await?;
            Ok::<Instance, Error>(instance)
        }
        .await;
        let instance = match started {
            Ok(instance) => instance,
            Err(error) => {
                // Safe rollback: the old core was never touched.
                cleanup_derived(Some(derived.path)).await;
                if let Some(from) = old_epoch {
                    self.inner
                        .publish_state(CoreState::Running { epoch: from, pid: old_pid });
                }
                return Err(error);
            }
        };

        // 2. Point of no return: stop the old core, releasing its listeners.
        let old = ctrl.current.take().expect("running checked by caller");
        old.forwarder.abort();
        let old_derived = old.derived_path.clone();
        old.instance.stop().await?;
        cleanup_derived(old_derived).await;

        // 3. Restore the original listeners on the new core (3 tries × 500ms).
        let patched = if derived.restore.is_empty() {
            true
        } else {
            let client = crate::health::build_client(instance.controller())?;
            let patch = derived.restore.to_patch();
            let mut ok = false;
            for attempt in 1..=3u32 {
                match client.patch_config(&patch).await {
                    Ok(()) => {
                        ok = true;
                        break;
                    }
                    Err(error) => {
                        tracing::warn!("listener-restore patch attempt {attempt} failed: {error}");
                        if attempt < 3 {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
            }
            ok
        };
        if !patched {
            // 4. Fallback: the old core is dead and its ports are free — hard
            //    restart the new instance on the full config, same epoch.
            instance.stop().await.ok();
            cleanup_derived(Some(derived.path)).await;
            self.start_locked_with_epoch(ctrl, spec, Some(epoch)).await?;
            return Ok(SwitchOutcome::Hard { reason: DegradeReason::PatchFailed });
        }

        // 5. Install the new core.
        let pid = instance.pid().unwrap_or_default();
        self.inner.publish_state(CoreState::Running { epoch, pid });
        let forwarder = spawn_forwarder(self.inner.clone(), instance.state(), epoch);
        ctrl.current = Some(Active {
            instance,
            forwarder,
            derived_path: Some(derived.path),
        });
        ctrl.last_spec = Some(spec);
        Ok(SwitchOutcome::Graceful)
    }
```

`lib.rs`: `pub use manager::{CoreManager, DegradeReason, SwitchOutcome};`.

- [ ] **Step 5: Run**

Run: `cargo test -p nyanpasu-core-manager`
Expected: PASS — matrix unit test, the graceful-flow test, and every earlier suite (M3 tests compile against the new `Result<SwitchOutcome, _>` without edits).

- [ ] **Step 6: Commit**

```powershell
git add crates/nyanpasu-core-manager
git commit -m "feat(nyanpasu-core-manager): add graceful switching with a degradation matrix"
```

### Task 20: graceful-switch failure paths — rollback and PATCH fallback

**Files:**
- Modify: `crates/nyanpasu-core-manager/tests/graceful_switch.rs`

- [ ] **Step 1: Write the two tests**

```rust
#[tokio::test]
async fn failed_new_core_rolls_back_without_touching_the_old_one() {
    let (_guard, dir) = common::utf8_tempdir();
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(&config_b_path, "x-fake-core:\n  never-ready: true\n").unwrap();

    let manager = managed_manager(dir.join("derived"));
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");
    let CoreState::Running { epoch: old_epoch, .. } = manager.status().state else {
        panic!("not running")
    };

    let mut spec_b = common::mihomo_spec(&dir, config_b_path.clone());
    spec_b.config_path = config_b_path;
    spec_b.options.startup_timeout = Duration::from_secs(1);
    manager.switch(spec_b).await.expect_err("switch must fail");

    // The old core is untouched and republished as Running.
    let CoreState::Running { epoch, .. } = manager.status().state else {
        panic!("old core must still be running, got {:?}", manager.status().state)
    };
    assert_eq!(epoch, old_epoch);
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("old core still holds its port");
    manager.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn rejected_patch_falls_back_to_a_hard_restart() {
    let (_guard, dir) = common::utf8_tempdir();
    let mixed = common::free_port();
    let config_a = common::write_config(&dir, &format!("mixed-port: {mixed}\n"));
    let config_b_path = dir.join("config-b.yaml");
    std::fs::write(
        &config_b_path,
        format!("mixed-port: {mixed}\nx-fake-core:\n  reject-patch: true\n"),
    )
    .unwrap();

    let manager = managed_manager(dir.join("derived"));
    manager
        .start(common::mihomo_spec(&dir, config_a))
        .await
        .expect("start A");

    let mut spec_b = common::mihomo_spec(&dir, config_b_path.clone());
    spec_b.config_path = config_b_path;
    let outcome = manager.switch(spec_b).await.expect("switch converges");
    assert_eq!(
        outcome,
        SwitchOutcome::Hard { reason: nyanpasu_core_manager::DegradeReason::PatchFailed }
    );
    // The fallback instance boots on the FULL config, so it binds the port itself.
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    tokio::net::TcpStream::connect(("127.0.0.1", mixed))
        .await
        .expect("fallback core serves the mixed port");
    manager.shutdown().await.expect("shutdown");
}
```

Note: the PATCH-fallback test spends ~1s in retry sleeps (3 × 500ms bounded) — acceptable.

- [ ] **Step 2: Run**

Run: `cargo test -p nyanpasu-core-manager --test graceful_switch`
Expected: PASS (4 tests). If the fallback test hangs, verify `start_locked_with_epoch` re-derives with `ControllerOnly` (ports intact) and that fake_core binds `mixed-port` at startup.

- [ ] **Step 3: Commit**

```powershell
git add crates/nyanpasu-core-manager/tests/graceful_switch.rs
git commit -m "test(nyanpasu-core-manager): cover graceful-switch rollback and patch fallback"
```

### Task 21: final polish — lints, docs, full suite

**Files:**
- Modify: `crates/nyanpasu-core-manager/src/lib.rs` (crate docs only, if gaps remain)

- [ ] **Step 1: Format and lint**

Run: `cargo fmt -p nyanpasu-core-manager` then `cargo clippy -p nyanpasu-core-manager --all-targets`
Expected: no warnings. Fix any findings (typical: needless clones in tests, missing `#[allow(dead_code)]` on platform-conditional fake_core fields).

- [ ] **Step 2: Run the full workspace suite**

Run: `cargo test --workspace`
Expected: PASS — nothing outside the crate changed behavior (`nyanpasu-utils`, `clash-api`, `nyanpasu_ipc`, `nyanpasu_service` untouched).

- [ ] **Step 3: Commit**

```powershell
git add -A
git commit -m "chore(nyanpasu-core-manager): apply lint fixes and finalize the crate"
```

---

## Out of scope (deliberately)

- P4: `nyanpasu_service` wiring (IPC `CoreState` mapping lives in the service per spec §7), deprecating `nyanpasu-utils::core`, deleting `recover_core`.
- IPC protocol enrichment (epoch/attempt over the wire).
- Verifying clash-rs/premium PATCH support and mihomo DNS-listen patching (spec risks O1/O2) — the degradation matrix stays conservative until verified.



