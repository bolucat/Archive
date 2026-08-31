//! The trait-boundary proof: the full control plane runs against an
//! in-memory backend with no child processes, and the StopProof→Quarantine
//! contract rows hold (audit §3.2).

mod common;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use nyanpasu_core_manager::{
    ConfigInput, ControlOptions, CoreCommand, CoreCommandEnvelope, CoreControl, CoreErrorKind,
    Error, ManagerOptions, OperationId, OperationOutput, OperationState, ProbePhase, ProbeResult,
    ReconcileRequest,
    manager::{ApplyOutcome, CoreManager},
    runtime::{BoxFuture, RuntimeBackend, RuntimeInstance, RuntimeLaunchRequest},
    spec::{InstanceSpec, ResolvedController},
    state::{InstanceState, InstanceStatus, StopReason},
};
use tokio::sync::{Notify, watch};

#[derive(Default)]
struct FakeBackend {
    refuse_stop: Arc<AtomicBool>,
    launched_epochs: Mutex<Vec<u64>>,
    /// Holds the first launch inside the executor so a test can fill the queue
    /// behind a transaction that is provably running.
    gate_launch: AtomicBool,
    gate_taken: AtomicBool,
    launch_started: Notify,
    release_launch: Notify,
    /// Injected here rather than in the control plane: the executor's panic
    /// boundary must be provable without a production-side hook.
    panic_on_launch: AtomicBool,
}

impl RuntimeBackend for FakeBackend {
    fn launch(
        &self,
        request: RuntimeLaunchRequest,
    ) -> BoxFuture<'_, Result<Box<dyn RuntimeInstance>, Error>> {
        Box::pin(async move {
            self.launched_epochs.lock().unwrap().push(request.epoch);
            if self.gate_launch.load(Ordering::SeqCst)
                && !self.gate_taken.swap(true, Ordering::SeqCst)
            {
                self.launch_started.notify_one();
                self.release_launch.notified().await;
            }
            assert!(
                !self.panic_on_launch.load(Ordering::SeqCst),
                "injected launch panic"
            );
            let (state_tx, _) = watch::channel(InstanceStatus {
                state: InstanceState::Starting,
                health: None,
            });
            Ok(Box::new(FakeInstance {
                spec: request.effective_spec,
                epoch: request.epoch,
                controller: request.controller,
                state_tx,
                refuse_stop: self.refuse_stop.clone(),
            }) as Box<dyn RuntimeInstance>)
        })
    }

    fn check_config<'a>(&'a self, _spec: &'a InstanceSpec) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async { Ok(()) })
    }
}

struct FakeInstance {
    spec: InstanceSpec,
    epoch: u64,
    controller: ResolvedController,
    state_tx: watch::Sender<InstanceStatus>,
    refuse_stop: Arc<AtomicBool>,
}

impl RuntimeInstance for FakeInstance {
    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn spec(&self) -> &InstanceSpec {
        &self.spec
    }

    fn controller(&self) -> &ResolvedController {
        &self.controller
    }

    fn pid(&self) -> Option<u32> {
        // An embedded runtime has no process; the portable model must cope.
        None
    }

    fn state(&self) -> watch::Receiver<InstanceStatus> {
        self.state_tx.subscribe()
    }

    fn wait_ready<'a>(&'a self) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(async move {
            let _ = self.state_tx.send(InstanceStatus {
                state: InstanceState::Running { pid: 0 },
                health: None,
            });
            Ok(())
        })
    }

    fn probe_now<'a>(&'a self, _phase: ProbePhase) -> BoxFuture<'a, ProbeResult> {
        Box::pin(async { ProbeResult::Healthy })
    }

    fn stop_and_confirm_dead(
        self: Box<Self>,
        _timeout: std::time::Duration,
    ) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(async move {
            if self.refuse_stop.load(Ordering::SeqCst) {
                return Err(Error::StopUnconfirmed(
                    "injected: death cannot be proven".into(),
                ));
            }
            let _ = self.state_tx.send(InstanceStatus {
                state: InstanceState::Stopped(StopReason::User),
                health: None,
            });
            Ok(())
        })
    }
}

async fn control_with_backend(dir: &camino::Utf8Path, backend: Arc<FakeBackend>) -> CoreControl {
    let manager = CoreManager::builder(ManagerOptions {
        runtime_dir: Some(dir.join("runtime")),
        ..ManagerOptions::default()
    })
    .runtime_backend(backend)
    .build()
    .await
    .expect("construct manager");
    CoreControl::spawn(
        manager,
        ControlOptions::new(dir.join("sources"), dir.to_owned()),
    )
}

fn reconcile_envelope(dir: &camino::Utf8Path, binary: camino::Utf8PathBuf) -> CoreCommandEnvelope {
    let mut spec = common::mihomo_spec(dir, dir.join("unused.yaml"));
    spec.core.binary_path = binary;
    CoreCommandEnvelope {
        operation_id: OperationId::generate(),
        command: CoreCommand::Reconcile(Box::new(ReconcileRequest {
            core: spec.core,
            // No process ever binds it, but config resolution requires a
            // controller endpoint.
            config: ConfigInput::inline(b"external-controller: 127.0.0.1:9090\n".to_vec()),
            options: spec.options,
            expected_applied: None,
        })),
    }
}

#[tokio::test]
async fn the_whole_lifecycle_runs_without_a_single_process() {
    let (_guard, dir) = common::utf8_tempdir();
    let backend = Arc::new(FakeBackend::default());
    let control = control_with_backend(&dir, backend.clone()).await;
    let first_binary = common::fake_core_bin();
    let second_binary = dir.join("second-core.exe");
    std::fs::copy(&first_binary, &second_binary).unwrap();

    // Cold start.
    let output = control
        .submit(reconcile_envelope(&dir, first_binary))
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(matches!(
        output,
        OperationOutput::Reconciled(ApplyOutcome::Started { .. })
    ));

    // A different binary is a spec change: classify → switch, with the old
    // instance's death proven in between.
    let output = control
        .submit(reconcile_envelope(&dir, second_binary))
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(
        matches!(
            output,
            OperationOutput::Reconciled(ApplyOutcome::Switched { .. })
        ),
        "expected a switch, got {output:?}"
    );
    assert_eq!(*backend.launched_epochs.lock().unwrap(), vec![1, 2]);

    let output = control
        .submit(CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Stop,
        })
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert!(matches!(output, OperationOutput::Stopped));

    control
        .submit(CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Shutdown,
        })
        .unwrap()
        .wait()
        .await
        .unwrap();
}

#[tokio::test]
async fn an_unproven_stop_quarantines_and_blocks_every_mutation() {
    let (_guard, dir) = common::utf8_tempdir();
    let backend = Arc::new(FakeBackend::default());
    let control = control_with_backend(&dir, backend.clone()).await;
    let first_binary = common::fake_core_bin();
    let second_binary = dir.join("second-core.exe");
    std::fs::copy(&first_binary, &second_binary).unwrap();

    control
        .submit(reconcile_envelope(&dir, first_binary.clone()))
        .unwrap()
        .wait()
        .await
        .unwrap();

    // From now on the running instance refuses to prove its death.
    backend.refuse_stop.store(true, Ordering::SeqCst);
    let error = control
        .submit(reconcile_envelope(&dir, second_binary))
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::StopUnconfirmed));

    // No StopProof → no next owner: every further mutation is refused.
    let error = control
        .submit(reconcile_envelope(&dir, first_binary))
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::Quarantined));

    // Recovery demands proof of death. The fake backend never wrote the
    // process backend's authoritative pid record, so the proof is
    // unavailable and the quarantine honestly stays.
    //
    // Known phase-1 gap, on the audit ledger: recovery proof is pid-file
    // mechanics inside the manager, not yet routed through RuntimeBackend.
    let error = control
        .submit(CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Recover,
        })
        .unwrap()
        .wait()
        .await
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::Quarantined));

    control
        .submit(CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Shutdown,
        })
        .unwrap()
        .wait()
        .await
        .unwrap();
}

/// A panicking transaction used to take the executor down with it: the
/// operation that panicked and everything queued behind it stayed `Queued` or
/// `Running` forever, because the registry kept their watch senders alive.
#[tokio::test]
async fn an_executor_panic_gives_every_operation_a_terminal_state_and_reports_died() {
    let (_guard, dir) = common::utf8_tempdir();
    let backend = Arc::new(FakeBackend::default());
    backend.gate_launch.store(true, Ordering::SeqCst);
    backend.panic_on_launch.store(true, Ordering::SeqCst);
    let control = control_with_backend(&dir, backend.clone()).await;

    let panicking = control
        .submit(reconcile_envelope(&dir, common::fake_core_bin()))
        .unwrap();
    // The second operation is admitted while the first is provably inside the
    // executor, so it is sitting in the queue when the panic happens.
    backend.launch_started.notified().await;
    let queued = control
        .submit(CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Stop,
        })
        .unwrap();
    // Dropped on purpose: the wire's submit answers from the admission
    // snapshot and keeps nothing, so the registry has to record the terminal
    // state with no receiver listening. Holding the handle here would keep a
    // `watch` receiver alive and hide exactly that.
    let queued_id = queued.id();
    drop(queued);
    backend.release_launch.notify_one();

    let error = panicking.wait().await.unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::Internal));
    assert!(
        matches!(
            control.operation(queued_id),
            Some(OperationState::Failed(ref error)) if error.kind == Some(CoreErrorKind::Internal)
        ),
        "a queued operation whose handle was dropped still needs a terminal state, got {:?}",
        control.operation(queued_id)
    );
    assert_eq!(
        control.until_closed().await,
        nyanpasu_core_manager::ExecutorExit::Died
    );

    // A dead executor admits nothing further.
    let error = control
        .submit(CoreCommandEnvelope {
            operation_id: OperationId::generate(),
            command: CoreCommand::Stop,
        })
        .unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::ShuttingDown));
}

/// The latch has to land before the command is queued. With the old order a
/// reconcile that was already waiting still ran, so a shutdown could hand back
/// a *running* core.
#[tokio::test]
async fn shutdown_refuses_already_queued_work_and_reports_clean() {
    let (_guard, dir) = common::utf8_tempdir();
    let backend = Arc::new(FakeBackend::default());
    backend.gate_launch.store(true, Ordering::SeqCst);
    let control = control_with_backend(&dir, backend.clone()).await;
    let binary = common::fake_core_bin();

    let running = control
        .submit(reconcile_envelope(&dir, binary.clone()))
        .unwrap();
    backend.launch_started.notified().await;
    let queued = control.submit(reconcile_envelope(&dir, binary)).unwrap();

    let shutting_down = tokio::spawn({
        let control = control.clone();
        async move { control.shutdown().await }
    });
    backend.release_launch.notify_one();

    running.wait().await.unwrap();
    let error = queued.wait().await.unwrap_err();
    assert_eq!(error.kind, Some(CoreErrorKind::ShuttingDown));
    shutting_down.await.unwrap().unwrap();
    assert_eq!(
        control.until_closed().await,
        nyanpasu_core_manager::ExecutorExit::Clean
    );
    assert_eq!(
        backend.launched_epochs.lock().unwrap().len(),
        1,
        "the queued reconcile must not launch a core behind the shutdown"
    );
}
