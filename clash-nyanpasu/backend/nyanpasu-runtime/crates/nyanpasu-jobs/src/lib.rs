#![doc = include_str!("../README.md")]
mod actor;
mod clock;
pub mod dto;
mod encoding;
mod runner;
mod typed;
pub use clock::{Clock, SystemClock};
pub use typed::{JobHandle, TypedJob, TypedRunHandle};
mod job;
mod journal;
mod logging;
mod model;
mod schedule;
pub mod storage;

pub use actor::{Inspection, JobSnapshot, ScopeSnapshot};
pub use job::{Job, JobContext, JobDefinition};
pub use logging::{JobJournalLayer, LogCapture, LogPolicy};
pub use model::*;
pub use schedule::Schedule;
pub use storage::JobStore;
#[cfg(feature = "redb-store")]
pub use storage::RedbJobStore;

use actor::{Args, JobsActor, Msg};
use journal::{Journal, JournalWorker};
use ractor::{Actor, ActorRef, RpcReplyPort};
use std::{sync::Arc, time::Duration};
use tokio::{sync::watch, task::JoinHandle};

/// Lifetime owner. Keep this until `shutdown` has joined all workers. A shutdown
/// timeout leaves this object usable for inspect/wait and another shutdown call.
#[must_use]
pub struct JobsService {
    client: Option<JobsClient>,
    actor: Option<JoinHandle<()>>,
    worker: Option<JoinHandle<()>>,
    closed: watch::Receiver<bool>,
    stop_sent: bool,
}
#[derive(Clone)]
pub struct JobsClient {
    actor: ActorRef<Msg>,
    journal: Journal,
    limits: Arc<Limits>,
}
impl JobsService {
    pub async fn start(
        store: Box<dyn JobStore>,
        capture: LogCapture,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::start_with_clock(store, capture, limits, Arc::new(SystemClock)).await
    }
    pub async fn start_with_clock(
        store: Box<dyn JobStore>,
        capture: LogCapture,
        limits: Limits,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, Error> {
        if limits.concurrency == 0
            || limits.input_bytes == 0
            || limits.output_bytes == 0
            || limits.control_timeout.is_zero()
            || limits.finalization_timeout.is_zero()
            || limits.retention.runs == 0
            || limits.retention.per_job == 0
        {
            return Err(Error::Invalid("zero service limit".into()));
        }
        let worker = JournalWorker::start(store);
        let now = clock.now();
        let retention = limits.retention.clone();
        if let Err(error) = worker
            .client
            .call(move |s| {
                s.recover(now)?;
                s.prune(now, &retention)
            })
            .await
        {
            drop(worker.client);
            let _ = worker.task.await;
            return Err(error);
        }
        let limits = Arc::new(limits);
        let (closed, receiver) = watch::channel(false);
        let (actor, task) = JobsActor::spawn(
            None,
            JobsActor,
            Args {
                journal: worker.client.clone(),
                logs: capture,
                limits: limits.clone(),
                closed,
                clock,
                dispatch: tracing::dispatcher::get_default(Clone::clone),
            },
        )
        .await
        .map_err(|_| Error::Stopped)?;
        Ok(Self {
            client: Some(JobsClient {
                actor,
                journal: worker.client,
                limits,
            }),
            actor: Some(task),
            worker: Some(worker.task),
            closed: receiver,
            stop_sent: false,
        })
    }
    pub fn client(&self) -> JobsClient {
        self.client
            .as_ref()
            .expect("service already closed")
            .clone()
    }
    pub async fn shutdown(&mut self) -> Result<ShutdownReport, Error> {
        let Some(client) = self.client.as_ref() else {
            return Ok(ShutdownReport {
                remaining: Vec::new(),
                journal_degraded: false,
                closed: true,
            });
        };
        let timeout = client.limits.shutdown_timeout;
        match tokio::time::timeout(timeout, self.drain()).await {
            Ok(result) => result,
            Err(_) => {
                let inspection = if let Some(client) = &self.client {
                    client.inspect().await.ok()
                } else {
                    None
                };
                Ok(ShutdownReport {
                    remaining: inspection
                        .as_ref()
                        .map(|i| i.active.iter().map(|r| r.id).collect())
                        .unwrap_or_default(),
                    journal_degraded: inspection.map(|i| i.journal_degraded).unwrap_or(true),
                    closed: false,
                })
            }
        }
    }
    async fn drain(&mut self) -> Result<ShutdownReport, Error> {
        let client = self.client.as_ref().ok_or(Error::Stopped)?;
        let already_closed = *self.closed.borrow();
        if !already_closed
            && let Err(error) = client.rpc(Msg::Shutdown).await
            && !*self.closed.borrow()
        {
            return Err(error);
        }
        self.closed
            .wait_for(|closed| *closed)
            .await
            .map_err(|_| Error::Stopped)?;
        if let Some(task) = self.actor.as_mut() {
            let _ = task.await;
        }
        self.actor.take();
        if !self.stop_sent {
            client.journal.close().await;
            self.stop_sent = true;
        }
        // Poll by reference: a timeout must leave the worker handle owned here.
        if let Some(task) = self.worker.as_mut() {
            let _ = task.await;
        }
        self.worker.take();
        self.client.take();
        Ok(ShutdownReport {
            remaining: Vec::new(),
            journal_degraded: false,
            closed: true,
        })
    }
}
impl JobsClient {
    async fn rpc<T: Send + 'static>(
        &self,
        msg: impl FnOnce(RpcReplyPort<T>) -> Msg,
    ) -> Result<T, Error> {
        match self
            .actor
            .call(msg, Some(self.limits.control_timeout))
            .await
            .map_err(|_| Error::Stopped)?
        {
            ractor::rpc::CallResult::Success(value) => Ok(value),
            ractor::rpc::CallResult::Timeout => Err(Error::TimedOut),
            ractor::rpc::CallResult::SenderError => Err(Error::Stopped),
        }
    }
    pub async fn reconcile(
        &self,
        scope: impl Into<String>,
        revision: u64,
        jobs: Vec<Job>,
    ) -> Result<(), Error> {
        let scope = scope.into();
        self.rpc(|r| Msg::Reconcile(scope, revision, jobs, r))
            .await?
    }
    pub async fn run_now(&self, key: impl Into<String>) -> Result<RunHandle, Error> {
        self.submit(key, RunId::new_v4(), None, Trigger::Manual)
            .await
    }
    pub async fn run_with<I: serde::Serialize>(
        &self,
        key: impl Into<String>,
        input: &I,
    ) -> Result<RunHandle, Error> {
        let input = encoding::json(input, self.limits.input_bytes)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        self.submit(key, RunId::new_v4(), Some(input), Trigger::Manual)
            .await
    }
    /// Caller-generated IDs allow recovery after an IPC/RPC response is lost.
    /// Never automatically retry an AdmissionUnknown with a new ID.
    pub async fn submit(
        &self,
        key: impl Into<String>,
        id: RunId,
        input: Option<String>,
        trigger: Trigger,
    ) -> Result<RunHandle, Error> {
        let key = key.into();
        if input
            .as_ref()
            .is_some_and(|value| value.len() > self.limits.input_bytes)
        {
            return Err(Error::Invalid("input limit".into()));
        }
        match self.rpc(|r| Msg::Submit(key, id, input, trigger, r)).await {
            Ok(result) => result?,
            Err(Error::TimedOut) => return Err(Error::AdmissionUnknown(id)),
            Err(e) => return Err(e),
        }
        Ok(RunHandle {
            id,
            client: self.clone(),
        })
    }
    pub async fn inspect(&self) -> Result<Inspection, Error> {
        self.rpc(Msg::Inspect).await
    }
    pub async fn get_run(&self, id: RunId) -> Result<RunRecord, Error> {
        if let Ok(Some(receiver)) = self.rpc(|r| Msg::Watch(id, r)).await {
            if let Some(record) = receiver.borrow().clone() {
                return Ok(record);
            }
            let active = self.inspect().await?;
            if let Some(record) = active.active.into_iter().find(|r| r.id == id) {
                return Ok(record);
            }
        }
        self.journal
            .query(self.limits.control_timeout, move |s| s.get(id))
            .await?
            .ok_or(Error::NotFound)
    }
    pub async fn wait(&self, id: RunId, timeout: Option<Duration>) -> Result<Completion, Error> {
        let wait = async {
            if let Ok(Some(mut receiver)) = self.rpc(|r| Msg::Watch(id, r)).await
                && let Ok(value) = receiver.wait_for(Option::is_some).await
                && let Some(record) = value.as_ref()
                && let Some(c) = record.completion()
            {
                return Ok(c.clone());
            }
            self.journal
                .query(self.limits.control_timeout, move |s| s.get(id))
                .await?
                .ok_or(Error::NotFound)?
                .completion()
                .cloned()
                .ok_or(Error::WaitTimedOut(id))
        };
        tokio::time::timeout(timeout.unwrap_or(self.limits.wait_timeout), wait)
            .await
            .map_err(|_| Error::WaitTimedOut(id))?
    }
    pub async fn cancel(&self, id: RunId) -> Result<CancelResult, Error> {
        if let Some(answer) = self.rpc(|r| Msg::Cancel(id, r)).await? {
            return Ok(answer);
        }
        self.journal
            .query(self.limits.control_timeout, move |s| s.get(id))
            .await?
            .ok_or(Error::NotFound)?;
        Ok(CancelResult::AlreadyFinished)
    }
    pub async fn runs(
        &self,
        job: impl Into<String>,
        after: Option<RunCursor>,
        limit: usize,
    ) -> Result<Page<RunRecord, RunCursor>, Error> {
        let job = job.into();
        self.journal
            .query(self.limits.control_timeout, move |s| {
                s.runs(&job, after, limit)
            })
            .await
    }
    pub async fn logs(
        &self,
        id: RunId,
        after: u64,
        limit: usize,
    ) -> Result<Page<RunLog, u64>, Error> {
        self.journal
            .query(self.limits.control_timeout, move |s| {
                s.logs(id, after, limit)
            })
            .await
    }
}
#[derive(Clone)]
pub struct RunHandle {
    pub id: RunId,
    client: JobsClient,
}
impl RunHandle {
    pub async fn wait(&self) -> Result<Completion, Error> {
        self.client.wait(self.id, None).await
    }
    pub async fn wait_output<O: serde::de::DeserializeOwned>(&self) -> Result<O, Error> {
        self.wait().await?.decode()
    }
    pub async fn cancel(&self) -> Result<CancelResult, Error> {
        self.client.cancel(self.id).await
    }
}
