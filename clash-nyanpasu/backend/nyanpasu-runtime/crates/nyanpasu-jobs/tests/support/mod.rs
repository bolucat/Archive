#![allow(dead_code)]
use jiff::Timestamp;
use nyanpasu_jobs::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[derive(Default)]
pub struct Faults {
    pub fail_admit: AtomicBool,
    pub uncertain_admit: AtomicBool,
    pub fail_finish: AtomicBool,
    pub fail_append: AtomicBool,
    pub finishes: AtomicUsize,
    pub admissions: AtomicUsize,
    pub block_admit: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    pub block_finish: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    pub block_get: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    pub block_append: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    pub entered: tokio::sync::Notify,
}
pub struct Store {
    pub inner: RedbJobStore,
    pub faults: Arc<Faults>,
}
impl JobStore for Store {
    fn recover(&mut self, now: Timestamp) -> Result<(), Error> {
        self.inner.recover(now)
    }
    fn admit(&mut self, record: RunRecord) -> Result<RunRecord, Error> {
        self.faults.admissions.fetch_add(1, Ordering::SeqCst);
        let block = self.faults.block_admit.lock().unwrap().take();
        if let Some(wait) = block {
            self.faults.entered.notify_one();
            wait.recv().unwrap();
        }
        if self.faults.fail_admit.load(Ordering::SeqCst) {
            return Err(Error::AdmissionFailed("injected admission failure".into()));
        }
        let saved = self.inner.admit(record)?;
        if self.faults.uncertain_admit.load(Ordering::SeqCst) {
            return Err(Error::Journal("commit acknowledgement lost".into()));
        }
        Ok(saved)
    }
    fn append(&mut self, id: RunId, logs: &[RunLog]) -> Result<(), Error> {
        let block = self.faults.block_append.lock().unwrap().take();
        if let Some(wait) = block {
            self.faults.entered.notify_one();
            wait.recv().unwrap();
        }
        if self.faults.fail_append.load(Ordering::SeqCst) {
            return Err(Error::Journal("injected log failure".into()));
        }
        self.inner.append(id, logs)
    }
    fn finish(&mut self, record: &RunRecord, logs: &[RunLog]) -> Result<(), Error> {
        self.faults.finishes.fetch_add(1, Ordering::SeqCst);
        let block = self.faults.block_finish.lock().unwrap().take();
        if let Some(wait) = block {
            self.faults.entered.notify_one();
            wait.recv().unwrap();
        }
        if self.faults.fail_finish.load(Ordering::SeqCst) {
            return Err(Error::Journal("injected commit failure".into()));
        }
        self.inner.finish(record, logs)
    }
    fn get(&mut self, id: RunId) -> Result<Option<RunRecord>, Error> {
        let block = self.faults.block_get.lock().unwrap().take();
        if let Some(wait) = block {
            self.faults.entered.notify_one();
            wait.recv().unwrap();
        }
        self.inner.get(id)
    }
    fn runs(
        &mut self,
        job: &str,
        after: Option<RunCursor>,
        limit: usize,
    ) -> Result<Page<RunRecord, RunCursor>, Error> {
        self.inner.runs(job, after, limit)
    }
    fn logs(&mut self, id: RunId, after: u64, limit: usize) -> Result<Page<RunLog, u64>, Error> {
        self.inner.logs(id, after, limit)
    }
    fn prune(&mut self, now: Timestamp, retention: &Retention) -> Result<(), Error> {
        self.inner.prune(now, retention)
    }
    fn health(&mut self) -> Result<(), Error> {
        self.inner.health()
    }
}
pub async fn service(
    limits: Limits,
    capture: LogCapture,
) -> (tempfile::TempDir, JobsService, Arc<Faults>) {
    let dir = tempfile::tempdir().unwrap();
    let faults = Arc::new(Faults::default());
    let store = Store {
        inner: RedbJobStore::open(dir.path().join("jobs.redb")).unwrap(),
        faults: faults.clone(),
    };
    let service = JobsService::start(Box::new(store), capture, limits)
        .await
        .unwrap();
    (dir, service, faults)
}
pub fn capture() -> LogCapture {
    LogCapture::new(LogPolicy::default())
}
pub async fn settled(client: &JobsClient) {
    // Actor RPC acknowledgement yields instead of sleeping; bounded test guard.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if client.inspect().await.unwrap().active.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

pub async fn drain(service: &mut JobsService) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if service.shutdown().await.unwrap().closed {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
