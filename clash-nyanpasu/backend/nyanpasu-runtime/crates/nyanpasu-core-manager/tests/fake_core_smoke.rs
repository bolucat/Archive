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
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  ready-delay-ms: 1500\n  patch-log: {patch_log}\n"
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
        .patch_config(&ConfigPatch {
            mixed_port: Some(0),
            ..Default::default()
        })
        .await
        .expect("patch accepted");
    let log = std::fs::read_to_string(&patch_log).expect("patch log written");
    assert!(log.contains("\"mixed-port\":0"), "log was: {log}");

    child.kill().await.ok();
}
