//! Integration tests against the four real cores (mihomo, clash-rs,
//! clash premium, meow), covering start/stop, config updates, and the
//! Force/Prefer/Disable local IPC policies.
//!
//! Prepare the binaries with `deno run -A scripts/prepare-cores.ts`, then run:
//! `cargo test -p nyanpasu-core-manager --test real_cores -- --ignored --nocapture`.

mod common;

use std::time::Duration;

use camino::Utf8PathBuf;
use common::{
    real::{self, RealCore},
    socks5::{self, Socks5Server},
};
use nyanpasu_core_manager::{
    ApplyOutcome, CoreState, Error, Host, LocalIpcPolicy, RevisionId, manager::CoreManager,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    task::JoinHandle,
};

/// Shared per-test fixture: a socks5 outbound server, an echo target behind
/// it, and free ports for the core's inbound and HTTP controller.
struct TestBed {
    _guard: tempfile::TempDir,
    dir: Utf8PathBuf,
    socks5: Socks5Server,
    echo_port: u16,
    _echo: JoinHandle<()>,
    mixed_port: u16,
    ctrl_port: u16,
}

async fn bed() -> TestBed {
    let (guard, dir) = common::utf8_tempdir();
    let socks5 = Socks5Server::start().await;
    let (echo_port, echo) = socks5::echo_server().await;
    TestBed {
        _guard: guard,
        dir,
        socks5,
        echo_port,
        _echo: echo,
        mixed_port: common::free_port(),
        ctrl_port: common::free_port(),
    }
}

fn write_proxy_config(bed: &TestBed, extra: &str) -> Utf8PathBuf {
    common::write_config(
        &bed.dir,
        &real::proxy_config(bed.mixed_port, bed.ctrl_port, bed.socks5.port(), extra),
    )
}

async fn start_bed(core: &RealCore, policy: LocalIpcPolicy) -> (TestBed, CoreManager) {
    let bed = bed().await;
    let config = write_proxy_config(&bed, "");
    let manager = real::manager_with_policy(&bed.dir, policy).await;
    manager
        .start(real::real_spec(core, &bed.dir, config))
        .await
        .unwrap_or_else(|error| panic!("{} failed to start: {error}", core.name));
    assert!(
        matches!(manager.status().state, CoreState::Running { .. }),
        "{} should be running",
        core.name
    );
    (bed, manager)
}

/// Drives traffic through the core's inbound and asserts it comes back via
/// the socks5 outbound node: client → core mixed-port → `out` proxy →
/// socks5 server → echo target.
///
/// The whole exchange is retried, not just the connect: on a busy runner the
/// core can accept the inbound connection before its outbound dial is ready
/// and drop the client mid-stream.
async fn assert_proxy_chain(bed: &TestBed) {
    let payload: &[u8; 14] = b"nyanpasu-probe";
    let before = bed.socks5.connection_count();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let mut last_error: Option<std::io::Error> = None;
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_secs(5),
            roundtrip(bed.mixed_port, bed.echo_port, payload),
        )
        .await
        {
            Ok(Ok(())) => {
                assert!(
                    bed.socks5.connection_count() > before,
                    "traffic must flow through the socks5 outbound node"
                );
                return;
            }
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => {
                last_error = Some(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "echo timed out",
                ));
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("proxy chain never came up: {last_error:?}");
}

async fn roundtrip(mixed_port: u16, echo_port: u16, payload: &[u8; 14]) -> std::io::Result<()> {
    let mut stream = socks5::connect_via(mixed_port, "127.0.0.1", echo_port).await?;
    stream.write_all(payload).await?;
    let mut buf = [0u8; 14];
    stream.read_exact(&mut buf).await?;
    if &buf != payload {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "echo payload mismatch",
        ));
    }
    Ok(())
}

fn assert_transport(host: &Host, expect_local: bool, context: &str) {
    #[cfg(windows)]
    let is_local = matches!(host, Host::NamedPipe(_));
    #[cfg(not(windows))]
    let is_local = matches!(host, Host::UnixSocket(_));
    if expect_local {
        assert!(is_local, "{context}: expected system IPC, got {host:?}");
    } else {
        assert!(
            matches!(host, Host::Http(_)),
            "{context}: expected HTTP, got {host:?}"
        );
    }
}

// === start / stop ===

async fn start_stop(core: RealCore) {
    let _serial = real::serial_lock().await;
    let (bed, manager) = start_bed(&core, LocalIpcPolicy::Prefer).await;
    let controller = manager.status().controller.expect("controller advertised");
    assert_transport(&controller, core.system_ipc, core.name);
    assert_proxy_chain(&bed).await;

    manager.stop().await.expect("stop");
    assert!(matches!(manager.status().state, CoreState::Stopped { .. }));
    common::wait_port_refused(bed.mixed_port).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn mihomo_starts_proxies_and_stops() {
    start_stop(real::MIHOMO).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn clash_rs_starts_proxies_and_stops() {
    start_stop(real::CLASH_RS).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn clash_premium_starts_proxies_and_stops() {
    start_stop(real::CLASH_PREMIUM).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn meow_starts_proxies_and_stops() {
    start_stop(real::MEOW).await;
}

// === config update ===

async fn config_update(core: RealCore) {
    let _serial = real::serial_lock().await;
    let (mut bed, manager) = start_bed(&core, LocalIpcPolicy::Prefer).await;
    let spec = || real::real_spec(&core, &bed.dir, bed.dir.join("config.yaml"));
    let mut extra = String::new();

    // Noop: applying the unchanged config is a no-op.
    let outcome = manager
        .apply_config(spec(), None)
        .await
        .expect("noop apply");
    assert!(
        matches!(outcome, ApplyOutcome::Noop { .. }),
        "unchanged config should be a noop, got {outcome:?}"
    );

    // CAS: a stale expected revision is rejected.
    let bogus = RevisionId {
        epoch: common::epoch(u64::MAX),
        generation: u64::MAX,
        effective_hash: "bogus".into(),
    };
    let error = manager
        .apply_config(spec(), Some(bogus))
        .await
        .expect_err("stale revision must conflict");
    assert!(
        matches!(error, Error::RevisionConflict { .. }),
        "got {error}"
    );

    // Scalar change (log-level). Mihomo patches it at runtime; the other
    // kinds conservatively restart into a new epoch.
    extra.push_str("log-level: debug\n");
    write_proxy_config(&bed, &extra);
    let outcome = manager
        .apply_config(spec(), None)
        .await
        .expect("log-level apply");
    match core.kind {
        nyanpasu_core_manager::CoreKind::Mihomo => assert!(
            matches!(outcome, ApplyOutcome::Patched { .. }),
            "mihomo log-level change should patch in place, got {outcome:?}"
        ),
        _ => assert!(
            matches!(outcome, ApplyOutcome::Switched { .. }),
            "{} log-level change should switch, got {outcome:?}",
            core.name
        ),
    }
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    assert_proxy_chain(&bed).await;

    // Collection change in the reload set (hosts). Only meaningful for
    // mihomo, which can reload without a restart.
    if matches!(core.kind, nyanpasu_core_manager::CoreKind::Mihomo) {
        extra.push_str("hosts:\n  fixture.test: 198.18.0.1\n");
        write_proxy_config(&bed, &extra);
        let outcome = manager
            .apply_config(spec(), None)
            .await
            .expect("hosts apply");
        assert!(
            matches!(outcome, ApplyOutcome::Reloaded { .. }),
            "mihomo hosts change should reload, got {outcome:?}"
        );
        assert_proxy_chain(&bed).await;
    }

    // Inbound change (mixed-port). Mihomo re-binds the listener at runtime;
    // the others restart. Either way the new port must serve and the old one
    // must be gone.
    let old_port = bed.mixed_port;
    bed.mixed_port = common::free_port();
    write_proxy_config(&bed, &extra);
    let outcome = manager
        .apply_config(spec(), None)
        .await
        .expect("mixed-port apply");
    match core.kind {
        nyanpasu_core_manager::CoreKind::Mihomo => assert!(
            matches!(outcome, ApplyOutcome::Patched { .. }),
            "mihomo mixed-port change should patch in place, got {outcome:?}"
        ),
        _ => assert!(
            matches!(outcome, ApplyOutcome::Switched { .. }),
            "{} mixed-port change should switch, got {outcome:?}",
            core.name
        ),
    }
    assert_proxy_chain(&bed).await;
    common::wait_port_refused(old_port).await;

    manager.stop().await.expect("stop");
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn mihomo_config_update() {
    config_update(real::MIHOMO).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn clash_rs_config_update() {
    config_update(real::CLASH_RS).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn clash_premium_config_update() {
    config_update(real::CLASH_PREMIUM).await;
}

#[ignore = "requires the platform core binaries in tests/bin"]
#[tokio::test]
async fn meow_config_update() {
    config_update(real::MEOW).await;
}

// === IPC modes ===

/// Starts the core in the given IPC mode, asserts the resolved transport, and
/// proves the control channel works over it with a config patch.
async fn ipc_mode_start(core: RealCore, policy: LocalIpcPolicy, expect_local: bool) {
    let _serial = real::serial_lock().await;
    let (bed, manager) = start_bed(&core, policy).await;
    let controller = manager.status().controller.expect("controller advertised");
    assert_transport(&controller, expect_local, core.name);
    assert_proxy_chain(&bed).await;

    // The control channel must work over the negotiated transport.
    write_proxy_config(&bed, "log-level: debug\n");
    let outcome = manager
        .apply_config(
            real::real_spec(&core, &bed.dir, bed.dir.join("config.yaml")),
            None,
        )
        .await
        .expect("apply over the negotiated transport");
    match core.kind {
        nyanpasu_core_manager::CoreKind::Mihomo => assert!(
            matches!(outcome, ApplyOutcome::Patched { .. }),
            "got {outcome:?}"
        ),
        _ => assert!(
            matches!(outcome, ApplyOutcome::Switched { .. }),
            "got {outcome:?}"
        ),
    }

    manager.stop().await.expect("stop");
}

/// Force on an HTTP-only core must fail the start with `RequiredLocalIpcUnsupported`.
async fn ipc_mode_force_rejected(core: RealCore) {
    let _serial = real::serial_lock().await;
    let bed = bed().await;
    let config = write_proxy_config(&bed, "");
    let manager = real::manager_with_policy(&bed.dir, LocalIpcPolicy::Force).await;
    let error = manager
        .start(real::real_spec(&core, &bed.dir, config))
        .await
        .expect_err("Force must reject HTTP-only cores");
    assert!(
        matches!(error, Error::RequiredLocalIpcUnsupported { .. }),
        "{}: got {error}",
        core.name
    );
}

macro_rules! ipc_mode_tests {
    ($force:ident, $prefer:ident, $http:ident, $core:expr) => {
        #[ignore = "requires the platform core binaries in tests/bin"]
        #[tokio::test]
        async fn $force() {
            ipc_mode_start($core, LocalIpcPolicy::Force, true).await;
        }
        #[ignore = "requires the platform core binaries in tests/bin"]
        #[tokio::test]
        async fn $prefer() {
            ipc_mode_start($core, LocalIpcPolicy::Prefer, true).await;
        }
        #[ignore = "requires the platform core binaries in tests/bin"]
        #[tokio::test]
        async fn $http() {
            ipc_mode_start($core, LocalIpcPolicy::Disable, false).await;
        }
    };
}

ipc_mode_tests!(
    mihomo_ipc_force,
    mihomo_ipc_prefer,
    mihomo_ipc_use_http,
    real::MIHOMO
);
ipc_mode_tests!(
    clash_rs_ipc_force,
    clash_rs_ipc_prefer,
    clash_rs_ipc_use_http,
    real::CLASH_RS
);

// HTTP-only cores: Force fails, Prefer falls back to HTTP, UseHttp is HTTP.
macro_rules! ipc_mode_http_only_tests {
    ($force:ident, $prefer:ident, $http:ident, $core:expr) => {
        #[ignore = "requires the platform core binaries in tests/bin"]
        #[tokio::test]
        async fn $force() {
            ipc_mode_force_rejected($core).await;
        }
        #[ignore = "requires the platform core binaries in tests/bin"]
        #[tokio::test]
        async fn $prefer() {
            ipc_mode_start($core, LocalIpcPolicy::Prefer, false).await;
        }
        #[ignore = "requires the platform core binaries in tests/bin"]
        #[tokio::test]
        async fn $http() {
            ipc_mode_start($core, LocalIpcPolicy::Disable, false).await;
        }
    };
}

ipc_mode_http_only_tests!(
    clash_premium_ipc_force,
    clash_premium_ipc_prefer,
    clash_premium_ipc_use_http,
    real::CLASH_PREMIUM
);
ipc_mode_http_only_tests!(
    meow_ipc_force,
    meow_ipc_prefer,
    meow_ipc_use_http,
    real::MEOW
);
