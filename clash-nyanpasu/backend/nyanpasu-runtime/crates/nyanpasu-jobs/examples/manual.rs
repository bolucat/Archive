use nyanpasu_jobs::*;
use tracing_subscriber::layer::SubscriberExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let capture = LogCapture::new(LogPolicy::default());
    // An application installs one subscriber at its composition root. Configure
    // target/field allowlists before including messages or sensitive job output.
    let subscriber = tracing_subscriber::registry().with(capture.layer());
    let path = directory.path().join("jobs.redb");
    let store = tokio::task::spawn_blocking(move || RedbJobStore::open(path)).await??;
    use tracing::instrument::WithSubscriber;
    let mut service = JobsService::start(Box::new(store), capture, Limits::default())
        .with_subscriber(subscriber)
        .await?;
    let client = service.client();
    let job = TypedJob::new(
        JobDefinition::manual("demo/double"),
        2u32,
        |_, n| async move { Ok(n * 2) },
    )?;
    client
        .reconcile("default", 1, vec![job.registration()])
        .await?;
    let handle = job.bind(client);
    let run = handle.run_with(&21).await?;
    println!("{}: {}", run.id(), run.output().await?);
    let report = service.shutdown().await?;
    assert!(report.closed);
    Ok(())
}
