//! External test host used to simulate a manager process disappearing without
//! running Rust destructors.

use std::future::pending;

use camino::Utf8PathBuf;
use nyanpasu_core_manager::{
    CoreKind, CoreManager, CoreSpec, CoreState, InstanceOptions, InstanceSpec, ManagerOptions,
};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let runtime_dir = Utf8PathBuf::from(args.next().expect("runtime dir"));
    let source_config = Utf8PathBuf::from(args.next().expect("source config"));
    let core_binary = Utf8PathBuf::from(args.next().expect("core binary"));
    let ready_file = Utf8PathBuf::from(args.next().expect("ready file"));
    let working_dir = source_config.parent().expect("config parent").to_owned();

    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(runtime_dir.clone()),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct hosted manager");
    manager
        .start(InstanceSpec {
            core: CoreSpec {
                kind: CoreKind::Mihomo,
                binary_path: core_binary,
                version: None,
                features: Vec::new(),
            },
            config_path: source_config,
            working_dir,
            pid_file: None,
            options: InstanceOptions::default(),
        })
        .await
        .expect("start hosted core");
    let status = manager.status();
    let CoreState::Running { epoch, pid } = status.state else {
        panic!("hosted core is not running")
    };
    let revision = status.revision.expect("hosted revision");
    let record = format!(
        "host_pid={}\nepoch={epoch}\ncore_pid={pid}\nruntime_path={}\npid_path={}\n",
        std::process::id(),
        revision.runtime_path,
        runtime_dir.join(format!("core-{epoch}.pid")),
    );
    std::fs::write(ready_file, record).expect("publish host readiness");

    let _manager = manager;
    pending::<()>().await;
}
