#![cfg(feature = "process")]

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use nyanpasu_utils::process::{
    Backoff, BackoffRange, Command, ProcessError, ProcessEvent, ReadinessProbe, RestartPolicy,
    Supervisor, SupervisorEvent,
};
use tokio_util::sync::CancellationToken;

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[derive(Clone, Default)]
struct EventLog(Arc<Mutex<Vec<SupervisorEvent>>>);

impl EventLog {
    fn push(&self, e: SupervisorEvent) {
        self.0.lock().unwrap().push(e);
    }
    fn snapshot(&self) -> Vec<SupervisorEvent> {
        self.0.lock().unwrap().clone()
    }
    async fn wait_for(&self, pred: impl Fn(&[SupervisorEvent]) -> bool, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if pred(&self.snapshot()) {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timeout; log = {:?}",
                self.snapshot()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

#[tokio::test]
async fn restarts_on_failure_then_gives_up() {
    let log = EventLog::default();
    let log2 = log.clone();
    let _sup = Supervisor::builder(|| Command::new(child()).args(["exit-with", "1"]))
        .restart_policy(RestartPolicy::OnFailure { max_restarts: 2 })
        .backoff(Backoff::exponential(BackoffRange {
            initial: Duration::from_millis(10),
            max: Duration::from_millis(40),
        }))
        .readiness(ReadinessProbe::AliveAfter(Duration::from_millis(5000))) // never ready
        .on_event(move |e| log2.push(e))
        .spawn()
        .await
        .unwrap();

    log.wait_for(
        |evs| evs.iter().any(|e| matches!(e, SupervisorEvent::GaveUp)),
        Duration::from_secs(10),
    )
    .await;
    let evs = log.snapshot();
    let starts = evs
        .iter()
        .filter(|e| matches!(e, SupervisorEvent::Started { .. }))
        .count();
    let restarts = evs
        .iter()
        .filter(|e| matches!(e, SupervisorEvent::Restarting { .. }))
        .count();
    assert_eq!(starts, 3, "initial + 2 restarts, log = {evs:?}");
    assert_eq!(restarts, 2);
    assert!(!evs.iter().any(|e| matches!(e, SupervisorEvent::Ready)));
}

#[tokio::test]
async fn ready_emitted_and_stop_is_clean() {
    let log = EventLog::default();
    let log2 = log.clone();
    let sup = Supervisor::builder(|| Command::new(child()).args(["sleep-forever"]))
        .readiness(ReadinessProbe::AliveAfter(Duration::from_millis(100)))
        .on_event(move |e| log2.push(e))
        .spawn()
        .await
        .unwrap();

    log.wait_for(
        |evs| evs.iter().any(|e| matches!(e, SupervisorEvent::Ready)),
        Duration::from_secs(5),
    )
    .await;
    sup.stop().await.unwrap();
    let evs = log.snapshot();
    assert!(
        matches!(evs.last().unwrap(), SupervisorEvent::Stopped),
        "log = {evs:?}"
    );
    // no restart after stop
    assert!(
        !evs.iter()
            .any(|e| matches!(e, SupervisorEvent::Restarting { .. }))
    );
}

#[tokio::test]
async fn restart_storm_gives_up_even_when_alive_after_resets_attempts() {
    let log = EventLog::default();
    let log2 = log.clone();
    let _supervisor =
        Supervisor::builder(|| Command::new(child()).args(["sleep-then-exit", "100", "1"]))
            .restart_policy(RestartPolicy::OnFailure { max_restarts: 2 })
            .backoff(Backoff::exponential(BackoffRange {
                initial: Duration::from_millis(5),
                max: Duration::from_millis(5),
            }))
            .readiness(ReadinessProbe::AliveAfter(Duration::from_millis(25)))
            .on_event(move |event| log2.push(event))
            .spawn()
            .await
            .unwrap();

    log.wait_for(
        |events| {
            events
                .iter()
                .any(|event| matches!(event, SupervisorEvent::GaveUp))
        },
        Duration::from_secs(3),
    )
    .await;
}

#[tokio::test]
async fn acknowledged_readiness_ignores_stale_pid() {
    let log = EventLog::default();
    let log2 = log.clone();
    let supervisor = Supervisor::builder(|| Command::new(child()).args(["sleep-forever"]))
        .readiness(ReadinessProbe::Acknowledged)
        .on_event(move |event| log2.push(event))
        .spawn()
        .await
        .unwrap();

    let pid = log
        .snapshot()
        .iter()
        .find_map(|event| match event {
            SupervisorEvent::Started { pid } => Some(*pid),
            _ => None,
        })
        .expect("first child started");
    let stale_pid = if pid == u32::MAX { pid - 1 } else { pid + 1 };
    assert!(!supervisor.acknowledge_ready(stale_pid).await);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        !log.snapshot()
            .iter()
            .any(|event| matches!(event, SupervisorEvent::Ready))
    );

    assert!(supervisor.acknowledge_ready(pid).await);
    log.wait_for(
        |events| {
            events
                .iter()
                .any(|event| matches!(event, SupervisorEvent::Ready))
        },
        Duration::from_secs(2),
    )
    .await;
    supervisor.stop().await.unwrap();
}

#[tokio::test]
async fn clean_exit_does_not_restart() {
    let log = EventLog::default();
    let log2 = log.clone();
    let _sup = Supervisor::builder(|| Command::new(child()).args(["exit-with", "0"]))
        .restart_policy(RestartPolicy::OnFailure { max_restarts: 3 })
        .on_event(move |e| log2.push(e))
        .spawn()
        .await
        .unwrap();
    log.wait_for(
        |evs| evs.iter().any(|e| matches!(e, SupervisorEvent::Exited(_))),
        Duration::from_secs(5),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let evs = log.snapshot();
    assert_eq!(
        evs.iter()
            .filter(|e| matches!(e, SupervisorEvent::Started { .. }))
            .count(),
        1
    );
    assert!(
        !evs.iter()
            .any(|e| matches!(e, SupervisorEvent::Restarting { .. }))
    );
}

#[tokio::test]
async fn first_spawn_failure_is_error() {
    let r = Supervisor::builder(|| Command::new("definitely-not-a-real-binary-42"))
        .spawn()
        .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn readiness_is_not_starved_by_continuous_output() {
    let output_count = Arc::new(AtomicUsize::new(0));
    let ready_output_count = Arc::new(AtomicUsize::new(usize::MAX));
    let output_count_for_process = output_count.clone();
    let output_count_for_ready = output_count.clone();
    let ready_output_count_for_hook = ready_output_count.clone();

    // Keep the child alive until stop: a fixed output count can finish before
    // the readiness delay and turn this scheduling test into an exit-time race.
    let supervisor = Supervisor::builder(|| {
        Command::new(child()).args(["spam-stdout".to_owned(), usize::MAX.to_string()])
    })
    .readiness(ReadinessProbe::AliveAfter(Duration::from_millis(100)))
    .on_process_event(move |event| {
        if matches!(event, ProcessEvent::Stdout(_)) {
            output_count_for_process.fetch_add(1, Ordering::Relaxed);
        }
    })
    .on_event(move |event| {
        if matches!(event, SupervisorEvent::Ready) {
            ready_output_count_for_hook.store(
                output_count_for_ready.load(Ordering::Relaxed),
                Ordering::Relaxed,
            );
        }
    })
    .spawn()
    .await
    .unwrap();

    let count_at_ready = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let count = ready_output_count.load(Ordering::Relaxed);
            if count != usize::MAX {
                break count;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("Ready was starved by output");

    tokio::time::timeout(Duration::from_secs(10), async {
        while output_count.load(Ordering::Relaxed) <= count_at_ready {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("output was no longer flowing when Ready was emitted");

    supervisor.stop().await.unwrap();
}

#[tokio::test]
async fn cancelled_token_prevents_first_spawn() {
    let token = CancellationToken::new();
    token.cancel();
    let factory_calls = Arc::new(AtomicUsize::new(0));
    let factory_calls_ = factory_calls.clone();

    let error = Supervisor::builder(move || {
        factory_calls_.fetch_add(1, Ordering::Relaxed);
        Command::new(child()).args(["sleep-forever"])
    })
    .cancel_token(token)
    .before_spawn(|| panic!("cancelled supervisor must not prepare resources"))
    .spawn()
    .await
    .err()
    .expect("cancelled supervisor must fail before spawning");

    assert!(matches!(
        error,
        ProcessError::Engine(message)
            if message == "supervisor started with cancelled token"
    ));
    assert_eq!(factory_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn preparation_failure_prevents_first_spawn() {
    let result = Supervisor::builder(|| panic!("must not build an unauthorized child"))
        .before_spawn(|| Err(ProcessError::Engine("store inaccessible".into())))
        .spawn()
        .await;
    assert!(
        matches!(result, Err(ProcessError::Engine(message)) if message == "store inaccessible")
    );
}

#[tokio::test]
async fn preparation_runs_again_and_can_reject_respawn() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let sup = Supervisor::builder(|| Command::new(child()).args(["exit-with", "1"]))
        .backoff(Backoff::exponential(BackoffRange {
            initial: Duration::ZERO,
            max: Duration::ZERO,
        }))
        .before_spawn(move || {
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(())
            } else {
                Err(ProcessError::Engine("store changed".into()))
            }
        })
        .on_event(move |event| {
            let _ = tx.send(event);
        })
        .spawn()
        .await
        .unwrap();
    let mut starts = 0;
    loop {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap()
        {
            SupervisorEvent::Started { .. } => starts += 1,
            SupervisorEvent::PreparationFailed { detail } => {
                assert!(detail.contains("store changed"));
                break;
            }
            _ => {}
        }
    }
    assert_eq!(starts, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    sup.stop().await.unwrap();
}

#[tokio::test]
async fn cancellation_during_preparation_prevents_first_spawn() {
    let cancel = CancellationToken::new();
    let result = Supervisor::builder(|| panic!("cancelled preparation must not spawn"))
        .cancel_token(cancel.clone())
        .before_spawn(move || {
            cancel.cancel();
            Ok(())
        })
        .spawn()
        .await;
    assert!(
        matches!(result, Err(ProcessError::Engine(message)) if message == "spawn preparation cancelled")
    );
}
