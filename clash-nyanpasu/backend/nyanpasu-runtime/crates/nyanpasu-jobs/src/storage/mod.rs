use crate::model::*;
use jiff::Timestamp;

/// Synchronous storage port. Called only by the owned, single blocking worker.
/// `admit`, `finish`, and `prune` must be atomic; retries of finish are idempotent.
/// `AdmissionFailed` guarantees no record was committed; other admission errors
/// are uncertain and will be resolved by reading the same ID, never by resubmitting.
/// Do not retain references to application actors or perform business effects here.
pub trait JobStore: Send + 'static {
    fn recover(&mut self, now: Timestamp) -> Result<(), Error>;
    fn admit(&mut self, record: RunRecord) -> Result<RunRecord, Error>;
    fn append(&mut self, id: RunId, logs: &[RunLog]) -> Result<(), Error>;
    fn finish(&mut self, record: &RunRecord, logs: &[RunLog]) -> Result<(), Error>;
    fn get(&mut self, id: RunId) -> Result<Option<RunRecord>, Error>;
    fn runs(
        &mut self,
        job: &str,
        after: Option<RunCursor>,
        limit: usize,
    ) -> Result<Page<RunRecord, RunCursor>, Error>;
    fn logs(&mut self, id: RunId, after: u64, limit: usize) -> Result<Page<RunLog, u64>, Error>;
    fn prune(&mut self, now: Timestamp, retention: &Retention) -> Result<(), Error>;
    fn health(&mut self) -> Result<(), Error>;
}

#[cfg(feature = "redb-store")]
mod redb;
#[cfg(feature = "redb-store")]
pub use redb::RedbJobStore;
