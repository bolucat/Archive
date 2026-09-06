//! The unified `reconcile` entry: cold start, convergence dispatch, CAS, and
//! the clean-abort guarantee of the internal check.

mod common;

use nyanpasu_core_manager::{
    CoreState, Error, ManagerOptions, RevisionId,
    manager::{ApplyOutcome, CoreManager},
};

async fn manager(dir: &camino::Utf8Path) -> CoreManager {
    CoreManager::new(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct manager")
}

#[tokio::test]
async fn reconcile_starts_cold_then_noops_on_the_same_config() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let manager = manager(&dir).await;

    let outcome = manager.reconcile(spec.clone(), None).await.unwrap();
    let ApplyOutcome::Started { revision } = outcome else {
        panic!("cold reconcile must report Started, got {outcome:?}");
    };
    assert!(matches!(manager.status().state, CoreState::Running { .. }));

    // The same source config converges to Noop, gated by the real revision.
    let outcome = manager.reconcile(spec, Some(revision.id())).await.unwrap();
    assert!(matches!(outcome, ApplyOutcome::Noop { .. }));

    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn reconcile_with_a_stale_expectation_changes_nothing() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let manager = manager(&dir).await;

    let ApplyOutcome::Started { revision } = manager.reconcile(spec.clone(), None).await.unwrap()
    else {
        panic!("cold reconcile must report Started");
    };
    let running = manager.status();

    let stale = RevisionId {
        epoch: common::epoch(99),
        generation: 9,
        effective_hash: "deadbeefdeadbeef".to_owned(),
    };
    let error = manager
        .reconcile(spec, Some(stale))
        .await
        .expect_err("stale expectation must conflict");
    let Error::RevisionConflict { actual, .. } = error else {
        panic!("expected RevisionConflict, got {error}");
    };
    assert_eq!(actual, Some(revision.id()));
    // Zero side effects: the runtime kept running on the same revision.
    let status = manager.status();
    assert_eq!(status.state, running.state);
    assert_eq!(
        status.revision.as_ref().map(|revision| revision.id()),
        Some(revision.id())
    );

    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn reconcile_against_a_stopped_manager_rejects_a_believed_revision() {
    let (_guard, dir) = common::utf8_tempdir();
    let manager = manager(&dir).await;
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let expected = RevisionId {
        epoch: common::epoch(1),
        generation: 1,
        effective_hash: "0123456789abcdef".to_owned(),
    };
    let error = manager
        .reconcile(spec, Some(expected))
        .await
        .expect_err("a believed revision cannot match a stopped manager");
    let Error::RevisionConflict { actual, .. } = error else {
        panic!("expected RevisionConflict, got {error}");
    };
    assert_eq!(actual, None);
    // The conflict must not have started anything.
    assert!(matches!(manager.status().state, CoreState::Stopped { .. }));

    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_rejected_config_aborts_cleanly_while_the_old_core_keeps_running() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);
    let manager = manager(&dir).await;

    let ApplyOutcome::Started { revision } = manager.reconcile(spec.clone(), None).await.unwrap()
    else {
        panic!("cold reconcile must report Started");
    };

    // Semantic rejection from the core's own dry run, inside the transaction.
    let rejected = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  check-fail: port already in use\n"
        ),
    );
    let mut rejected_spec = spec.clone();
    rejected_spec.config_path = rejected;
    let error = manager
        .reconcile(rejected_spec, Some(revision.id()))
        .await
        .expect_err("the dry-run rejection must abort the transaction");
    assert!(matches!(error, Error::ConfigCheckFailed(_)), "{error}");

    // Clean abort: the old core is still running on the old revision.
    let status = manager.status();
    assert!(matches!(status.state, CoreState::Running { .. }));
    assert_eq!(
        status.revision.as_ref().map(|revision| revision.id()),
        Some(revision.id())
    );

    // A parse failure aborts just as cleanly, without touching a process.
    let malformed = common::write_config(&dir, "external-controller: [unclosed\n");
    let mut malformed_spec = spec.clone();
    malformed_spec.config_path = malformed;
    let error = manager
        .reconcile(malformed_spec, Some(revision.id()))
        .await
        .expect_err("a malformed config must abort the transaction");
    assert!(
        matches!(error, Error::Yaml(_) | Error::InvalidConfig(_)),
        "{error}"
    );
    let status = manager.status();
    assert!(matches!(status.state, CoreState::Running { .. }));
    assert_eq!(
        status.revision.as_ref().map(|revision| revision.id()),
        Some(revision.id())
    );

    manager.shutdown().await.unwrap();
}
