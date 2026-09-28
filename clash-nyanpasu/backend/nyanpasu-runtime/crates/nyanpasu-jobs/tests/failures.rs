#![cfg(feature = "redb-store")]
mod support;
use nyanpasu_jobs::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use support::*;

#[tokio::test]
async fn failed_admission_has_no_handler_effect() {
    let (_dir, mut service, faults) = service(Limits::default(), capture()).await;
    let client = service.client();
    let effects = Arc::new(AtomicUsize::new(0));
    let n = effects.clone();
    let job = Job::new(JobDefinition::manual("job"), (), move |_, ()| {
        n.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    faults.fail_admit.store(true, Ordering::SeqCst);
    assert!(matches!(
        client.run_now("job").await,
        Err(Error::AdmissionFailed(_))
    ));
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert!(client.runs("job", None, 50).await.unwrap().items.is_empty());
    faults.fail_admit.store(false, Ordering::SeqCst);
    drain(&mut service).await;
}

#[tokio::test]
async fn finish_failure_preserves_success_closes_admission_and_retries_only_journal() {
    let (_dir, mut service, faults) = service(Limits::default(), capture()).await;
    let client = service.client();
    let effects = Arc::new(AtomicUsize::new(0));
    let n = effects.clone();
    let job = Job::new(JobDefinition::manual("job"), (), move |_, ()| {
        n.fetch_add(1, Ordering::SeqCst);
        async { Ok(55) }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    faults.fail_finish.store(true, Ordering::SeqCst);
    let run = client.run_now("job").await.unwrap();
    let completion = run.wait().await.unwrap();
    assert_eq!(completion.outcome, Outcome::Succeeded);
    assert_eq!(completion.decode::<i32>().unwrap(), 55);
    assert!(matches!(completion.journal, JournalState::Degraded(_)));
    assert!(matches!(
        client.run_now("job").await,
        Err(Error::Journal(_))
    ));
    faults.fail_finish.store(false, Ordering::SeqCst);
    settled(&client).await;
    assert_eq!(run.wait().await.unwrap().journal, JournalState::Durable);
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    assert!(faults.finishes.load(Ordering::SeqCst) >= 2);
    drain(&mut service).await;
}

#[tokio::test]
async fn blocked_admission_does_not_block_inspect_cancel_or_shutdown() {
    let limits = Limits {
        control_timeout: Duration::from_millis(30),
        shutdown_timeout: Duration::from_millis(30),
        ..Limits::default()
    };
    let (_dir, mut service, faults) = service(limits, capture()).await;
    let client = service.client();
    let effects = Arc::new(AtomicUsize::new(0));
    let n = effects.clone();
    let job = Job::new(JobDefinition::manual("job"), (), move |_, ()| {
        n.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    *faults.block_admit.lock().unwrap() = Some(wait);
    let id = RunId::new_v4();
    let c = client.clone();
    let submit = tokio::spawn(async move { c.submit("job", id, None, Trigger::Manual).await });
    faults.entered.notified().await;
    assert_eq!(
        client.cancel(id).await.unwrap(),
        CancelResult::AdmissionPending
    );
    assert_eq!(client.inspect().await.unwrap().active.len(), 1);
    assert!(matches!(submit.await.unwrap(),Err(Error::AdmissionUnknown(run)) if run==id));
    assert!(!service.shutdown().await.unwrap().closed);
    release.send(()).unwrap();
    let result = client.wait(id, None).await.unwrap();
    assert_eq!(result.outcome, Outcome::Cancelled);
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    drain(&mut service).await;
}

#[tokio::test]
async fn blocked_terminal_write_returns_degraded_without_starting_another_write() {
    let limits = Limits {
        finalization_timeout: Duration::from_millis(20),
        ..Limits::default()
    };
    let (_dir, mut service, faults) = service(limits, capture()).await;
    let client = service.client();
    client
        .reconcile(
            "default",
            1,
            vec![Job::new(JobDefinition::manual("job"), (), |_, ()| async { Ok(9) }).unwrap()],
        )
        .await
        .unwrap();
    let (release, wait) = std::sync::mpsc::channel();
    *faults.block_finish.lock().unwrap() = Some(wait);
    let run = client.run_now("job").await.unwrap();
    faults.entered.notified().await;
    let completion = run.wait().await.unwrap();
    assert_eq!(completion.decode::<i32>().unwrap(), 9);
    assert!(matches!(completion.journal, JournalState::Degraded(_)));
    assert_eq!(faults.finishes.load(Ordering::SeqCst), 1);
    release.send(()).unwrap();
    settled(&client).await;
    assert_eq!(faults.finishes.load(Ordering::SeqCst), 1);
    assert_eq!(run.wait().await.unwrap().journal, JournalState::Durable);
    drain(&mut service).await;
}

#[tokio::test]
async fn output_size_or_serializer_failure_does_not_change_business_success() {
    #[derive(Clone)]
    struct Panics;
    impl serde::Serialize for Panics {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            panic!("serializer panic")
        }
    }
    let limits = Limits {
        output_bytes: 8,
        ..Limits::default()
    };
    let (_dir, mut service, _) = service(limits, capture()).await;
    let client = service.client();
    client
        .reconcile(
            "default",
            1,
            vec![
                Job::new(JobDefinition::manual("large"), (), |_, ()| async {
                    Ok("x".repeat(1000))
                })
                .unwrap(),
                Job::new(JobDefinition::manual("panic"), (), |_, ()| async {
                    Ok(Panics)
                })
                .unwrap(),
            ],
        )
        .await
        .unwrap();
    for key in ["large", "panic"] {
        let completion = client.run_now(key).await.unwrap().wait().await.unwrap();
        assert_eq!(completion.outcome, Outcome::Succeeded);
        assert!(matches!(completion.output, Output::Unavailable(_)));
    }
    drain(&mut service).await;
}

#[tokio::test]
async fn uncertain_admission_is_read_back_without_a_second_admission_or_effect() {
    let (_dir, mut service, faults) = service(Limits::default(), capture()).await;
    let client = service.client();
    let effects = Arc::new(AtomicUsize::new(0));
    let n = effects.clone();
    client
        .reconcile(
            "default",
            1,
            vec![
                Job::new(JobDefinition::manual("job"), (), move |_, ()| {
                    n.fetch_add(1, Ordering::SeqCst);
                    async { Ok(()) }
                })
                .unwrap(),
            ],
        )
        .await
        .unwrap();
    faults.uncertain_admit.store(true, Ordering::SeqCst);
    let id = RunId::new_v4();
    assert!(
        matches!(client.submit("job",id,None,Trigger::Manual).await,Err(Error::AdmissionUnknown(run)) if run==id)
    );
    assert_eq!(
        client.wait(id, None).await.unwrap().outcome,
        Outcome::Succeeded
    );
    assert_eq!(faults.admissions.load(Ordering::SeqCst), 1);
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    drain(&mut service).await;
}

#[tokio::test]
async fn shutdown_budget_includes_blocking_query_and_cloned_clients_do_not_hold_the_file() {
    let limits = Limits {
        shutdown_timeout: Duration::from_millis(20),
        ..Limits::default()
    };
    let (dir, mut service, faults) = service(limits, capture()).await;
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
    let (release, wait) = std::sync::mpsc::channel();
    *faults.block_get.lock().unwrap() = Some(wait);
    let c = client.clone();
    let query = tokio::spawn(async move { c.get_run(run.id).await });
    faults.entered.notified().await;
    let report = service.shutdown().await.unwrap();
    assert!(!report.closed);
    assert!(report.remaining.is_empty());
    release.send(()).unwrap();
    query.await.unwrap().unwrap();
    drain(&mut service).await;
    let _reopened = RedbJobStore::open(dir.path().join("jobs.redb")).unwrap();
    assert!(matches!(client.run_now("job").await, Err(Error::Stopped)));
}
