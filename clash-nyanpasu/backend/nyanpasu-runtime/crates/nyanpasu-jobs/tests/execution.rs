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
use tokio::sync::{Notify, oneshot};

#[tokio::test]
async fn typed_execution_multiple_waiters_late_wait_and_noop_reconcile() {
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let job = Job::new(JobDefinition::manual("double"), 3u32, |_, n| async move {
        Ok(n * 2)
    })
    .unwrap();
    client
        .reconcile("default", 1, vec![job.clone()])
        .await
        .unwrap();
    let generation = client.inspect().await.unwrap().jobs[0].generation;
    client.reconcile("default", 2, vec![job]).await.unwrap();
    assert_eq!(
        client.inspect().await.unwrap().jobs[0].generation,
        generation
    );
    let run = client.run_with("double", &7u32).await.unwrap();
    let (a, b) = tokio::join!(run.wait_output::<u32>(), run.wait_output::<u32>());
    assert_eq!(a.unwrap(), 14);
    assert_eq!(b.unwrap(), 14);
    settled(&client).await;
    assert_eq!(run.wait_output::<u32>().await.unwrap(), 14);
    assert_eq!(
        client.cancel(run.id).await.unwrap(),
        CancelResult::AlreadyFinished
    );
    drain(&mut service).await;
}

#[tokio::test]
async fn wait_timeout_and_delete_recreate_never_release_running_key() {
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let release = Arc::new(Notify::new());
    let job = Job::new(JobDefinition::manual("held"), (), {
        let release = release.clone();
        move |_, ()| {
            let release = release.clone();
            async move {
                release.notified().await;
                Ok(42)
            }
        }
    })
    .unwrap();
    client
        .reconcile("default", 1, vec![job.clone()])
        .await
        .unwrap();
    let run = client.run_now("held").await.unwrap();
    assert_eq!(
        client.wait(run.id, Some(Duration::ZERO)).await.unwrap_err(),
        Error::WaitTimedOut(run.id)
    );
    assert_eq!(run.cancel().await.unwrap(), CancelResult::Unsupported);
    client.reconcile("default", 2, vec![]).await.unwrap();
    client.reconcile("default", 3, vec![job]).await.unwrap();
    assert!(matches!(client.run_now("held").await, Err(Error::Busy)));
    release.notify_one();
    assert_eq!(run.wait_output::<i32>().await.unwrap(), 42);
    drain(&mut service).await;
}

#[tokio::test]
async fn cancellation_requires_ack_and_success_wins_race() {
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let ack = Arc::new(Notify::new());
    let (started, ready) = oneshot::channel();
    let started = Arc::new(std::sync::Mutex::new(Some(started)));
    let mut definition = JobDefinition::manual("cancel");
    definition.cancellation = Cancellation::Cooperative;
    let job = Job::new(definition, (), {
        let ack = ack.clone();
        move |ctx, ()| {
            let ack = ack.clone();
            let started = started.clone();
            async move {
                started.lock().unwrap().take().unwrap().send(()).unwrap();
                ctx.cancelled().await;
                ack.notified().await;
                Ok(7)
            }
        }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    let run = client.run_now("cancel").await.unwrap();
    ready.await.unwrap();
    assert_eq!(run.cancel().await.unwrap(), CancelResult::Requested);
    assert_eq!(run.cancel().await.unwrap(), CancelResult::Requested);
    assert!(matches!(client.run_now("cancel").await, Err(Error::Busy)));
    ack.notify_one();
    assert_eq!(run.wait().await.unwrap().outcome, Outcome::Succeeded);
    drain(&mut service).await;
}

#[tokio::test]
async fn duplicate_id_and_invalid_input_never_repeat_side_effects() {
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let calls = Arc::new(AtomicUsize::new(0));
    let job = Job::new(JobDefinition::manual("count"), 0u32, {
        let calls = calls.clone();
        move |_, n| {
            calls.fetch_add(1, Ordering::SeqCst);
            async move { Ok(n) }
        }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    assert!(matches!(
        client.run_with("count", &"wrong type").await,
        Err(Error::Invalid(_))
    ));
    let run = client.run_now("count").await.unwrap();
    run.wait().await.unwrap();
    settled(&client).await;
    assert!(
        matches!(client.submit("count",run.id,None,Trigger::Manual).await,Err(Error::AlreadySubmitted(id)) if id==run.id)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drain(&mut service).await;
}

#[tokio::test]
async fn panic_waits_for_delegated_work_and_shutdown_reports_it() {
    let limits = Limits {
        shutdown_timeout: Duration::from_millis(10),
        ..Limits::default()
    };
    let (_dir, mut service, _) = service(limits, capture()).await;
    let client = service.client();
    let (send, recv) = oneshot::channel();
    let send = Arc::new(std::sync::Mutex::new(Some(send)));
    let job = Job::new(JobDefinition::manual("panic"), (), move |ctx, ()| {
        let send = send.clone();
        async move {
            send.lock()
                .unwrap()
                .take()
                .unwrap()
                .send(ctx.delegation().unwrap())
                .unwrap();
            panic!("test panic");
            #[allow(unreachable_code)]
            Ok(())
        }
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    let run = client.run_now("panic").await.unwrap();
    let token = recv.await.unwrap();
    let report = service.shutdown().await.unwrap();
    assert!(!report.closed);
    assert_eq!(report.remaining, vec![run.id]);
    assert!(
        client
            .inspect()
            .await
            .unwrap()
            .execution_uncertain
            .contains(&run.id)
    );
    drop(token);
    assert!(
        matches!(run.wait().await.unwrap().outcome,Outcome::Failed(error) if error.code=="panicked")
    );
    drain(&mut service).await;
}

#[tokio::test]
async fn blocking_work_is_owned_until_real_completion() {
    let (_dir, mut service, _) = service(Limits::default(), capture()).await;
    let client = service.client();
    let (release, recv) = std::sync::mpsc::channel();
    let recv = Arc::new(std::sync::Mutex::new(recv));
    let (send, ready) = oneshot::channel();
    let send = Arc::new(std::sync::Mutex::new(Some(send)));
    let job = Job::blocking(JobDefinition::manual("blocking"), (), move |_, ()| {
        send.lock().unwrap().take().unwrap().send(()).unwrap();
        recv.lock().unwrap().recv().unwrap();
        Ok(())
    })
    .unwrap();
    client.reconcile("default", 1, vec![job]).await.unwrap();
    let run = client.run_now("blocking").await.unwrap();
    ready.await.unwrap();
    assert_eq!(run.cancel().await.unwrap(), CancelResult::Unsupported);
    assert!(matches!(client.run_now("blocking").await, Err(Error::Busy)));
    release.send(()).unwrap();
    run.wait().await.unwrap();
    drain(&mut service).await;
}
