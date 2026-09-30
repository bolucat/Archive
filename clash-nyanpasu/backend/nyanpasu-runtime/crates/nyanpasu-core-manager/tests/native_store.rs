mod common;

use camino::Utf8Path;
use nyanpasu_core_manager::{
    CoreKind, CoreManager, CoreState, DegradeReason, LocalIpcPolicy, ManagerOptions, SwitchOutcome,
    native_store::{FsNativeStore, NativeStore, StoreOwner},
};
use std::{sync::Arc, time::Duration};

async fn manager(dir: &Utf8Path, runtime: &str) -> CoreManager {
    CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join(runtime)),
        local_ipc_policy: LocalIpcPolicy::Disable,
        ..ManagerOptions::default()
    })
    .native_store(Arc::new(FsNativeStore::new(
        dir.into(),
        StoreOwner::current(),
        None,
    )))
    .liveness_with_readiness_probe()
    .build()
    .await
    .unwrap()
}

#[tokio::test]
async fn managed_switch_stops_old_writer_and_releases_lease_for_another_host() {
    let (_temp, dir) = common::utf8_tempdir();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{}\n", common::free_port()),
    );
    let spec = common::mihomo_spec(&dir, config);
    let first = manager(&dir, "local").await;
    first.start(spec.clone()).await.unwrap();
    let CoreState::Running { pid: old, .. } = first.status().state else {
        panic!("not running")
    };
    let blocked = FsNativeStore::new(dir.clone(), StoreOwner::current(), None);
    assert!(blocked.acquire(CoreKind::Mihomo).is_err());
    assert_eq!(
        first.restart().await.unwrap(),
        SwitchOutcome::Hard {
            reason: DegradeReason::NativeStoreShared
        }
    );
    let CoreState::Running { pid: new, .. } = first.status().state else {
        panic!("not running")
    };
    assert_ne!(old, new);
    first.stop().await.unwrap();
    let second = manager(&dir, "service").await;
    second.start(spec).await.unwrap();
    second.shutdown().await.unwrap();
    first.shutdown().await.unwrap();
    assert!(!dir.join("native-store/active").exists());
    drop(blocked.acquire(CoreKind::Mihomo).unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn failed_spawn_releases_native_store_for_a_retry() {
    use std::os::unix::fs::PermissionsExt;
    let (_temp, dir) = common::utf8_tempdir();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{}\n", common::free_port()),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    let invalid_binary = dir.join("disable-after-check");
    // Pass the config check, then fail exec after native-store preparation.
    std::fs::write(&invalid_binary, b"#!/bin/sh\nchmod 600 \"$0\"\nexit 0\n").unwrap();
    std::fs::set_permissions(&invalid_binary, std::fs::Permissions::from_mode(0o755)).unwrap();
    spec.core.binary_path = invalid_binary;
    let manager = manager(&dir, "runtime").await;

    let error = manager.start(spec.clone()).await.unwrap_err();
    assert!(
        dir.join("native-store/v1/mihomo/cache.db").exists(),
        "{error}"
    );
    assert!(!dir.join("native-store/active").exists());
    let store = FsNativeStore::new(dir.clone(), StoreOwner::current(), None);
    drop(store.acquire(CoreKind::Mihomo).unwrap());

    spec.core.binary_path = common::fake_core_bin();
    manager.start(spec).await.unwrap();
    manager.shutdown().await.unwrap();
}

#[ignore = "requires MIHOMO_BIN, CLASH_RS_BIN and NYANPASU_TEST_RESOURCES"]
#[tokio::test]
async fn real_cores_keep_flushed_selections_across_hosts_and_core_switches() {
    use clash_api::{Client, Host};
    let (_temp, dir) = common::utf8_tempdir();
    let resources = std::env::var("NYANPASU_TEST_RESOURCES").expect("resource directory");
    for name in ["Country.mmdb", "geoip.dat", "geosite.dat"] {
        std::fs::copy(std::path::Path::new(&resources).join(name), dir.join(name)).unwrap();
    }
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nprofile: {{store-selected: true}}\nproxy-groups:\n  - name: Proxy\n    type: select\n    proxies: [DIRECT, REJECT]\nrules: ['MATCH,Proxy']\n"
        ),
    );

    let api = Client::new(Host::http(format!("127.0.0.1:{port}")).unwrap()).unwrap();
    let local = manager(&dir, "local").await;
    let mihomo = common::real::real_spec(&common::real::MIHOMO, &dir, config.clone());
    let rust = common::real::real_spec(&common::real::CLASH_RS, &dir, config);
    local.start(mihomo.clone()).await.unwrap();
    select_reject(&api).await;
    local.restart().await.unwrap();
    assert_eq!(selected(&api).await, "REJECT");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let CoreState::Running { pid, .. } = local.status().state else {
            panic!("not running")
        };
        std::fs::set_permissions(
            dir.join("native-store/v1/mihomo/cache.db"),
            std::fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        let mut states = local.subscribe();
        assert_eq!(unsafe { libc::kill(pid as i32, libc::SIGKILL) }, 0);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if matches!(states.borrow_and_update().state, CoreState::Running { pid: next, .. } if next != pid) { break; }
                states.changed().await.unwrap();
            }
        }).await.expect("automatic respawn");
        assert_eq!(selected(&api).await, "REJECT");
    }
    local.switch(rust.clone()).await.unwrap();
    assert_eq!(selected(&api).await, "DIRECT");
    select_reject(&api).await;
    // Observe actual native persistence instead of assuming a fixed flush delay.
    let rust_cache = dir.join("native-store/v1/clash-rs/cache.db");
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let raw = std::fs::read(&rust_cache).unwrap();
            if let Ok(doc) = serde_yaml_ng::from_slice::<serde_yaml_ng::Value>(&raw) {
                if doc["selected"]["Proxy"].as_str() == Some("REJECT") {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("clash-rs flushed its selection");
    local.switch(mihomo.clone()).await.unwrap();
    assert_eq!(selected(&api).await, "REJECT");
    local.stop().await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.join("native-store/v1/mihomo/cache.db"),
            std::fs::Permissions::from_mode(0o444),
        )
        .unwrap();
    }
    let service = manager(&dir, "service").await;
    service.start(mihomo).await.unwrap();
    assert_eq!(selected(&api).await, "REJECT");
    service.switch(rust).await.unwrap();
    assert_eq!(selected(&api).await, "REJECT");
    service.shutdown().await.unwrap();
    local.shutdown().await.unwrap();
}

async fn select_reject(api: &clash_api::Client) {
    api.put("/proxies/Proxy")
        .unwrap()
        .json(&serde_json::json!({"name": "REJECT"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}

async fn selected(api: &clash_api::Client) -> String {
    let proxy: serde_json::Value = api
        .get("/proxies/Proxy")
        .unwrap()
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    proxy["now"].as_str().unwrap().to_owned()
}

#[ignore = "requires MEOW_BIN"]
#[tokio::test]
async fn real_meow_migrates_and_retains_its_selector_json() {
    let (_temp, dir) = common::utf8_tempdir();
    std::fs::write(dir.join("selector-cache.json"), br#"{"Proxy":"REJECT"}"#).unwrap();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nproxy-groups:\n  - name: Proxy\n    type: select\n    proxies: [DIRECT, REJECT]\nrules: ['MATCH,Proxy']\n"
        ),
    );
    let manager = manager(&dir, "local").await;
    let api = clash_api::Client::new(clash_api::Host::http(format!("127.0.0.1:{port}")).unwrap())
        .unwrap();
    manager
        .start(common::real::real_spec(&common::real::MEOW, &dir, config))
        .await
        .unwrap();
    assert_eq!(selected(&api).await, "REJECT");
    manager.restart().await.unwrap();
    assert_eq!(selected(&api).await, "REJECT");
    manager.shutdown().await.unwrap();
    assert_eq!(
        std::fs::read(dir.join("native-store/v1/meow/selector-cache.json")).unwrap(),
        br#"{"Proxy":"REJECT"}"#
    );
}

#[tokio::test]
async fn cache_failure_remains_degraded_after_successful_controller_probes() {
    let (_temp, dir) = common::utf8_tempdir();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{}\nx-fake-core:\n  stdout-lines:\n    - 'time=\"2026-09-29T00:00:00Z\" level=warning msg=\"[CacheFile] write cache to /cache.db failed: permission denied\"'\n",
            common::free_port()
        ),
    );
    let manager = manager(&dir, "local").await;
    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .unwrap();
    let status = manager.status();
    assert!(matches!(status.state, CoreState::Running { .. }));
    let health = status.health.expect("health");
    assert_eq!(health.state, nyanpasu_core_manager::HealthState::Unhealthy);
    assert!(health.last_error.unwrap().contains("native store"));
    let mut states = manager.subscribe();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let next = states.borrow_and_update().health.clone().unwrap();
            assert_eq!(next.state, nyanpasu_core_manager::HealthState::Unhealthy);
            if next.last_success_at != health.last_success_at {
                break;
            }
            states.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    manager.shutdown().await.unwrap();
}
