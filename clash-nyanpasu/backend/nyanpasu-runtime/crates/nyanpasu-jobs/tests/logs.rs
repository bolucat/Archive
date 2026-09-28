#![cfg(feature = "redb-store")]
mod support;
use nyanpasu_jobs::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use support::*;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::{Layer, layer::SubscriberExt};

fn policy() -> LogPolicy {
    LogPolicy {
        targets: BTreeMap::from([("safe_job".into(), BTreeSet::from(["code".into()]))]),
        ..LogPolicy::default()
    }
}
#[tokio::test]
async fn scoped_tracing_crosses_children_and_blocking_without_secrets_or_global_filter() {
    let capture = LogCapture::new(policy());
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_filter(tracing_subscriber::filter::LevelFilter::WARN),
        )
        .with(capture.layer());
    let (_dir, mut service, _) = service(Limits::default(), capture)
        .with_subscriber(subscriber)
        .await;
    let client = service.client();
    let job = Job::new(JobDefinition::manual("logged"), (), |ctx, ()| async move {
        tracing::info!(target:"safe_job",code="start",token="secret", "sensitive message");
        ctx.spawn(async {
            tracing::info!(target:"safe_job",code="child");
        })
        .unwrap();
        tracing::info!(target:"unapproved",code="must not store");
        Ok(())
    })
    .unwrap();
    let blocking = Job::blocking(JobDefinition::manual("blocking"), (), |_, ()| {
        tracing::info!(target:"safe_job",code="blocking");
        Ok(())
    })
    .unwrap();
    client
        .reconcile("default", 1, vec![job, blocking])
        .await
        .unwrap();
    let run = client.run_now("logged").await.unwrap();
    run.wait().await.unwrap();
    let logs = client.logs(run.id, 0, 50).await.unwrap();
    assert_eq!(logs.items.len(), 2);
    for log in logs.items {
        assert_eq!(log.fields.len(), 1);
        assert!(!serde_json::to_string(&log).unwrap().contains("secret"));
    }
    let run = client.run_now("blocking").await.unwrap();
    run.wait().await.unwrap();
    assert_eq!(client.logs(run.id, 0, 50).await.unwrap().items.len(), 1);
    assert!(service.shutdown().await.unwrap().closed);
}

#[tokio::test]
async fn overflow_is_counted_and_sealed_context_cannot_append_after_wait() {
    let mut policy = policy();
    policy.queue_entries = 2;
    policy.run_entries = 3;
    let capture = LogCapture::new(policy);
    let subscriber = tracing_subscriber::registry().with(capture.layer());
    let (_dir, mut service, _) = service(Limits::default(), capture)
        .with_subscriber(subscriber)
        .await;
    let client = service.client();
    let (send, recv) = tokio::sync::oneshot::channel();
    let send = Arc::new(std::sync::Mutex::new(Some(send)));
    client
        .reconcile(
            "default",
            1,
            vec![
                Job::new(JobDefinition::manual("burst"), (), move |ctx, ()| {
                    let send = send.clone();
                    async move {
                        for i in 0..20 {
                            tracing::info!(target:"safe_job",code=i);
                        }
                        send.lock().unwrap().take().unwrap().send(ctx).ok();
                        Ok(())
                    }
                })
                .unwrap(),
            ],
        )
        .await
        .unwrap();
    let run = client.run_now("burst").await.unwrap();
    let context = recv.await.unwrap();
    run.wait().await.unwrap();
    let record = client.get_run(run.id).await.unwrap();
    assert_eq!(record.last_log_sequence, 20);
    assert_eq!(record.dropped_log_count, 18);
    let first = client.logs(run.id, 0, 1).await.unwrap();
    assert_eq!(first.items.len(), 1);
    let second = client.logs(run.id, first.next.unwrap(), 1).await.unwrap();
    assert_eq!(second.items.len(), 1);
    assert!(second.next.is_none());
    context
        .instrument(async {
            tracing::info!(target:"safe_job",code="late");
        })
        .await;
    assert_eq!(client.logs(run.id, 0, 50).await.unwrap().items.len(), 2);
    assert!(context.spawn(async {}).is_err());
    assert!(service.shutdown().await.unwrap().closed);
}

#[tokio::test]
async fn blocked_log_flush_does_not_block_business_and_reports_finalization_degraded() {
    let capture = LogCapture::new(policy());
    let subscriber = tracing_subscriber::registry().with(capture.layer());
    let limits = Limits {
        finalization_timeout: Duration::from_millis(20),
        ..Limits::default()
    };
    let (_dir, mut service, faults) = service(limits, capture).with_subscriber(subscriber).await;
    let client = service.client();
    let release_job = Arc::new(tokio::sync::Notify::new());
    let release = release_job.clone();
    client
        .reconcile(
            "default",
            1,
            vec![
                Job::new(JobDefinition::manual("job"), (), move |_, ()| {
                    let release = release.clone();
                    async move {
                        tracing::info!(target:"safe_job",code="fetch");
                        release.notified().await;
                        Ok(1)
                    }
                })
                .unwrap(),
            ],
        )
        .await
        .unwrap();
    let (release_write, wait) = std::sync::mpsc::channel();
    *faults.block_append.lock().unwrap() = Some(wait);
    let run = client.run_now("job").await.unwrap();
    faults.entered.notified().await;
    release_job.notify_one();
    let result = run.wait().await.unwrap();
    assert_eq!(result.outcome, Outcome::Succeeded);
    assert!(matches!(result.journal, JournalState::Degraded(_)));
    release_write.send(()).unwrap();
    settled(&client).await;
    assert_eq!(client.logs(run.id, 0, 50).await.unwrap().items.len(), 1);
    assert_eq!(run.wait().await.unwrap().journal, JournalState::Durable);
    assert!(service.shutdown().await.unwrap().closed);
}
