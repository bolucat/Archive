//! Smoke tests against a real mihomo binary, complementing the fake-core suite.
//! Prepare the binary with `deno run -A scripts/prepare-mihomo.ts`, then run:
//! `cargo test -p nyanpasu-core-manager --test real_mihomo_smoke -- --ignored --nocapture`.

mod common;

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    CoreKind, CoreSpec, Error, Instance, InstanceOptions, InstanceSpec, kind::check_config,
    spec::ResolvedController, state::InstanceState,
};
use tokio_util::sync::CancellationToken;

fn workspace_root() -> Utf8PathBuf {
    let manifest = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(Utf8Path::parent)
        .expect("crate lives in <workspace>/crates")
        .to_owned()
}

fn real_mihomo_bin() -> Utf8PathBuf {
    let binary = std::env::var_os("MIHOMO_BIN")
        .map(|p| Utf8PathBuf::from_path_buf(p.into()).expect("MIHOMO_BIN must be UTF-8"))
        .unwrap_or_else(|| {
            workspace_root()
                .join("tests/bin")
                .join(format!("mihomo{}", std::env::consts::EXE_SUFFIX))
        });
    assert!(
        binary.is_file(),
        "mihomo was not found at {binary}; run `deno run -A scripts/prepare-mihomo.ts` or set MIHOMO_BIN"
    );
    binary
}

fn real_spec(dir: &Utf8Path, config_path: Utf8PathBuf) -> InstanceSpec {
    InstanceSpec {
        core: CoreSpec {
            kind: CoreKind::Mihomo,
            binary_path: real_mihomo_bin(),
            version: None,
            features: Vec::new(),
        },
        config_path,
        working_dir: dir.to_owned(),
        pid_file: None,
        options: InstanceOptions {
            startup_timeout: Duration::from_secs(15),
            ..common::fast_options()
        },
    }
}

#[ignore = "requires the platform mihomo binary in tests/bin"]
#[tokio::test]
async fn real_core_starts_probes_and_stops() {
    let (_guard, dir) = common::utf8_tempdir();
    let ctrl_port = common::free_port();
    let mixed_port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("mixed-port: {mixed_port}\nexternal-controller: 127.0.0.1:{ctrl_port}\n"),
    );
    let spec = real_spec(&dir, config);

    check_config(&spec)
        .await
        .expect("real core accepts the config");

    let controller = ResolvedController {
        host: clash_api::Host::http(format!("127.0.0.1:{ctrl_port}")).unwrap(),
        secret: None,
    };
    let instance = Instance::spawn(spec, 1, controller, CancellationToken::new())
        .await
        .expect("spawn");
    instance
        .wait_ready()
        .await
        .expect("real mihomo passes the version probe");
    assert!(matches!(
        instance.state().borrow().state,
        InstanceState::Running { pid } if pid > 0
    ));

    instance.stop().await.expect("stop");
    common::wait_port_refused(ctrl_port).await;
}

#[ignore = "requires the platform mihomo binary in tests/bin"]
#[tokio::test]
async fn real_core_check_config_reports_invalid_config() {
    let (_guard, dir) = common::utf8_tempdir();
    let bad_config = common::write_config(&dir, "mixed-port: not-a-port\n");
    let spec = real_spec(&dir, bad_config);

    let err = check_config(&spec).await.expect_err("must fail");
    match err {
        Error::ConfigCheckFailed(msg) => {
            assert!(!msg.is_empty(), "error message should be condensed");
        }
        other => panic!("unexpected error: {other}"),
    }
}
