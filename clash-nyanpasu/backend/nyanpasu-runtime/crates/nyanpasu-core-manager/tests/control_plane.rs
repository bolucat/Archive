//! Contract tests for the control plane: admission, idempotency, cancellation
//! isolation, the closing latch, and the advisory check (audit §3.2 matrix,
//! executor rows).

mod common;

use std::time::Duration;

use camino::Utf8Path;
use nyanpasu_core_manager::{
    CheckRequest, ConfigInput, ControlOptions, CoreCommand, CoreCommandEnvelope, CoreControl,
    CoreErrorKind, CoreState, ExecutorExit, ManagerOptions, OperationId, OperationOutput,
    OperationState, ReconcileRequest,
    manager::{ApplyOutcome, CoreManager},
};

fn config_body(port: u16, extra: &str) -> String {
    format!("external-controller: 127.0.0.1:{port}\n{extra}")
}

async fn control_with(
    dir: &Utf8Path,
    tweak: impl FnOnce(ControlOptions) -> ControlOptions,
) -> CoreControl {
    let manager = CoreManager::new(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        ..ManagerOptions::default()
    })
    .await
    .expect("construct manager");
    let options = tweak(ControlOptions::new(dir.join("sources"), dir.to_owned()));
    CoreControl::spawn(manager, options)
}

async fn control(dir: &Utf8Path) -> CoreControl {
    control_with(dir, |options| options).await
}

fn reconcile_envelope(id: OperationId, dir: &Utf8Path, body: &str) -> CoreCommandEnvelope {
    let spec = common::mihomo_spec(dir, dir.join("unused.yaml"));
    CoreCommandEnvelope {
        operation_id: id,
        command: CoreCommand::Reconcile(Box::new(ReconcileRequest {
            core: spec.core,
            config: ConfigInput::inline(body.as_bytes().to_vec()),
            options: spec.options,
            expected_applied: None,
        })),
    }
}

fn command_envelope(command: CoreCommand) -> CoreCommandEnvelope {
    CoreCommandEnvelope {
        operation_id: OperationId::generate(),
        command,
    }
}

async fn wait_core_state(
    control: &CoreControl,
    pred: impl Fn(&CoreState) -> bool,
    what: &str,
) -> CoreState {
    let mut rx = control.subscribe();
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let current = rx.borrow_and_update().state.clone();
            if pred(&current) {
                return current;
            }
            rx.changed().await.expect("status channel closed");
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

#[tokio::test]
async fn a_submitted_reconcile_runs_to_started_and_stop_completes() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;

    let id = OperationId::generate();
    let handle = control
        .submit(reconcile_envelope(id, &dir, &config_body(port, "")))
        .unwrap();
    let output = handle.wait().await.unwrap();
    let OperationOutput::Reconciled(ApplyOutcome::Started { .. }) = output else {
        panic!("expected a cold start, got {output:?}");
    };
    assert!(matches!(control.status().state, CoreState::Running { .. }));
    assert!(matches!(
        control.operation(id),
        Some(OperationState::Succeeded(_))
    ));

    let output = control
        .submit(command_envelope(CoreCommand::Stop))
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(matches!(output, OperationOutput::Stopped));

    // Stopping again fails with the manager's own classification.
    let error = control
        .submit(command_envelope(CoreCommand::Stop))
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::NotStarted));

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn an_idempotent_resubmit_attaches_to_the_original_operation() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;
    let body = config_body(port, "");

    let id = OperationId::generate();
    let first = control.submit(reconcile_envelope(id, &dir, &body)).unwrap();
    let second = control.submit(reconcile_envelope(id, &dir, &body)).unwrap();
    assert!(first.newly_admitted(), "the first submit registers it");
    assert!(!second.newly_admitted(), "the second attaches to it");
    let first = first.wait().await.unwrap();
    let second = second.wait().await.unwrap();
    // One operation, one start: both callers observe the identical outcome.
    assert_eq!(first, second);
    let OperationOutput::Reconciled(ApplyOutcome::Started { revision }) = first else {
        panic!("expected a cold start, got {first:?}");
    };
    assert_eq!(
        revision.epoch,
        common::epoch(1),
        "a second start would have bumped the epoch"
    );

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn an_id_reuse_with_a_different_payload_conflicts_at_submit() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;

    let id = OperationId::generate();
    let handle = control
        .submit(reconcile_envelope(id, &dir, &config_body(port, "")))
        .unwrap();
    let error = control
        .submit(reconcile_envelope(
            id,
            &dir,
            &config_body(port, "mixed-port: 7899\n"),
        ))
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::OperationConflict));
    assert!(
        !error.retryable,
        "retrying the same conflict cannot succeed"
    );

    handle.wait().await.unwrap();
    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn a_full_queue_answers_queue_full() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control_with(&dir, |mut options| {
        options.queue_capacity = 1;
        options
    })
    .await;
    // The dry-run delay holds the executor busy long enough for the queue to
    // fill deterministically.
    let slow = config_body(port, "x-fake-core:\n  check-delay-ms: 1500\n");

    let first_id = OperationId::generate();
    let first = control
        .submit(reconcile_envelope(first_id, &dir, &slow))
        .unwrap();
    // The queue slot frees only once the executor dequeues the first
    // operation; wait for Running before filling the slot again.
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(control.operation(first_id), Some(OperationState::Running)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the first operation must start running");
    let second = control
        .submit(reconcile_envelope(OperationId::generate(), &dir, &slow))
        .unwrap();
    let rejected_id = OperationId::generate();
    let error = control
        .submit(reconcile_envelope(rejected_id, &dir, &slow))
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::QueueFull));
    assert!(error.retryable);
    // Admission is one critical section: a rejected enqueue leaves nothing
    // behind for a concurrent caller to attach to, and the id stays reusable.
    assert!(control.operation(rejected_id).is_none());

    first.wait().await.unwrap();
    let _ = second.wait().await;
    control
        .submit(reconcile_envelope(
            rejected_id,
            &dir,
            &config_body(port, ""),
        ))
        .expect("a queue-full rejection keeps its id submittable")
        .wait()
        .await
        .unwrap();
    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn dropping_the_handle_never_cancels_the_transaction() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;

    let handle = control
        .submit(reconcile_envelope(
            OperationId::generate(),
            &dir,
            &config_body(port, ""),
        ))
        .unwrap();
    drop(handle);

    // The transaction runs to its terminal state with nobody waiting.
    wait_core_state(
        &control,
        |state| matches!(state, CoreState::Running { .. }),
        "the abandoned reconcile to finish starting the core",
    )
    .await;

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn shutdown_latches_admission_and_drains_cleanly() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;

    let shutdown = control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap();
    let error = control
        .submit(reconcile_envelope(
            OperationId::generate(),
            &dir,
            &config_body(port, ""),
        ))
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::ShuttingDown));

    let output = shutdown.wait().await.unwrap();
    assert!(matches!(output, OperationOutput::ShutDown));
    assert_eq!(control.until_closed().await, ExecutorExit::Clean);
}

#[tokio::test]
async fn the_advisory_check_neither_queues_nor_disturbs_the_runtime() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;
    let spec = common::mihomo_spec(&dir, dir.join("unused.yaml"));

    let started = control
        .submit(reconcile_envelope(
            OperationId::generate(),
            &dir,
            &config_body(port, ""),
        ))
        .unwrap()
        .wait()
        .await
        .unwrap();
    let OperationOutput::Reconciled(ApplyOutcome::Started { revision }) = started else {
        panic!("expected a cold start");
    };

    control
        .check(CheckRequest {
            core: spec.core.clone(),
            config: ConfigInput::inline(config_body(port, "mixed-port: 7899\n").into_bytes()),
        })
        .await
        .expect("a valid config passes the advisory check");

    let error = control
        .check(CheckRequest {
            core: spec.core,
            config: ConfigInput::inline(
                config_body(port, "x-fake-core:\n  check-fail: port already in use\n").into_bytes(),
            ),
        })
        .await
        .expect_err("the core's dry-run rejection surfaces");
    assert_eq!(error.kind, Some(CoreErrorKind::ConfigCheckFailed));

    // Advisory means advisory: the runtime never moved.
    let status = control.status();
    assert!(matches!(status.state, CoreState::Running { .. }));
    assert_eq!(
        status.revision.as_ref().map(|revision| revision.id()),
        Some(revision.id())
    );

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn a_declared_digest_mismatch_aborts_before_anything_happens() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;
    let spec = common::mihomo_spec(&dir, dir.join("unused.yaml"));

    let envelope = CoreCommandEnvelope {
        operation_id: OperationId::generate(),
        command: CoreCommand::Reconcile(Box::new(ReconcileRequest {
            core: spec.core,
            config: ConfigInput::Inline {
                bytes: config_body(port, "").into_bytes(),
                expected_digest: Some("0000000000000000".to_owned()),
            },
            options: spec.options,
            expected_applied: None,
        })),
    };
    let error = control.submit(envelope).unwrap().wait().await.unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::InvalidConfig));
    assert!(matches!(control.status().state, CoreState::Stopped { .. }));

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

/// The declared digest is part of the operation's identity: correcting it
/// describes a different claim, so re-using the id is a conflict rather than a
/// replay of the first attempt's rejection.
#[tokio::test]
async fn a_corrected_declared_digest_conflicts_instead_of_attaching() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;
    let spec = common::mihomo_spec(&dir, dir.join("unused.yaml"));
    let bytes = config_body(port, "").into_bytes();
    let id = OperationId::generate();
    let envelope = |expected_digest: Option<String>| CoreCommandEnvelope {
        operation_id: id,
        command: CoreCommand::Reconcile(Box::new(ReconcileRequest {
            core: spec.core.clone(),
            config: ConfigInput::Inline {
                bytes: bytes.clone(),
                expected_digest,
            },
            options: spec.options.clone(),
            expected_applied: None,
        })),
    };

    let error = control
        .submit(envelope(Some("0000000000000000".to_owned())))
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::InvalidConfig));

    let error = control
        .submit(envelope(Some(nyanpasu_core_manager::payload_digest(
            &bytes,
        ))))
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::OperationConflict));

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

/// Two concurrent checks of the same bytes must not share one source file:
/// the first to finish would delete the config the other is still using.
#[tokio::test]
async fn same_digest_checks_use_distinct_source_files() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let control = control(&dir).await;
    let spec = common::mihomo_spec(&dir, dir.join("unused.yaml"));
    let started = dir.join("check-started");
    let body = config_body(
        port,
        &format!("x-fake-core:\n  check-delay-ms: 1500\n  check-started-file: '{started}'\n"),
    );
    let request = || CheckRequest {
        core: spec.core.clone(),
        config: ConfigInput::inline(body.clone().into_bytes()),
    };
    let sources = dir.join("sources");
    let source_count = || {
        std::fs::read_dir(&sources)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with("check-source")
                    })
                    .count()
            })
            .unwrap_or(0)
    };

    let first = tokio::spawn({
        let control = control.clone();
        let request = request();
        async move { control.check(request).await }
    });
    wait_until(|| started.exists(), "the first check to reach its core").await;
    let second = tokio::spawn({
        let control = control.clone();
        let request = request();
        async move { control.check(request).await }
    });
    wait_until(
        || source_count() == 2,
        "both checks to own a distinct source file",
    )
    .await;

    first.await.unwrap().expect("the first check completes");
    second.await.unwrap().expect("the second check completes");
    assert_eq!(source_count(), 0, "each check removes its own source file");

    control
        .submit(command_envelope(CoreCommand::Shutdown))
        .unwrap()
        .wait()
        .await
        .unwrap();
}

async fn wait_until(mut condition: impl FnMut() -> bool, what: &str) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

/// The registry, not the caller, owns operation state. `watch::Sender::send`
/// hands the value back as an error when nothing is subscribed and leaves the
/// stored one untouched, so a fire-and-forget submit -- which is exactly what
/// `POST /v2/core/submit` is -- would leave its operation `Queued` forever and
/// every later poll would subscribe to a state that stopped advancing.
#[tokio::test]
async fn an_operation_whose_handle_was_dropped_still_reaches_a_terminal_state() {
    let (_guard, dir) = common::utf8_tempdir();
    let control = control(&dir).await;

    let handle = control
        .submit(command_envelope(CoreCommand::Stop))
        .expect("the stop is admitted");
    let id = handle.id();
    drop(handle);

    wait_until(
        || {
            matches!(
                control.operation(id),
                Some(OperationState::Succeeded(_)) | Some(OperationState::Failed(_))
            )
        },
        "the dropped operation to reach a terminal state",
    )
    .await;
}

/// The executor's drain rests on three properties of a tokio mpsc, and a
/// change in any of them silently reopens the hole it closes: an operation
/// admitted through a reserved permit would land in a queue nobody reads and
/// never reach a terminal state.
#[tokio::test]
async fn a_closed_queue_still_delivers_an_outstanding_permit() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<u8>(4);
    let permit = tx.reserve().await.expect("the queue is open");
    rx.close();

    // 1. `try_recv` reports the queue empty while the permit is outstanding.
    assert!(rx.try_recv().is_err());

    // 2. `recv` waits for it rather than ending the drain early.
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
            .is_err()
    );

    // 3. The permit still delivers into a closed queue, and only then does the
    //    drain see the end.
    permit.send(7);
    assert_eq!(rx.recv().await, Some(7));
    assert_eq!(rx.recv().await, None);
}
