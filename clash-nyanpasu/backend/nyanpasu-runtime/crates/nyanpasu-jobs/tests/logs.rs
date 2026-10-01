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

#[tokio::test]
async fn disabled_capture_preserves_logging_context_children_and_history() {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use tracing::{Event, Subscriber};
    use tracing_subscriber::layer::Context;

    struct OrdinaryLogs(Arc<AtomicUsize>);
    impl<S: Subscriber> Layer<S> for OrdinaryLogs {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            if event.metadata().target() == "safe_job" {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    let ordinary = Arc::new(AtomicUsize::new(0));
    let capture = LogCapture::new(policy());
    let subscriber = tracing_subscriber::registry()
        .with(OrdinaryLogs(ordinary.clone()))
        .with(capture.layer());
    let (_dir, mut service, _) = service(Limits::default(), capture)
        .with_subscriber(subscriber)
        .await;
    let client = service.client();
    let (send, receive) = tokio::sync::oneshot::channel();
    let send = Arc::new(Mutex::new(Some(send)));
    let disabled = Job::new(JobDefinition::manual("disabled"), (), move |ctx, ()| {
        let send = send.clone();
        async move {
            tracing::info!(target:"safe_job",code="disabled");
            let child = ctx.clone();
            ctx.spawn(async move {
                child
                    .instrument(async {
                        tracing::info!(target:"safe_job",code="disabled_child");
                    })
                    .await;
            })
            .unwrap();
            send.lock().unwrap().take().unwrap().send(ctx).ok();
            Ok(42)
        }
    })
    .unwrap()
    .with_log_capture(LogCaptureMode::Disabled);
    let disabled_for_reconcile = disabled.clone();
    let blocking = Job::blocking(JobDefinition::manual("disabled_blocking"), (), |_, ()| {
        tracing::info!(target:"safe_job",code="disabled_blocking");
        Ok(())
    })
    .unwrap()
    .with_log_capture(LogCaptureMode::Disabled);
    // Keep the enabled run alive while using the disabled run's context inside it.
    let receive = Arc::new(tokio::sync::Mutex::new(Some(receive)));
    let enabled = Job::new(JobDefinition::manual("enabled"), (), move |_, ()| {
        let receive = receive.clone();
        async move {
            let disabled_context = receive.lock().await.take().unwrap().await.unwrap();
            disabled_context
                .instrument(async {
                    tracing::info!(target:"safe_job",code="nested_disabled");
                })
                .await;
            tracing::info!(target:"safe_job",code="enabled");
            Ok(())
        }
    })
    .unwrap();
    let registrations = vec![enabled, disabled, blocking];
    client
        .reconcile("default", 1, registrations.clone())
        .await
        .unwrap();
    let enabled_run = client.run_now("enabled").await.unwrap();
    let disabled_run = client.run_now("disabled").await.unwrap();
    assert_eq!(disabled_run.wait_output::<u32>().await.unwrap(), 42);
    enabled_run.wait().await.unwrap();
    let blocking_run = client.run_now("disabled_blocking").await.unwrap();
    blocking_run.wait().await.unwrap();
    assert_eq!(ordinary.load(Ordering::SeqCst), 5);
    for id in [disabled_run.id, blocking_run.id] {
        let record = client.get_run(id).await.unwrap();
        assert_eq!(record.last_log_sequence, 0);
        assert_eq!(record.dropped_log_count, 0);
        assert_eq!(record.completion().unwrap().journal, JournalState::Durable);
        assert!(client.logs(id, 0, 50).await.unwrap().items.is_empty());
    }
    let logs = client.logs(enabled_run.id, 0, 50).await.unwrap();
    assert_eq!(logs.items.len(), 1);
    assert_eq!(logs.items[0].fields["code"], "\"enabled\"");
    let mut changed = registrations;
    changed[1] = disabled_for_reconcile.with_log_capture(LogCaptureMode::Inherit);
    assert!(matches!(
        client.reconcile("default", 1, changed.clone()).await,
        Err(Error::Invalid(_))
    ));
    client.reconcile("default", 2, changed).await.unwrap();
    assert!(service.shutdown().await.unwrap().closed);
}
