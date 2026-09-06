//! Shared utilities for nyanpasu-core-manager integration tests.
#![allow(dead_code)]

pub mod real;
pub mod socks5;

use std::{sync::Arc, time::Duration};

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_core_manager::{
    CoreKind, CoreSpec, Epoch, HealthPolicy, HealthPolicySpec, HealthThresholds, InstanceOptions,
    InstanceSpec,
    state::{HealthState, InstanceState, InstanceStatus},
};
use nyanpasu_utils::process::{Backoff, BackoffRange, RestartPolicy};
use parking_lot::Mutex;
use tokio::sync::watch;

/// The epoch a test means when it writes a number. Deliberately not a
/// `From<u64>` on the type itself: production code must keep paying the
/// nonzero check at every boundary, and a test that skips it stops exercising
/// the barrier it is meant to prove.
pub fn epoch(value: u64) -> Epoch {
    Epoch::new(value).expect("a test epoch must be nonzero")
}

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
        health: HealthPolicy::new(HealthPolicySpec {
            interval: Duration::from_millis(50),
            timeout: Duration::from_secs(1),
            thresholds: HealthThresholds {
                failure: std::num::NonZeroU32::new(3).unwrap(),
                success: std::num::NonZeroU32::MIN,
            },
            start_period: Duration::ZERO,
        })
        .unwrap(),
        restart_policy: RestartPolicy::OnFailure { max_restarts: 2 },
        backoff: Backoff::exponential(BackoffRange {
            initial: Duration::from_millis(50),
            max: Duration::from_millis(200),
        }),
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
            version: Some("v1.18.9".into()),
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
    rx: &mut watch::Receiver<InstanceStatus>,
    pred: impl Fn(&InstanceState) -> bool,
    timeout: Duration,
) -> InstanceState {
    tokio::time::timeout(timeout, async {
        loop {
            let current = rx.borrow_and_update().state.clone();
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

pub async fn wait_for_health(
    rx: &mut watch::Receiver<InstanceStatus>,
    target: HealthState,
    timeout: Duration,
) -> InstanceStatus {
    tokio::time::timeout(timeout, async {
        loop {
            let current = rx.borrow_and_update().clone();
            if current
                .health
                .as_ref()
                .is_some_and(|health| health.state == target)
            {
                return current;
            }
            if rx.changed().await.is_err() {
                panic!("state channel closed while waiting for health");
            }
        }
    })
    .await
    .expect("timed out waiting for health")
}

/// Records every state transition for later sequence assertions.
pub fn record_states(
    mut rx: watch::Receiver<InstanceStatus>,
) -> (tokio::task::JoinHandle<()>, Arc<Mutex<Vec<InstanceState>>>) {
    let log = Arc::new(Mutex::new(vec![rx.borrow().state.clone()]));
    let log_ = log.clone();
    let handle = tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            log_.lock().push(rx.borrow().state.clone());
        }
    });
    (handle, log)
}

/// Asserts the process behind `port` is gone by polling until connect is refused.
pub async fn wait_port_refused(port: u16) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_err()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("port was never released");
}
