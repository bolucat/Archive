use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type JobKey = String;
pub type RunId = Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    Manual,
    Once,
    Interval,
    Cron,
    StartupCatchUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Cancellation {
    #[default]
    Unsupported,
    Cooperative,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct JobError {
    pub code: String,
    pub message: String,
}
impl JobError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
    pub(crate) fn bounded(mut self) -> Self {
        fn trim(value: &mut String, limit: usize) {
            let mut end = value.len().min(limit);
            while !value.is_char_boundary(end) {
                end -= 1;
            }
            value.truncate(end);
        }
        trim(&mut self.code, 128);
        trim(&mut self.message, 4096);
        self
    }
    pub fn cancelled() -> Self {
        Self::new("cancelled", "Work acknowledged cancellation")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Succeeded,
    Failed(JobError),
    Cancelled,
    Interrupted,
    Skipped,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Output {
    Json(String),
    Unavailable(String),
    None,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JournalState {
    Durable,
    Degraded(String),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    pub outcome: Outcome,
    pub output: Output,
    pub journal: JournalState,
}
impl Completion {
    pub fn decode<T: serde::de::DeserializeOwned>(&self) -> Result<T, Error> {
        match &self.output {
            Output::Json(json) => serde_json::from_str(json)
                .map_err(|_| Error::OutputUnavailable("schema_mismatch".into())),
            Output::Unavailable(code) => Err(Error::OutputUnavailable(code.clone())),
            Output::None => Err(Error::OutputUnavailable("no_output".into())),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    Admitted,
    Running,
    Cancelling,
    Finalizing,
    Finished(Completion),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: RunId,
    pub job: JobKey,
    pub definition_version: u64,
    pub trigger: Trigger,
    pub scheduled_at: Option<Timestamp>,
    pub admitted_at: Timestamp,
    pub admission_sequence: u64,
    pub finished_at: Option<Timestamp>,
    pub state: RunState,
    pub last_log_sequence: u64,
    pub dropped_log_count: u64,
}
impl RunRecord {
    pub fn completion(&self) -> Option<&Completion> {
        match &self.state {
            RunState::Finished(c) => Some(c),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunLog {
    pub run_id: RunId,
    pub sequence: u64,
    pub time: Timestamp,
    pub level: String,
    pub target: String,
    /// Only fields explicitly permitted by the injected LogPolicy are captured.
    pub fields: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunCursor {
    pub sequence: u64,
    pub id: RunId,
}
#[derive(Debug, Clone)]
pub struct Page<T, C> {
    pub items: Vec<T>,
    pub next: Option<C>,
}
#[derive(Debug, Clone)]
pub struct Retention {
    pub age: Duration,
    pub per_job: usize,
    pub runs: usize,
    pub bytes: usize,
}
impl Default for Retention {
    fn default() -> Self {
        Self {
            age: Duration::from_secs(7 * 86400),
            per_job: 100,
            runs: 10_000,
            bytes: 256 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Limits {
    pub concurrency: usize,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub control_timeout: Duration,
    pub finalization_timeout: Duration,
    pub wait_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub lateness: Duration,
    pub retention: Retention,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            concurrency: 8,
            input_bytes: 64 * 1024,
            output_bytes: 64 * 1024,
            control_timeout: Duration::from_secs(10),
            finalization_timeout: Duration::from_secs(10),
            wait_timeout: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            lateness: Duration::from_secs(5),
            retention: Retention::default(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid definition or input: {0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
    #[error("job or global capacity busy")]
    Busy,
    #[error("service is shutting down")]
    ShuttingDown,
    #[error("journal unavailable: {0}")]
    Journal(String),
    #[error("run already submitted: {0}")]
    AlreadySubmitted(RunId),
    #[error("admission result unknown; query {0}, do not resubmit")]
    AdmissionUnknown(RunId),
    #[error("admission failed: {0}")]
    AdmissionFailed(String),
    #[error("wait timed out for {0}")]
    WaitTimedOut(RunId),
    #[error("control call timed out")]
    TimedOut,
    #[error("service stopped")]
    Stopped,
    #[error("output unavailable: {0}")]
    OutputUnavailable(String),
    #[error("stale scope revision")]
    StaleRevision,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CancelResult {
    Requested,
    Unsupported,
    AdmissionPending,
    AlreadyFinished,
}
#[derive(Debug, Clone)]
pub struct ShutdownReport {
    pub remaining: Vec<RunId>,
    pub journal_degraded: bool,
    pub closed: bool,
}
