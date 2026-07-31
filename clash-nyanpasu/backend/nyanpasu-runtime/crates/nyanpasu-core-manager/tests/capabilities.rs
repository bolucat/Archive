mod common;

#[cfg(windows)]
use std::sync::atomic::{AtomicU32, Ordering};
use std::{
    fs::{FileTimes, OpenOptions},
    time::Duration,
};

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    CoreKind, CoreManager, CoreState, Error, Feature, Host, LocalIpcPolicy, ManagerOptions,
    RuntimeFeature,
};

fn unique_template() -> Option<String> {
    #[cfg(windows)]
    {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
        Some(format!(
            r"\\.\pipe\nyanpasu-capability-{}-{sequence}-{{epoch}}",
            std::process::id()
        ))
    }
    #[cfg(not(windows))]
    {
        None // unix default derives the socket under the runtime dir (already unique)
    }
}

async fn manager_with_policy(runtime_dir: Utf8PathBuf, policy: LocalIpcPolicy) -> CoreManager {
    CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir),
        local_ipc_policy: policy,
        controller_template: unique_template(),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct managed manager")
}

fn platform_local_feature() -> Feature {
    #[cfg(windows)]
    {
        Feature::NamedPipeIpc
    }
    #[cfg(not(windows))]
    {
        Feature::UnixSocketIpc
    }
}

fn unsupported_mihomo_version() -> &'static str {
    #[cfg(windows)]
    {
        "1.18.8"
    }
    #[cfg(not(windows))]
    {
        "1.18.3"
    }
}

fn source_local_controller(_dir: &Utf8Path) -> String {
    #[cfg(windows)]
    {
        format!(
            r"external-controller-pipe: \\.\pipe\nyanpasu-source-{}-{}",
            std::process::id(),
            common::free_port()
        )
    }
    #[cfg(not(windows))]
    {
        format!(
            "external-controller-unix: {}",
            _dir.join("source-controller.sock")
        )
    }
}

fn copied_probe_binary(dir: &Utf8Path) -> Utf8PathBuf {
    std::fs::create_dir_all(dir).expect("create probe binary directory");
    let binary = dir.join(format!(
        "nyanpasu-version-probe-core{}",
        std::env::consts::EXE_SUFFIX
    ));
    std::fs::copy(common::fake_core_bin(), &binary).expect("copy fake core");
    binary
}

fn probe_counter(binary: &Utf8Path) -> Utf8PathBuf {
    binary.with_extension("version-probes")
}

fn probe_count(binary: &Utf8Path) -> u64 {
    std::fs::read_to_string(probe_counter(binary))
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

#[tokio::test]
async fn force_rejects_unsupported_mihomo_before_staging_check_or_spawn() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    let check_started = dir.join("check-started");
    let launches = dir.join("launches");
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  check-started-file: '{}'\n  launch-count-file: '{}'\n  fail-after-launches: 99\n",
            check_started.as_str().replace('\\', "/"),
            launches.as_str().replace('\\', "/"),
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.core.version = Some(unsupported_mihomo_version().into());
    let manager = manager_with_policy(runtime.clone(), LocalIpcPolicy::Force).await;

    let error = manager.start(spec).await.expect_err("Force must reject");

    assert!(matches!(
        error,
        Error::RequiredLocalIpcUnsupported {
            kind: CoreKind::Mihomo,
            ref version,
        } if version == unsupported_mihomo_version()
    ));
    assert!(!runtime.join("config-1.yaml").exists());
    assert!(!check_started.exists(), "config check unexpectedly ran");
    assert!(!launches.exists(), "core process unexpectedly started");
    assert!(matches!(
        manager.status().state,
        CoreState::Stopped { reason: Some(_) }
    ));
}

#[tokio::test]
async fn force_rejects_clash_premium_without_probing_its_version() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    let binary = copied_probe_binary(&dir);
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.core.kind = CoreKind::ClashPremium;
    spec.core.binary_path = binary.clone();
    spec.core.version = None;
    let manager = manager_with_policy(runtime.clone(), LocalIpcPolicy::Force).await;

    let error = manager.start(spec).await.expect_err("Force must reject");

    assert!(matches!(
        error,
        Error::RequiredLocalIpcUnsupported {
            kind: CoreKind::ClashPremium,
            ..
        }
    ));
    assert_eq!(probe_count(&binary), 0);
    assert!(!runtime.join("config-1.yaml").exists());
}

#[tokio::test]
async fn prefer_falls_back_to_the_upstream_http_controller_and_secret() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime = dir.join("runtime");
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\n{}\nsecret: upstream-secret\n",
            source_local_controller(&dir)
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.core.version = Some(unsupported_mihomo_version().into());
    let manager = manager_with_policy(runtime, LocalIpcPolicy::Prefer).await;

    manager.start(spec).await.expect("Prefer HTTP fallback");

    let status = manager.status();
    assert!(matches!(
        status.controller.as_ref(),
        Some(Host::Http(url)) if url.as_str() == format!("http://127.0.0.1:{port}/")
    ));
    let summary = status.spec.as_ref().expect("spec summary");
    assert!(!summary.capabilities.contains(&platform_local_feature()));
    assert!(!summary.runtime_features.contains(&RuntimeFeature::LocalIpc));
    let runtime = std::fs::read_to_string(&status.revision.as_ref().unwrap().runtime_path).unwrap();
    assert!(runtime.contains(&format!("external-controller: 127.0.0.1:{port}")));
    assert!(runtime.contains("secret: upstream-secret"));
    assert!(!format!("{status:?}").contains("upstream-secret"));
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn disable_uses_http_even_when_local_ipc_is_supported() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\n{}\n",
            source_local_controller(&dir)
        ),
    );
    let manager = manager_with_policy(dir.join("runtime"), LocalIpcPolicy::Disable).await;

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("Disable start");

    let status = manager.status();
    assert!(matches!(status.controller, Some(Host::Http(_))));
    let summary = status.spec.as_ref().unwrap();
    // The capability is still reported even though policy keeps it off.
    assert!(summary.capabilities.contains(&platform_local_feature()));
    assert!(!summary.runtime_features.contains(&RuntimeFeature::LocalIpc));
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn force_uses_only_local_ipc_and_retains_the_secret() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nsecret: retained-secret\n"),
    );
    let manager = manager_with_policy(dir.join("runtime"), LocalIpcPolicy::Force).await;

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("Force local start");

    let status = manager.status();
    #[cfg(windows)]
    assert!(matches!(status.controller, Some(Host::NamedPipe(_))));
    #[cfg(not(windows))]
    assert!(matches!(status.controller, Some(Host::UnixSocket(_))));
    let summary = status.spec.as_ref().unwrap();
    assert!(summary.capabilities.contains(&platform_local_feature()));
    assert!(
        !summary
            .capabilities
            .contains(&Feature::DisableTcpController)
    );
    assert!(summary.runtime_features.contains(&RuntimeFeature::LocalIpc));
    let runtime = std::fs::read_to_string(status.revision.unwrap().runtime_path).unwrap();
    assert!(!runtime.contains(&format!("external-controller: 127.0.0.1:{port}")));
    assert!(runtime.contains("secret: retained-secret"));
    #[cfg(windows)]
    assert!(runtime.contains("external-controller-pipe:"));
    #[cfg(not(windows))]
    assert!(runtime.contains("external-controller-unix:"));
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn version_probe_is_cached_by_path_and_mtime_and_supplied_versions_skip_it() {
    let (_guard, dir) = common::utf8_tempdir();
    let binary = copied_probe_binary(&dir);
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut probed = common::mihomo_spec(&dir, config.clone());
    probed.core.binary_path = binary.clone();
    probed.core.version = None;
    let manager = manager_with_policy(dir.join("runtime"), LocalIpcPolicy::Force).await;

    manager.start(probed.clone()).await.unwrap();
    manager.stop().await.unwrap();
    manager.start(probed.clone()).await.unwrap();
    manager.stop().await.unwrap();
    assert_eq!(probe_count(&binary), 1);

    let file = OpenOptions::new().write(true).open(&binary).unwrap();
    let modified = std::fs::metadata(&binary)
        .unwrap()
        .modified()
        .unwrap()
        .checked_add(Duration::from_secs(2))
        .unwrap();
    file.set_times(FileTimes::new().set_modified(modified))
        .unwrap();
    drop(file);
    manager.start(probed).await.unwrap();
    manager.stop().await.unwrap();
    assert_eq!(probe_count(&binary), 2);

    let supplied_binary = copied_probe_binary(&dir.join("supplied"));
    let mut supplied = common::mihomo_spec(&dir, config);
    supplied.core.binary_path = supplied_binary.clone();
    let supplied_manager =
        manager_with_policy(dir.join("supplied-runtime"), LocalIpcPolicy::Force).await;
    supplied_manager.start(supplied).await.unwrap();
    supplied_manager.shutdown().await.unwrap();
    assert_eq!(probe_count(&supplied_binary), 0);
}
