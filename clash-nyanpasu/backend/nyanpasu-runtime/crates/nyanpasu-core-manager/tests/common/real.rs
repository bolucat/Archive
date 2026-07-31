//! Helpers for integration tests against the real core binaries in
//! `tests/bin/` (downloaded by `deno run -A scripts/prepare-cores.ts`).

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    CoreKind, CoreManager, CoreSpec, InstanceOptions, InstanceSpec, LocalIpcPolicy, ManagerOptions,
};

/// A real core under test: its manager kind and the binary name in tests/bin.
#[derive(Debug, Clone, Copy)]
pub struct RealCore {
    pub kind: CoreKind,
    pub name: &'static str,
    /// Whether the core can serve its controller over system IPC
    /// (named pipe / unix socket) at the downloaded version.
    pub system_ipc: bool,
}

pub const MIHOMO: RealCore = RealCore {
    kind: CoreKind::Mihomo,
    name: "mihomo",
    system_ipc: true,
};
pub const CLASH_RS: RealCore = RealCore {
    kind: CoreKind::ClashRust,
    name: "clash-rs",
    system_ipc: true,
};
pub const CLASH_PREMIUM: RealCore = RealCore {
    kind: CoreKind::ClashPremium,
    name: "clash",
    system_ipc: false,
};
pub const MEOW: RealCore = RealCore {
    kind: CoreKind::Meow,
    name: "meow",
    system_ipc: false,
};

pub const REAL_CORES: &[RealCore] = &[MIHOMO, CLASH_RS, CLASH_PREMIUM, MEOW];

pub fn workspace_root() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Utf8Path::parent)
        .expect("crate lives in <workspace>/crates")
        .to_owned()
}

/// Serializes the real-core tests: parallel runs contend on CPU and on
/// ephemeral ports (a free port can be grabbed by another test's core before
/// this one binds it), which made startup and proxy-chain checks flaky.
pub async fn serial_lock() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await
}

/// Locates the core binary in `tests/bin`; `<NAME>_BIN` (e.g. `MIHOMO_BIN`,
/// `CLASH_RS_BIN`) overrides the path.
pub fn real_core_bin(core: &RealCore) -> Utf8PathBuf {
    let env_name = format!("{}_BIN", core.name.to_uppercase().replace('-', "_"));
    let binary = std::env::var_os(&env_name)
        .map(|p| Utf8PathBuf::from_path_buf(p.into()).expect("{env_name} must be UTF-8"))
        .unwrap_or_else(|| {
            workspace_root().join("tests/bin").join(format!(
                "{}{}",
                core.name,
                std::env::consts::EXE_SUFFIX
            ))
        });
    assert!(
        binary.is_file(),
        "{} was not found at {binary}; run `deno run -A scripts/prepare-cores.ts` or set {env_name}",
        core.name,
    );
    binary
}

pub fn real_spec(core: &RealCore, dir: &Utf8Path, config_path: Utf8PathBuf) -> InstanceSpec {
    InstanceSpec {
        core: CoreSpec {
            kind: core.kind,
            binary_path: real_core_bin(core),
            version: None,
            features: Vec::new(),
        },
        config_path,
        working_dir: dir.to_owned(),
        pid_file: None,
        options: InstanceOptions {
            startup_timeout: Duration::from_secs(20),
            ..super::fast_options()
        },
    }
}

/// A config routing every connection through the test socks5 server, so tests
/// can prove the core really loaded and uses the proxy node. The
/// `external-controller` must be a real free port: the Prefer/Disable policies
/// keep it as the control channel instead of rewriting it.
pub fn proxy_config(mixed_port: u16, ctrl_port: u16, socks5_port: u16, extra: &str) -> String {
    format!(
        "mixed-port: {mixed_port}\n\
         external-controller: 127.0.0.1:{ctrl_port}\n\
         proxies:\n\
         \x20 - name: out\n\
         \x20   type: socks5\n\
         \x20   server: 127.0.0.1\n\
         \x20   port: {socks5_port}\n\
         rules:\n\
         \x20 - MATCH,out\n\
         {extra}"
    )
}

fn unique_template() -> Option<String> {
    #[cfg(windows)]
    {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        Some(format!(
            r"\\.\pipe\nyanpasu-real-{}-{n}-{{epoch}}",
            std::process::id()
        ))
    }
    #[cfg(not(windows))]
    {
        None // unix default derives the socket under the runtime dir (already unique)
    }
}

pub async fn manager_with_policy(dir: &Utf8Path, local_ipc_policy: LocalIpcPolicy) -> CoreManager {
    CoreManager::new(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        local_ipc_policy,
        controller_template: unique_template(),
        control_timeout: Duration::from_secs(15),
        reconcile_timeout: Duration::from_secs(15),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct manager")
}
