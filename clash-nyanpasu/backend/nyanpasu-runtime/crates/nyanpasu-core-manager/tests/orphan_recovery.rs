mod common;

use std::{collections::HashMap, process::Stdio, time::Duration};

use nyanpasu_core_manager::{CoreManager, CoreState, ManagerOptions};

struct HostProcess(std::process::Child);

impl HostProcess {
    fn hard_kill(&mut self) {
        self.0.kill().expect("hard-kill manager host");
        self.0.wait().expect("wait for killed manager host");
    }
}

impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn parse_ready(contents: &str) -> HashMap<&str, &str> {
    contents
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect()
}

#[tokio::test]
async fn hard_killed_manager_is_reaped_and_all_epoch_artifacts_are_swept() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    let ready_file = dir.join("manager-ready.txt");
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut host = HostProcess(
        std::process::Command::new(env!("CARGO_BIN_EXE_nyanpasu-manager-host"))
            .arg(&runtime_dir)
            .arg(&config)
            .arg(common::fake_core_bin())
            .arg(&ready_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn manager host"),
    );

    let ready = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Ok(contents) = tokio::fs::read_to_string(&ready_file).await {
                let values = parse_ready(&contents);
                if values.contains_key("epoch") && values.contains_key("core_pid") {
                    break contents;
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("manager host never became ready");
    let values = parse_ready(&ready);
    let swept_epoch: u64 = values["epoch"].parse().unwrap();
    let core_pid: u32 = values["core_pid"].parse().unwrap();
    let runtime_path = camino::Utf8Path::new(values["runtime_path"]);
    let pid_path = camino::Utf8Path::new(values["pid_path"]);
    assert_eq!(swept_epoch, 1);
    assert!(core_pid > 0);
    assert_eq!(runtime_path.file_name(), Some("config-1.yaml"));
    assert_eq!(pid_path.file_name(), Some("core-1.pid"));
    assert_eq!(
        std::fs::canonicalize(runtime_path.parent().unwrap()).unwrap(),
        std::fs::canonicalize(&runtime_dir).unwrap()
    );
    assert_eq!(
        std::fs::canonicalize(pid_path.parent().unwrap()).unwrap(),
        std::fs::canonicalize(&runtime_dir).unwrap()
    );
    assert!(runtime_path.is_file());
    assert!(pid_path.is_file());
    tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("hosted core serves before manager kill");

    std::fs::write(
        runtime_dir.join(format!("config-{swept_epoch}.yaml.backup-99")),
        "backup",
    )
    .unwrap();
    std::fs::write(
        runtime_dir.join(format!(".config-{swept_epoch}.yaml.tmp-orphan")),
        "temporary",
    )
    .unwrap();
    #[cfg(windows)]
    std::fs::write(
        runtime_dir.join(format!("core-{swept_epoch}.sock")),
        "socket-placeholder",
    )
    .unwrap();
    #[cfg(unix)]
    {
        let socket = std::os::unix::net::UnixListener::bind(
            runtime_dir.join(format!("core-{swept_epoch}.sock")),
        )
        .unwrap();
        drop(socket);
    }

    host.hard_kill();
    // A Windows Job Object may eagerly kill the core when the host's final job
    // handle closes. Unix process groups leave it alive. Recovery must handle
    // both states and still sweep the durable epoch artifacts.
    let _orphan_was_alive = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok();

    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir.clone()),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct recovery manager");
    common::wait_port_refused(port).await;

    let mut entries = tokio::fs::read_dir(&runtime_dir).await.unwrap();
    let mut leftovers = Vec::new();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        leftovers.push(entry.file_name().to_string_lossy().into_owned());
    }
    assert_eq!(leftovers, [".manager.lock"], "unswept artifacts");

    manager
        .start(common::mihomo_spec(&dir, config))
        .await
        .expect("start after orphan sweep");
    let CoreState::Running { epoch, .. } = manager.status().state else {
        panic!("recovery core is not running")
    };
    assert!(
        epoch > swept_epoch,
        "epoch {epoch} must exceed {swept_epoch}"
    );
    manager.shutdown().await.expect("shutdown recovery core");
}
