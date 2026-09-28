use jiff::Timestamp;
use nyanpasu_jobs::*;
use std::collections::BTreeMap;

// Injected test-only store: verifies the execution API without the redb feature.
#[derive(Default)]
struct MemoryStore {
    runs: BTreeMap<RunId, RunRecord>,
    sequence: u64,
}
impl JobStore for MemoryStore {
    fn recover(&mut self, _: Timestamp) -> Result<(), Error> {
        Ok(())
    }
    fn admit(&mut self, mut r: RunRecord) -> Result<RunRecord, Error> {
        if self.runs.contains_key(&r.id) {
            return Err(Error::AlreadySubmitted(r.id));
        }
        self.sequence += 1;
        r.admission_sequence = self.sequence;
        self.runs.insert(r.id, r.clone());
        Ok(r)
    }
    fn append(&mut self, _: RunId, _: &[RunLog]) -> Result<(), Error> {
        Ok(())
    }
    fn finish(&mut self, r: &RunRecord, _: &[RunLog]) -> Result<(), Error> {
        self.runs.insert(r.id, r.clone());
        Ok(())
    }
    fn get(&mut self, id: RunId) -> Result<Option<RunRecord>, Error> {
        Ok(self.runs.get(&id).cloned())
    }
    fn runs(
        &mut self,
        _: &str,
        _: Option<RunCursor>,
        _: usize,
    ) -> Result<Page<RunRecord, RunCursor>, Error> {
        Err(Error::Invalid("query not used by this fake".into()))
    }
    fn logs(&mut self, _: RunId, _: u64, _: usize) -> Result<Page<RunLog, u64>, Error> {
        Err(Error::Invalid("query not used by this fake".into()))
    }
    fn prune(&mut self, _: Timestamp, _: &Retention) -> Result<(), Error> {
        Ok(())
    }
    fn health(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
#[tokio::test]
async fn typed_definition_and_handle_work_with_injected_store_without_redb() {
    let mut service = JobsService::start(
        Box::<MemoryStore>::default(),
        LogCapture::new(LogPolicy::default()),
        Limits::default(),
    )
    .await
    .unwrap();
    let job = TypedJob::new(JobDefinition::manual("double"), 3u32, |_, n| async move {
        Ok(n * 2)
    })
    .unwrap();
    let client = service.client();
    client
        .reconcile("default", 1, vec![job.registration()])
        .await
        .unwrap();
    let handle = job.bind(client);
    let run = handle.run_with(&7).await.unwrap();
    let output: u32 = run.output().await.unwrap();
    assert_eq!(output, 14);
    assert!(service.shutdown().await.unwrap().closed);
}
#[tokio::test]
async fn cooperative_cancelled_result_and_panic_do_not_leak_capacity() {
    let mut service = JobsService::start(
        Box::<MemoryStore>::default(),
        LogCapture::new(LogPolicy::default()),
        Limits::default(),
    )
    .await
    .unwrap();
    let client = service.client();
    let mut definition = JobDefinition::manual("cancel");
    definition.cancellation = Cancellation::Cooperative;
    let cancel = Job::new(definition, (), |ctx, ()| async move {
        ctx.cancelled().await;
        Err::<(), _>(JobError::cancelled())
    })
    .unwrap();
    let panic = Job::new(JobDefinition::manual("panic"), (), |_, ()| async {
        panic!("pure handler");
        #[allow(unreachable_code)]
        Ok(())
    })
    .unwrap();
    client
        .reconcile("default", 1, vec![cancel, panic])
        .await
        .unwrap();
    let run = client.run_now("cancel").await.unwrap();
    assert_eq!(run.cancel().await.unwrap(), CancelResult::Requested);
    assert_eq!(run.wait().await.unwrap().outcome, Outcome::Cancelled);
    let run = client.run_now("panic").await.unwrap();
    assert!(matches!(run.wait().await.unwrap().outcome,Outcome::Failed(e) if e.code=="panicked"));
    assert!(service.shutdown().await.unwrap().closed);
}

#[tokio::test]
async fn custom_input_decoder_panic_is_a_validation_error_not_an_actor_crash() {
    #[derive(serde::Serialize)]
    struct BadInput;
    impl<'de> serde::Deserialize<'de> for BadInput {
        fn deserialize<D: serde::Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
            panic!("input decoder")
        }
    }
    let mut service = JobsService::start(
        Box::<MemoryStore>::default(),
        LogCapture::new(LogPolicy::default()),
        Limits::default(),
    )
    .await
    .unwrap();
    let client = service.client();
    let job = Job::new(JobDefinition::manual("bad"), BadInput, |_, _| async {
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        client.reconcile("default", 1, vec![job]).await,
        Err(Error::Invalid(_))
    ));
    assert!(client.inspect().await.unwrap().jobs.is_empty());
    assert!(service.shutdown().await.unwrap().closed);
}
