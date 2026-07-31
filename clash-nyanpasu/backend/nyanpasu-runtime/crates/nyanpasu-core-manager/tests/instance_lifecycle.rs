mod common;

use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use nyanpasu_core_manager::{
    HealthPolicy, ProbeHandle, ProbePhase, ProbeResult,
    instance::Instance,
    spec::ResolvedController,
    state::{HealthState, InstanceState},
};
use tokio_util::sync::CancellationToken;

fn http_controller(port: u16) -> ResolvedController {
    ResolvedController {
        host: clash_api::Host::http(format!("127.0.0.1:{port}")).unwrap(),
        secret: None,
    }
}

#[tokio::test]
async fn start_confirms_via_version_probe() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nx-fake-core:\n  ready-delay-ms: 300\n"),
    );
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    let (recorder, log) = common::record_states(instance.state());

    instance.wait_ready().await.expect("becomes healthy");
    assert!(matches!(
        instance.state().borrow().state,
        InstanceState::Running { pid } if pid > 0
    ));
    assert_eq!(instance.epoch(), 1);

    instance.stop().await.expect("stop");
    recorder.abort();
    let states = log.lock().clone();
    // Starting must precede Running: the probe gates the Running transition.
    let starting = states
        .iter()
        .position(|s| matches!(s, InstanceState::Starting));
    let running = states
        .iter()
        .position(|s| matches!(s, InstanceState::Running { .. }));
    assert!(
        starting.unwrap() < running.unwrap(),
        "sequence was {states:?}"
    );
}

#[tokio::test]
async fn dropping_an_instance_kills_the_core() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    instance.wait_ready().await.expect("healthy");
    drop(instance);
    common::wait_port_refused(port).await;
}

#[tokio::test]
async fn startup_timeout_kills_the_core() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!("external-controller: 127.0.0.1:{port}\nx-fake-core:\n  never-ready: true\n"),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = std::time::Duration::from_secs(1);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    let err = instance.wait_ready().await.expect_err("must time out");
    assert!(
        matches!(err, nyanpasu_core_manager::Error::StartupTimeout { .. }),
        "got {err}"
    );
    // The tree is killed: the fake core's controller port must be released.
    common::wait_port_refused(port).await;
}

#[tokio::test]
async fn immediate_exit_reports_stderr_tail() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  exit-code: 1\n  stderr-lines:\n    - \"boot marker failure\"\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    // One retry, tiny backoff: GaveUp arrives well inside the startup deadline.
    spec.options.restart_policy =
        nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 1 };

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn succeeds; the failure is the exit");
    let err = instance.wait_ready().await.expect_err("must fail");
    match err {
        nyanpasu_core_manager::Error::StartupFailed { stderr_tail } => {
            assert!(
                stderr_tail.contains("boot marker failure"),
                "tail: {stderr_tail}"
            )
        }
        other => panic!("unexpected error: {other}"),
    }
}

/// mihomo and clash premium print their fatal record on stdout and leave stderr
/// empty, so the diagnostic tail has to follow severity rather than the stream.
#[tokio::test]
async fn immediate_exit_reports_a_stdout_fatal() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  exit-code: 1\n  stdout-lines:\n    - 'time=\"2026-07-29T00:17:26.518376100+08:00\" level=fatal msg=\"Parse config error: bad port\"'\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.restart_policy =
        nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 1 };

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn succeeds; the failure is the exit");
    let err = instance.wait_ready().await.expect_err("must fail");
    match err {
        nyanpasu_core_manager::Error::StartupFailed { stderr_tail } => {
            assert_eq!(stderr_tail, "Parse config error: bad port")
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[tokio::test]
async fn crash_recovers_through_restart_and_reprobe() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 1200\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    let (recorder, log) = common::record_states(instance.state());
    instance.wait_ready().await.expect("initially healthy");

    // The first run crashes; the supervisor restarts; the re-probe
    // confirms the second (healthy) run.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (restarted, running) = {
                let states = log.lock();
                (
                    states
                        .iter()
                        .any(|state| matches!(state, InstanceState::Restarting { .. })),
                    states
                        .iter()
                        .filter(|state| matches!(state, InstanceState::Running { .. }))
                        .count(),
                )
            };
            if restarted && running >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("replacement never became ready");

    instance.stop().await.expect("stop");
    recorder.abort();
    let states = log.lock().clone();
    let running_count = states
        .iter()
        .filter(|s| matches!(s, InstanceState::Running { .. }))
        .count();
    assert!(running_count >= 2, "sequence was {states:?}");
}

#[tokio::test]
async fn crash_loop_exhausts_the_budget() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 1200\n  crash-times: 99\n  state-file: {state_file}\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.restart_policy =
        nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 1 };

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    instance
        .wait_ready()
        .await
        .expect("first run is briefly healthy");

    let mut rx = instance.state();
    let terminal = common::wait_for_state(
        &mut rx,
        |s| s.is_terminal(),
        std::time::Duration::from_secs(15),
    )
    .await;
    assert!(
        matches!(
            &terminal,
            InstanceState::Stopped(nyanpasu_core_manager::StopReason::Error(msg))
                if msg.contains("restart budget exhausted")
        ),
        "terminal was {terminal:?}"
    );
}

#[tokio::test]
async fn user_stop_is_terminal_and_releases_the_port() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let spec = common::mihomo_spec(&dir, config);

    let instance = Instance::spawn(spec, 1, http_controller(port), CancellationToken::new())
        .await
        .expect("spawn");
    instance.wait_ready().await.expect("healthy");
    let mut rx = instance.state();
    instance.stop().await.expect("stop");

    let terminal = common::wait_for_state(
        &mut rx,
        |s| s.is_terminal(),
        std::time::Duration::from_secs(5),
    )
    .await;
    assert!(
        matches!(
            terminal,
            InstanceState::Stopped(nyanpasu_core_manager::StopReason::User)
        ),
        "terminal was {terminal:?}"
    );
    common::wait_port_refused(port).await;
}

#[tokio::test]
async fn custom_readiness_and_success_threshold_gate_running() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(20),
        Duration::from_secs(1),
        NonZeroU32::new(3).unwrap(),
        NonZeroU32::new(3).unwrap(),
        Duration::ZERO,
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let probe = ProbeHandle::from_fn("threshold-readiness", {
        let calls = calls.clone();
        move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            async { ProbeResult::Healthy }
        }
    });

    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(probe)
        .spawn()
        .await
        .expect("spawn");
    instance.wait_ready().await.expect("threshold reached");
    assert!(calls.load(Ordering::SeqCst) >= 3);
    assert!(matches!(
        instance.state().borrow().state,
        InstanceState::Running { .. }
    ));
    instance.stop().await.expect("stop");
}

#[tokio::test]
async fn liveness_is_off_by_default_after_custom_readiness() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(20),
        Duration::from_secs(1),
        NonZeroU32::new(3).unwrap(),
        NonZeroU32::MIN,
        Duration::ZERO,
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let probe = ProbeHandle::from_fn("readiness-only", {
        let calls = calls.clone();
        move |_| {
            calls.fetch_add(1, Ordering::SeqCst);
            async { ProbeResult::Healthy }
        }
    });
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(probe)
        .spawn()
        .await
        .unwrap();
    instance.wait_ready().await.unwrap();
    let after_ready = calls.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(calls.load(Ordering::SeqCst), after_ready);
    instance.stop().await.unwrap();
}

#[tokio::test]
async fn readiness_probe_can_be_reused_for_liveness() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(20),
        Duration::from_secs(1),
        NonZeroU32::MIN,
        NonZeroU32::MIN,
        Duration::ZERO,
    )
    .unwrap();
    let phases = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let probe = ProbeHandle::from_fn("shared-probe", {
        let phases = phases.clone();
        move |context| {
            phases.lock().push(context.phase);
            async { ProbeResult::Healthy }
        }
    });
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(probe)
        .liveness_with_readiness_probe()
        .spawn()
        .await
        .unwrap();
    instance.wait_ready().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        while !phases.lock().contains(&ProbePhase::Liveness) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("shared probe never entered liveness phase");
    assert!(phases.lock().contains(&ProbePhase::Readiness));
    instance.stop().await.unwrap();
}

#[tokio::test]
async fn liveness_hysteresis_is_observe_only_and_recovers() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(20),
        Duration::from_secs(1),
        NonZeroU32::new(2).unwrap(),
        NonZeroU32::new(2).unwrap(),
        Duration::ZERO,
    )
    .unwrap();
    let readiness = ProbeHandle::from_fn("ready", |_| async { ProbeResult::Healthy });
    let failures_remaining = Arc::new(AtomicUsize::new(0));
    let liveness_calls = Arc::new(AtomicUsize::new(0));
    let liveness = ProbeHandle::from_fn("scripted-liveness", {
        let failures_remaining = failures_remaining.clone();
        let liveness_calls = liveness_calls.clone();
        move |_| {
            liveness_calls.fetch_add(1, Ordering::SeqCst);
            let fail = failures_remaining
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok();
            async move {
                if fail {
                    ProbeResult::Unhealthy {
                        detail: Some("credential=must-not-appear-in-debug".repeat(64)),
                    }
                } else {
                    ProbeResult::Healthy
                }
            }
        }
    });
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(readiness)
        .liveness_probe(liveness)
        .spawn()
        .await
        .unwrap();
    instance.wait_ready().await.unwrap();
    let pid = instance.pid().unwrap();

    let before_blip = liveness_calls.load(Ordering::SeqCst);
    failures_remaining.store(1, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), async {
        while liveness_calls.load(Ordering::SeqCst) < before_blip + 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        instance.state().borrow().health.as_ref().unwrap().state,
        HealthState::Healthy,
        "one failure must not flap health"
    );

    failures_remaining.store(4, Ordering::SeqCst);
    let mut rx = instance.state();
    let unhealthy =
        common::wait_for_health(&mut rx, HealthState::Unhealthy, Duration::from_secs(2)).await;
    assert!(matches!(unhealthy.state, InstanceState::Running { pid: current } if current == pid));
    let health = unhealthy.health.unwrap();
    assert!(health.consecutive_failures >= 2);
    assert!(health.last_error.as_ref().unwrap().len() <= 512);
    assert!(!format!("{health:?}").contains("credential="));
    let unhealthy_changed_at = health.changed_at;

    let recovered =
        common::wait_for_health(&mut rx, HealthState::Healthy, Duration::from_secs(2)).await;
    assert!(matches!(recovered.state, InstanceState::Running { pid: current } if current == pid));
    assert!(recovered.health.unwrap().changed_at >= unhealthy_changed_at);
    instance
        .wait_ready()
        .await
        .expect("runtime unhealthy does not revoke readiness");
    assert_eq!(instance.pid(), Some(pid), "observe-only must not restart");
    instance.stop().await.unwrap();
}

#[tokio::test]
async fn absolute_startup_deadline_rejects_late_probe_success() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_millis(120);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(10),
        Duration::from_secs(1),
        NonZeroU32::MIN,
        NonZeroU32::MIN,
        Duration::from_secs(1),
    )
    .unwrap();
    let probe = ProbeHandle::from_fn("late-success", |_| async {
        tokio::time::sleep(Duration::from_millis(400)).await;
        ProbeResult::Healthy
    });
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(probe)
        .spawn()
        .await
        .unwrap();
    let error = instance.wait_ready().await.expect_err("deadline must win");
    assert!(matches!(
        error,
        nyanpasu_core_manager::Error::StartupTimeout { .. }
    ));
    common::wait_port_refused(port).await;
}

#[tokio::test]
async fn stop_cancels_a_hanging_liveness_probe() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let config = common::write_config(&dir, &format!("external-controller: 127.0.0.1:{port}\n"));
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(10),
        Duration::from_secs(5),
        NonZeroU32::MIN,
        NonZeroU32::MIN,
        Duration::ZERO,
    )
    .unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let liveness = ProbeHandle::from_fn("hanging", {
        let started = started.clone();
        move |context| {
            let started = started.clone();
            async move {
                started.notify_one();
                context.cancel.cancelled().await;
                ProbeResult::Healthy
            }
        }
    });
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(ProbeHandle::from_fn("ready", |_| async {
            ProbeResult::Healthy
        }))
        .liveness_probe(liveness)
        .spawn()
        .await
        .unwrap();
    instance.wait_ready().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), started.notified())
        .await
        .expect("liveness did not start");
    tokio::time::timeout(Duration::from_secs(2), instance.stop())
        .await
        .expect("stop blocked behind probe")
        .unwrap();
    common::wait_port_refused(port).await;
}

#[tokio::test]
async fn startup_timeout_is_one_budget_across_crash_retries() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("retry-state");
    let launch_count = dir.join("launch-count");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 150\n  crash-times: 1\n  state-file: {state_file}\n  launch-count-file: {launch_count}\n  fail-after-launches: 99\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_secs(10);
    spec.options.restart_policy =
        nyanpasu_utils::process::RestartPolicy::OnFailure { max_restarts: 20 };
    spec.options.backoff = nyanpasu_utils::process::Backoff::exponential(
        Duration::from_millis(10),
        Duration::from_millis(10),
    );
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(ProbeHandle::from_fn("never-ready", |_| async {
            ProbeResult::Unhealthy {
                detail: Some("not ready".into()),
            }
        }))
        .spawn()
        .await
        .unwrap();
    let started = std::time::Instant::now();
    let error = instance.wait_ready().await.expect_err("must time out");
    assert!(matches!(
        error,
        nyanpasu_core_manager::Error::StartupTimeout { .. }
    ));
    assert!(
        started.elapsed() < Duration::from_secs(11),
        "crash retries extended the absolute startup deadline"
    );
    let launches: usize = std::fs::read_to_string(launch_count)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0);
    assert!(launches > 1, "test did not exercise a retry");
}

#[tokio::test]
async fn exited_run_cancels_its_in_flight_probe_before_replacement() {
    let (_guard, dir) = common::utf8_tempdir();
    let port = common::free_port();
    let state_file = dir.join("one-crash-state");
    let config = common::write_config(
        &dir,
        &format!(
            "external-controller: 127.0.0.1:{port}\nx-fake-core:\n  crash-after-ms: 80\n  crash-times: 1\n  state-file: {state_file}\n"
        ),
    );
    let mut spec = common::mihomo_spec(&dir, config);
    spec.options.startup_timeout = Duration::from_millis(450);
    spec.options.health = HealthPolicy::new(
        Duration::from_millis(5),
        Duration::from_secs(1),
        NonZeroU32::MIN,
        NonZeroU32::MIN,
        Duration::ZERO,
    )
    .unwrap();
    let first_pid = Arc::new(parking_lot::Mutex::new(None));
    let probe = ProbeHandle::from_fn("late-first-run", {
        let first_pid = first_pid.clone();
        move |context| {
            let is_first = {
                let mut first = first_pid.lock();
                let pid = *first.get_or_insert(context.pid);
                pid == context.pid
            };
            async move {
                if is_first {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    ProbeResult::Healthy
                } else {
                    ProbeResult::Unhealthy {
                        detail: Some("replacement remains unhealthy".into()),
                    }
                }
            }
        }
    });
    let instance = Instance::builder(spec, 1, http_controller(port), CancellationToken::new())
        .readiness_probe(probe)
        .spawn()
        .await
        .unwrap();
    let error = instance
        .wait_ready()
        .await
        .expect_err("cancelled first-run probe must not ready the replacement");
    assert!(matches!(
        error,
        nyanpasu_core_manager::Error::StartupTimeout { .. }
    ));
}
