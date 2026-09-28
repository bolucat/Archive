#![cfg(feature = "redb-store")]
mod support;
use jiff::Timestamp;
use nyanpasu_jobs::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use support::*;
struct FakeClock(Mutex<Timestamp>);
impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        *self.0.lock().unwrap()
    }
}
async fn spin_until(mut test: impl FnMut() -> bool) {
    let until = std::time::Instant::now() + Duration::from_secs(5);
    while !test() {
        assert!(
            std::time::Instant::now() < until,
            "condition not acknowledged"
        );
        tokio::task::yield_now().await;
    }
}
fn keeper() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        loop {
            tokio::task::yield_now().await;
        }
    })
}

#[tokio::test(start_paused = true)]
async fn interval_first_delay_manual_anchor_noop_reconcile_and_missed_ticks() {
    let keep = keeper();
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut definition = JobDefinition::manual("interval");
    definition.schedule = Schedule::Interval { every_ms: 10_000 };
    let job = Job::new(definition, (), move |_, ()| {
        count.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    })
    .unwrap();
    client
        .reconcile("default", 1, vec![job.clone()])
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    tokio::time::advance(Duration::from_secs(4)).await;
    client
        .run_now("interval")
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    settled(&client).await;
    client.reconcile("default", 2, vec![job]).await.unwrap();
    tokio::time::advance(Duration::from_secs(6)).await;
    spin_until(|| calls.load(Ordering::SeqCst) == 2).await;
    settled(&client).await;
    tokio::time::advance(Duration::from_secs(37)).await;
    // Synchronize with the timer tick through the actor's observable counter.
    loop {
        if client.inspect().await.unwrap().missed_trigger_count > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tokio::time::advance(Duration::from_secs(3)).await;
    spin_until(|| calls.load(Ordering::SeqCst) == 3).await;
    assert!(service.shutdown().await.unwrap().closed);
    keep.abort();
}

#[tokio::test(start_paused = true)]
async fn once_manual_run_does_not_consume_schedule_and_reconcile_fences_old_deadline() {
    let keep = keeper();
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut definition = JobDefinition::manual("once");
    definition.schedule = Schedule::Once { delay_ms: 10_000 };
    let job = Job::new(definition, (), move |_, ()| {
        count.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    })
    .unwrap();
    client
        .reconcile("default", 1, vec![job.clone()])
        .await
        .unwrap();
    client.run_now("once").await.unwrap().wait().await.unwrap();
    settled(&client).await;
    tokio::time::advance(Duration::from_secs(4)).await;
    client.reconcile("default", 2, vec![]).await.unwrap();
    client.reconcile("default", 3, vec![job]).await.unwrap();
    tokio::time::advance(Duration::from_secs(6)).await;
    client.inspect().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(4)).await;
    spin_until(|| calls.load(Ordering::SeqCst) == 2).await;
    settled(&client).await;
    assert!(
        client.inspect().await.unwrap().jobs[0]
            .next_run_at
            .is_none()
    );
    assert!(service.shutdown().await.unwrap().closed);
    keep.abort();
}

#[tokio::test]
async fn invalid_scope_snapshot_is_atomic_and_cannot_delete_another_owner() {
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let good = Job::new(JobDefinition::manual("good"), (), |_, ()| async { Ok(()) }).unwrap();
    client
        .reconcile("default", 1, vec![good.clone()])
        .await
        .unwrap();
    let mut bad = good.clone();
    bad.definition.schedule = Schedule::Interval { every_ms: 0 };
    assert!(client.reconcile("default", 2, vec![bad]).await.is_err());
    let inspect = client.inspect().await.unwrap();
    assert_eq!(inspect.jobs.len(), 1);
    assert_eq!(inspect.scopes["default"].applied_revision, Some(1));
    assert!(inspect.scopes["default"].last_error.is_some());
    assert_eq!(
        client.reconcile("default", 1, vec![]).await.unwrap_err(),
        Error::StaleRevision
    );
    let mut other = good;
    other.definition.scope = "other".into();
    assert!(client.reconcile("other", 1, vec![other]).await.is_err());
    client.reconcile("other", 2, vec![]).await.unwrap();
    assert_eq!(client.inspect().await.unwrap().jobs.len(), 1);
    assert!(service.shutdown().await.unwrap().closed);
}

#[tokio::test(start_paused = true)]
async fn cron_clock_forward_skips_missed_and_rollback_does_not_replay() {
    let keep = keeper();
    let dir = tempfile::tempdir().unwrap();
    let start = "2026-09-07T00:00:00Z".parse().unwrap();
    let clock = Arc::new(FakeClock(Mutex::new(start)));
    let mut service = JobsService::start_with_clock(
        Box::new(RedbJobStore::open(dir.path().join("jobs.redb")).unwrap()),
        capture(),
        Limits::default(),
        clock.clone(),
    )
    .await
    .unwrap();
    let client = service.client();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let mut definition = JobDefinition::manual("cron");
    definition.schedule = Schedule::Cron {
        expression: "* * * * *".into(),
        timezone: "UTC".into(),
    };
    client
        .reconcile(
            "default",
            1,
            vec![
                Job::new(definition, (), move |_, ()| {
                    count.fetch_add(1, Ordering::SeqCst);
                    async { Ok(()) }
                })
                .unwrap(),
            ],
        )
        .await
        .unwrap();
    *clock.0.lock().unwrap() = start.checked_add(Duration::from_secs(60)).unwrap();
    tokio::time::advance(Duration::from_secs(5)).await;
    spin_until(|| calls.load(Ordering::SeqCst) == 1).await;
    settled(&client).await;
    *clock.0.lock().unwrap() = start;
    tokio::time::advance(Duration::from_secs(5)).await;
    assert_eq!(
        client.inspect().await.unwrap().jobs[0].next_run_at,
        Some(start.checked_add(Duration::from_secs(120)).unwrap())
    );
    *clock.0.lock().unwrap() = start.checked_add(Duration::from_secs(600)).unwrap();
    tokio::time::advance(Duration::from_secs(5)).await;
    loop {
        if client.inspect().await.unwrap().missed_trigger_count > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(service.shutdown().await.unwrap().closed);
    keep.abort();
}

#[tokio::test(start_paused = true)]
async fn scheduled_busy_is_journaled_and_global_capacity_never_creates_a_queue() {
    let keep = keeper();
    let limits = Limits {
        concurrency: 2,
        ..Limits::default()
    };
    let (_dir, mut service, _) = service(limits, capture()).await;
    let client = service.client();
    let release = Arc::new(tokio::sync::Notify::new());
    let gate = release.clone();
    let mut definition = JobDefinition::manual("held");
    definition.schedule = Schedule::Interval { every_ms: 10_000 };
    let job = Job::new(definition, (), move |_, ()| {
        let gate = gate.clone();
        async move {
            gate.notified().await;
            Ok(())
        }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    let run = client.run_now("held").await.unwrap();
    tokio::time::advance(Duration::from_secs(10)).await;
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let page = client.runs("held", None, 10).await.unwrap();
        if page.items.iter().any(|r| {
            r.completion()
                .is_some_and(|c| c.outcome == Outcome::Skipped)
        }) {
            break;
        }
        assert!(std::time::Instant::now() < until);
        tokio::task::yield_now().await;
    }
    assert!(client.inspect().await.unwrap().active.len() <= 2);
    assert!(matches!(client.run_now("held").await, Err(Error::Busy)));
    release.notify_one();
    run.wait().await.unwrap();
    assert!(service.shutdown().await.unwrap().closed);
    keep.abort();
}

#[tokio::test(start_paused = true)]
async fn idle_retention_removes_expired_history_without_another_job() {
    let keep = keeper();
    let dir = tempfile::tempdir().unwrap();
    let start = "2026-09-07T00:00:00Z".parse().unwrap();
    let clock = Arc::new(FakeClock(Mutex::new(start)));
    let limits = Limits {
        retention: Retention {
            age: Duration::from_secs(30),
            ..Retention::default()
        },
        ..Limits::default()
    };
    let mut service = JobsService::start_with_clock(
        Box::new(RedbJobStore::open(dir.path().join("jobs.redb")).unwrap()),
        capture(),
        limits,
        clock.clone(),
    )
    .await
    .unwrap();
    let client = service.client();
    client
        .reconcile(
            "default",
            1,
            vec![Job::new(JobDefinition::manual("job"), (), |_, ()| async { Ok(()) }).unwrap()],
        )
        .await
        .unwrap();
    let run = client.run_now("job").await.unwrap();
    run.wait().await.unwrap();
    settled(&client).await;
    *clock.0.lock().unwrap() = start.checked_add(Duration::from_secs(120)).unwrap();
    tokio::time::advance(Duration::from_secs(60)).await;
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if matches!(client.get_run(run.id).await, Err(Error::NotFound)) {
            break;
        }
        assert!(std::time::Instant::now() < until);
        tokio::task::yield_now().await;
    }
    assert!(service.shutdown().await.unwrap().closed);
    keep.abort();
}
