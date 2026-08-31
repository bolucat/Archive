//! DNS override wiring: fixed-phase converge/restore, the persisted record,
//! orphan reconcile at construction, and the never-fail-the-transaction
//! policy — all against a fake controller.

mod common;

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use nyanpasu_core_manager::{
    CoreState, Error, ManagerOptions, RevisionId,
    dns::{DnsController, DnsError, DnsIntent, DnsOverrideRecord, DnsOverrideState},
    manager::{ApplyOutcome, CoreManager},
    runtime::BoxFuture,
};
use tokio::sync::Notify;

/// Stateful on purpose: `previous` has to be something the controller
/// *observed*, not a constant the fake hands back. A fake that always answers
/// with the original resolver would prove the baseline was kept even when the
/// orchestrator overwrote it.
struct FakeDns {
    applied: Mutex<Vec<(DnsIntent, u64)>>,
    restored: Mutex<Vec<DnsOverrideRecord>>,
    /// What the interface currently resolves through.
    current: Mutex<Vec<String>>,
    fail_apply: AtomicBool,
    fail_restore: AtomicBool,
    hang_apply: AtomicBool,
    hang_restore: AtomicBool,
    /// Never notified: a call that waits on it never returns.
    never: Notify,
}

impl Default for FakeDns {
    fn default() -> Self {
        Self {
            applied: Mutex::new(Vec::new()),
            restored: Mutex::new(Vec::new()),
            current: Mutex::new(vec!["10.0.0.1".to_owned()]),
            fail_apply: AtomicBool::new(false),
            fail_restore: AtomicBool::new(false),
            hang_apply: AtomicBool::new(false),
            hang_restore: AtomicBool::new(false),
            never: Notify::new(),
        }
    }
}

impl DnsController for FakeDns {
    fn desired(&self, _effective: &serde_yaml_ng::Mapping) -> Option<DnsIntent> {
        Some(DnsIntent {
            servers: vec!["198.18.0.2".to_owned()],
        })
    }

    fn apply<'a>(
        &'a self,
        intent: &'a DnsIntent,
        runtime_epoch: u64,
    ) -> BoxFuture<'a, Result<DnsOverrideRecord, DnsError>> {
        Box::pin(async move {
            if self.hang_apply.load(Ordering::SeqCst) {
                self.never.notified().await;
            }
            if self.fail_apply.load(Ordering::SeqCst) {
                return Err(DnsError::Command("injected apply failure".into()));
            }
            let previous =
                std::mem::replace(&mut *self.current.lock().unwrap(), intent.servers.clone());
            self.applied
                .lock()
                .unwrap()
                .push((intent.clone(), runtime_epoch));
            Ok(DnsOverrideRecord {
                interface: "Wi-Fi".to_owned(),
                previous,
                applied: intent.servers.clone(),
                runtime_epoch,
                owner_generation: None,
                state: DnsOverrideState::Applied,
            })
        })
    }

    fn restore<'a>(&'a self, record: &'a DnsOverrideRecord) -> BoxFuture<'a, Result<(), DnsError>> {
        Box::pin(async move {
            if self.hang_restore.load(Ordering::SeqCst) {
                self.never.notified().await;
            }
            if self.fail_restore.load(Ordering::SeqCst) {
                return Err(DnsError::Command("injected restore failure".into()));
            }
            self.restored.lock().unwrap().push(record.clone());
            *self.current.lock().unwrap() = record.previous.clone();
            Ok(())
        })
    }
}

async fn manager_with_dns(dir: &camino::Utf8Path, dns: Arc<FakeDns>) -> CoreManager {
    CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        ..ManagerOptions::default()
    })
    .dns_controller(dns)
    .build()
    .await
    .expect("construct manager")
}

async fn manager_with_dns_timeout(
    dir: &camino::Utf8Path,
    dns: Arc<FakeDns>,
    dns_timeout: Duration,
) -> CoreManager {
    CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        dns_timeout,
        ..ManagerOptions::default()
    })
    .dns_controller(dns)
    .build()
    .await
    .expect("construct manager")
}

fn record_path(dir: &camino::Utf8Path) -> camino::Utf8PathBuf {
    dir.join("runtime").join("dns-override.json")
}

#[tokio::test]
async fn reconcile_applies_the_override_and_stop_restores_it_first() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;

    let outcome = manager.reconcile(spec.clone(), None).await.unwrap();
    assert!(matches!(outcome, ApplyOutcome::Started { .. }));
    {
        let applied = dns.applied.lock().unwrap();
        assert_eq!(applied.len(), 1, "start tail applies exactly once");
        assert_eq!(applied[0].0.servers, vec!["198.18.0.2".to_owned()]);
        assert_eq!(applied[0].1, 1, "the record names the running epoch");
    }
    // The record survived to disk with the controller's read-back data.
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(record.state, DnsOverrideState::Applied);
    assert_eq!(record.previous, vec!["10.0.0.1".to_owned()]);

    manager.stop().await.unwrap();
    {
        let restored = dns.restored.lock().unwrap();
        assert_eq!(restored.len(), 1, "stop head restores exactly once");
        assert_eq!(restored[0].applied, vec!["198.18.0.2".to_owned()]);
    }
    assert!(
        !record_path(&dir).exists(),
        "a restored override leaves no record behind"
    );

    manager.shutdown().await.unwrap();
    assert_eq!(
        dns.restored.lock().unwrap().len(),
        1,
        "shutdown after a clean stop has nothing left to restore"
    );
}

#[tokio::test]
async fn a_noop_reconcile_reapplies_idempotently() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;

    manager.reconcile(spec.clone(), None).await.unwrap();
    let outcome = manager.reconcile(spec, None).await.unwrap();
    assert!(matches!(outcome, ApplyOutcome::Noop { .. }));
    // The converge tail runs each transaction; idempotency is the
    // controller's contract, observed here as a second harmless apply.
    assert_eq!(dns.applied.lock().unwrap().len(), 2);
    // ...and the baseline the first one captured is still the one on record.
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(record.previous, vec!["10.0.0.1".to_owned()]);

    manager.shutdown().await.unwrap();
    assert_eq!(*dns.current.lock().unwrap(), vec!["10.0.0.1".to_owned()]);
}

#[tokio::test]
async fn an_orphan_record_is_restored_at_construction() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let orphan = DnsOverrideRecord {
        interface: "Wi-Fi".to_owned(),
        previous: vec!["10.0.0.1".to_owned()],
        applied: vec!["198.18.0.2".to_owned()],
        runtime_epoch: 7,
        owner_generation: None,
        state: DnsOverrideState::Applied,
    };
    std::fs::write(
        runtime_dir.join("dns-override.json"),
        serde_json::to_vec(&orphan).unwrap(),
    )
    .unwrap();

    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;
    {
        let restored = dns.restored.lock().unwrap();
        assert_eq!(restored.len(), 1, "construction reconciles the orphan");
        assert_eq!(restored[0], orphan);
    }
    assert!(!record_path(&dir).exists());

    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_dns_apply_failure_never_fails_the_transaction() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    dns.fail_apply.store(true, Ordering::SeqCst);
    let manager = manager_with_dns(&dir, dns.clone()).await;

    let outcome = manager.reconcile(spec, None).await.unwrap();
    assert!(
        matches!(outcome, ApplyOutcome::Started { .. }),
        "the core transaction succeeds regardless of DNS"
    );
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    // The pre-record is kept: the side effect is uncertain and a later
    // restore must still be able to undo it.
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert!(
        record.previous.is_empty(),
        "a pre-record has no read-back yet"
    );

    manager.shutdown().await.unwrap();
    assert_eq!(
        dns.restored.lock().unwrap().len(),
        1,
        "shutdown restores the uncertain pre-record"
    );
    assert!(!record_path(&dir).exists());
}

/// Record before side effect is a crash-recovery guarantee, so the apply
/// direction is fail-closed: an override nothing recorded is an override
/// nothing can undo. The core transaction is unaffected either way.
#[tokio::test]
async fn an_unwritable_record_blocks_the_override_but_not_the_transaction() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;
    // A directory where the record belongs: the publish cannot succeed.
    std::fs::create_dir(record_path(&dir)).unwrap();

    let outcome = manager.reconcile(spec, None).await.unwrap();
    assert!(
        matches!(outcome, ApplyOutcome::Started { .. }),
        "the core transaction succeeds regardless of DNS"
    );
    assert!(matches!(manager.status().state, CoreState::Running { .. }));
    assert!(
        dns.applied.lock().unwrap().is_empty(),
        "no record, no side effect"
    );
    assert_eq!(*dns.current.lock().unwrap(), vec!["10.0.0.1".to_owned()]);

    manager.shutdown().await.unwrap();
}

/// The write is atomic, so a crash cannot leave a half-written record that
/// parses as nothing and undoes nothing.
#[tokio::test]
async fn the_published_record_parses_and_leaves_no_staging_file() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns).await;

    manager.reconcile(spec, None).await.unwrap();
    serde_json::from_slice::<DnsOverrideRecord>(&std::fs::read(record_path(&dir)).unwrap())
        .expect("the published record parses");
    assert!(
        !dir.join("runtime").join("dns-override.json.tmp").exists(),
        "the staging file is consumed by the replace"
    );

    manager.shutdown().await.unwrap();
}

/// The baseline is whatever resolution looked like *before* this process took
/// over. Re-reading it on every converge would record our own override as the
/// thing to restore to, and the original resolver would be lost after the
/// second transaction.
#[tokio::test]
async fn a_second_converge_keeps_the_original_previous() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;

    manager.reconcile(spec.clone(), None).await.unwrap();
    manager.reconcile(spec, None).await.unwrap();
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(record.previous, vec!["10.0.0.1".to_owned()]);

    manager.stop().await.unwrap();
    assert_eq!(
        *dns.current.lock().unwrap(),
        vec!["10.0.0.1".to_owned()],
        "the interface is handed back to the resolver it started with"
    );

    manager.shutdown().await.unwrap();
}

/// The converge tail runs under the control lock. An unbounded platform
/// command there freezes every other transaction, so each call is bounded and
/// a timeout is treated as an uncertain side effect.
#[tokio::test]
async fn a_hung_dns_apply_cannot_freeze_the_control_plane() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    dns.hang_apply.store(true, Ordering::SeqCst);
    let manager = manager_with_dns_timeout(&dir, dns.clone(), Duration::from_millis(200)).await;

    manager.reconcile(spec, None).await.unwrap();
    // The pre-record survives: the side effect may or may not have landed.
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(record.state, DnsOverrideState::Applied);

    dns.hang_apply.store(false, Ordering::SeqCst);
    manager.stop().await.unwrap();
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_hung_dns_restore_is_bounded() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns_timeout(&dir, dns.clone(), Duration::from_millis(200)).await;
    manager.reconcile(spec, None).await.unwrap();

    dns.hang_restore.store(true, Ordering::SeqCst);
    manager.stop().await.unwrap();
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(
        record.state,
        DnsOverrideState::RestorePending,
        "an unfinished restore stays on record for the next attempt"
    );
}

/// The orphan reconcile runs inside manager construction, before anything can
/// serve. It is bounded for the same reason the converge tail is.
#[tokio::test]
async fn a_hung_orphan_restore_does_not_block_construction() {
    let (_guard, dir) = common::utf8_tempdir();
    let runtime_dir = dir.join("runtime");
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let orphan = DnsOverrideRecord {
        interface: "Wi-Fi".to_owned(),
        previous: vec!["10.0.0.1".to_owned()],
        applied: vec!["198.18.0.2".to_owned()],
        runtime_epoch: 7,
        owner_generation: None,
        state: DnsOverrideState::Applied,
    };
    std::fs::write(record_path(&dir), serde_json::to_vec(&orphan).unwrap()).unwrap();
    let dns = Arc::new(FakeDns::default());
    dns.hang_restore.store(true, Ordering::SeqCst);

    let manager = manager_with_dns_timeout(&dir, dns, Duration::from_millis(200)).await;
    assert!(
        record_path(&dir).exists(),
        "an unfinished orphan restore keeps its record"
    );

    manager.shutdown().await.unwrap();
}

/// A clean abort changed nothing, so it must have no DNS side effect at all.
#[tokio::test]
async fn a_revision_conflict_never_touches_dns() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;
    manager.reconcile(spec.clone(), None).await.unwrap();
    let before = std::fs::read(record_path(&dir)).unwrap();

    let stale = RevisionId {
        epoch: 99,
        generation: 99,
        effective_hash: "deadbeef".to_owned(),
    };
    let error = manager.reconcile(spec, Some(stale)).await.unwrap_err();
    assert!(matches!(error, Error::RevisionConflict { .. }));
    assert_eq!(
        dns.applied.lock().unwrap().len(),
        1,
        "a refused transaction re-applies nothing"
    );
    assert_eq!(std::fs::read(record_path(&dir)).unwrap(), before);

    manager.shutdown().await.unwrap();
}

/// The other half of the same rule. A transaction that failed *and* left
/// nothing running is exactly where an override outlives the core it was
/// applied for, so the converge tail still runs and restores it. Skipping DNS
/// on every `Err` would leave the system resolving through a dead core until
/// the next successful operation.
#[tokio::test]
async fn a_failed_transaction_that_leaves_nothing_running_restores_dns() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let counter = dir.join("launch-count.txt").as_str().replace('\\', "/");
    let behavior =
        format!("x-fake-core:\n  launch-count-file: '{counter}'\n  fail-after-launches: 1\n");
    let first = dir.join("first.yaml");
    std::fs::write(
        &first,
        format!("external-controller: 127.0.0.1:{port}\nx-setting: old\n{behavior}"),
    )
    .unwrap();
    let desired = dir.join("desired.yaml");
    std::fs::write(
        &desired,
        format!(
            "external-controller: 127.0.0.1:{port}\nx-setting: desired\n{behavior}  exit-code: 23\n"
        ),
    )
    .unwrap();

    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;
    manager
        .reconcile(common::mihomo_spec(&dir, first), None)
        .await
        .expect("the first start succeeds");
    assert_eq!(dns.applied.lock().unwrap().len(), 1);

    // Both the apply and its rollback fail to launch, so no runtime survives.
    manager
        .reconcile(common::mihomo_spec(&dir, desired), None)
        .await
        .expect_err("both launch attempts fail");
    assert!(matches!(manager.status().state, CoreState::Stopped { .. }));

    assert_eq!(
        dns.restored.lock().unwrap().len(),
        1,
        "the override must not outlive the core it was applied for"
    );
    assert_eq!(*dns.current.lock().unwrap(), vec!["10.0.0.1".to_owned()]);
    assert!(!record_path(&dir).exists());

    manager.shutdown().await.unwrap();
}

/// A restore that did not succeed leaves the record `RestorePending` and the
/// override still in place. That record is still this process's proof of what
/// resolution looked like before it took over -- treating it as "no baseline"
/// makes the next converge read our own override back as `previous`, and the
/// restore after that pins the host at the core's resolver forever.
#[tokio::test]
async fn a_restore_pending_record_keeps_its_baseline() {
    let (_guard, dir) = common::utf8_tempdir();
    let dns = Arc::new(FakeDns::default());
    let manager = manager_with_dns(&dir, dns.clone()).await;

    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    manager
        .reconcile(common::mihomo_spec(&dir, config), None)
        .await
        .unwrap();
    assert_eq!(*dns.current.lock().unwrap(), vec!["198.18.0.2".to_owned()]);

    // The stop restores at its head, and the restore fails: the record is left
    // RestorePending with the override still active.
    dns.fail_restore.store(true, Ordering::SeqCst);
    manager.stop().await.unwrap();
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(record.state, DnsOverrideState::RestorePending);
    assert_eq!(record.previous, vec!["10.0.0.1".to_owned()]);
    assert_eq!(*dns.current.lock().unwrap(), vec!["198.18.0.2".to_owned()]);

    // Converging again must re-use that baseline rather than reading back the
    // override we ourselves installed.
    dns.fail_restore.store(false, Ordering::SeqCst);
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    manager
        .reconcile(common::mihomo_spec(&dir, config), None)
        .await
        .unwrap();
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert_eq!(
        record.previous,
        vec!["10.0.0.1".to_owned()],
        "the baseline survives a failed restore"
    );

    // And the proof that it matters: the restore lands on the original.
    manager.shutdown().await.unwrap();
    assert_eq!(*dns.current.lock().unwrap(), vec!["10.0.0.1".to_owned()]);
}

/// An interface with no resolvers is a baseline like any other. Reading an
/// empty `previous` as "nothing was captured" makes the next converge record
/// our own override instead, and the host never gets its empty list back.
#[tokio::test]
async fn an_empty_baseline_is_still_a_baseline() {
    let (_guard, dir) = common::utf8_tempdir();
    let dns = Arc::new(FakeDns::default());
    *dns.current.lock().unwrap() = Vec::new();
    let manager = manager_with_dns(&dir, dns.clone()).await;

    for _ in 0..2 {
        let port = common::free_port();
        let config =
            common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
        manager
            .reconcile(common::mihomo_spec(&dir, config), None)
            .await
            .unwrap();
    }
    let record: DnsOverrideRecord =
        serde_json::from_slice(&std::fs::read(record_path(&dir)).unwrap()).unwrap();
    assert!(
        record.previous.is_empty(),
        "the empty baseline must not be replaced by our own override, got {:?}",
        record.previous
    );

    manager.shutdown().await.unwrap();
    assert!(dns.current.lock().unwrap().is_empty());
}
