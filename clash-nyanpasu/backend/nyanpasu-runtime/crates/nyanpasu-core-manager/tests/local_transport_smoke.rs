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
    let config = common::write_config(&dir, &format!("external-controller-pipe: {pipe}\n"));
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
    let config = common::write_config(&dir, &format!("external-controller-unix: {socket}\n"));
    let mut child = tokio::process::Command::new(common::fake_core_bin())
        .args(["-m", "-d", dir.as_str(), "-f", config.as_str()])
        .kill_on_drop(true)
        .spawn()
        .expect("spawn");
    let client = Client::new_unix_socket(socket.as_str()).unwrap();
    wait_version(&client).await;
    child.kill().await.ok();
}
