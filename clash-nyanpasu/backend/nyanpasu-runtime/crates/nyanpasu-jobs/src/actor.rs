use crate::{job::Job, journal::Journal, logging::LogCapture, model::*, schedule::Schedule};
use jiff::Timestamp;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{oneshot, watch},
    task::JoinHandle,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::instrument::WithSubscriber;

#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub definition: crate::JobDefinition,
    pub generation: u64,
    pub next_run_at: Option<Timestamp>,
    pub active_runs: Vec<RunId>,
}
#[derive(Debug, Clone, Default)]
pub struct ScopeSnapshot {
    pub source_revision: u64,
    pub applied_revision: Option<u64>,
    pub last_error: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Inspection {
    pub jobs: Vec<JobSnapshot>,
    pub scopes: BTreeMap<String, ScopeSnapshot>,
    pub active: Vec<RunRecord>,
    pub execution_uncertain: Vec<RunId>,
    pub journal_degraded: bool,
    pub shutting_down: bool,
    pub dropped_trigger_count: u64,
    pub missed_trigger_count: u64,
}
type ReplyHandoff = oneshot::Sender<RpcReplyPort<Result<(), Error>>>;
struct Submission {
    key: JobKey,
    id: RunId,
    input: Option<String>,
    trigger: Trigger,
    scheduled: Option<Timestamp>,
    skip: bool,
}

pub(crate) struct Registration {
    job: Job,
    generation: u64,
    deadline: Option<Instant>,
    next: Option<Timestamp>,
}
pub(crate) struct Active {
    record: RunRecord,
    cancel: CancellationToken,
    capability: Cancellation,
    result: watch::Sender<Option<RunRecord>>,
    task: JoinHandle<()>,
    admitting: bool,
    uncertain: bool,
}
pub(crate) struct State {
    jobs: BTreeMap<JobKey, Registration>,
    scopes: BTreeMap<String, ScopeSnapshot>,
    active: BTreeMap<RunId, Active>,
    generation: u64,
    maintenance: Option<JoinHandle<()>>,
    maintenance_due: Instant,
    faults: BTreeSet<RunId>,
    shutting_down: bool,
    journal: Journal,
    logs: LogCapture,
    limits: Arc<Limits>,
    dropped: u64,
    missed: u64,
    timer: Option<JoinHandle<()>>,
    timer_cancel: CancellationToken,
    dispatch: tracing::Dispatch,
    clock: Arc<dyn crate::Clock>,
    closed: watch::Sender<bool>,
    wake: Arc<tokio::sync::Notify>,
}
pub(crate) enum Msg {
    Reconcile(String, u64, Vec<Job>, RpcReplyPort<Result<(), Error>>),
    Submit(
        JobKey,
        RunId,
        Option<String>,
        Trigger,
        RpcReplyPort<Result<(), Error>>,
    ),
    Watch(
        RunId,
        RpcReplyPort<Option<watch::Receiver<Option<RunRecord>>>>,
    ),
    Cancel(RunId, RpcReplyPort<Option<CancelResult>>),
    Inspect(RpcReplyPort<Inspection>),
    Shutdown(RpcReplyPort<()>),
    Tick(RpcReplyPort<Duration>),
    Started(RunId, RunRecord, oneshot::Sender<bool>),
    Fault(RunId),
    Progress(RunId, RunRecord, bool),
    Done(RunId),
    MaintenanceDone(RunId),
}
pub(crate) struct JobsActor;
pub(crate) struct Args {
    pub journal: Journal,
    pub logs: LogCapture,
    pub limits: Arc<Limits>,
    pub closed: watch::Sender<bool>,
    pub clock: Arc<dyn crate::Clock>,
    pub dispatch: tracing::Dispatch,
}
impl Actor for JobsActor {
    type Msg = Msg;
    type State = State;
    type Arguments = Args;
    async fn pre_start(
        &self,
        myself: ActorRef<Msg>,
        args: Args,
    ) -> Result<State, ActorProcessingErr> {
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let wake = Arc::new(tokio::sync::Notify::new());
        let notify = wake.clone();
        let timer = tokio::spawn(async move {
            let mut delay = Duration::from_secs(5);
            loop {
                tokio::select! { _=token.cancelled()=>break,_=notify.notified()=>{},_=tokio::time::sleep(delay)=>{} }
                // One outstanding tick at most. Reconcile wakes this timer;
                // no task captures an obsolete registration or generation.
                let acknowledged = tokio::select! {
                    _=token.cancelled()=>break,
                    reply=myself.call(Msg::Tick,None)=>reply,
                };
                match acknowledged {
                    Ok(ractor::rpc::CallResult::Success(next)) => delay = next,
                    _ => break,
                }
            }
        });
        Ok(State {
            jobs: BTreeMap::new(),
            scopes: BTreeMap::new(),
            active: BTreeMap::new(),
            generation: 0,
            maintenance: None,
            maintenance_due: Instant::now() + Duration::from_secs(60),
            faults: BTreeSet::new(),
            shutting_down: false,
            journal: args.journal,
            logs: args.logs,
            limits: args.limits,
            dropped: 0,
            missed: 0,
            timer: Some(timer),
            timer_cancel: cancel,
            dispatch: args.dispatch,
            clock: args.clock,
            closed: args.closed,
            wake,
        })
    }
    async fn handle(
        &self,
        myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Reconcile(scope, revision, jobs, reply) => {
                let result = state.reconcile(scope, revision, jobs);
                let _ = reply.send(result);
            }
            Msg::Submit(key, id, input, trigger, reply) => {
                match state.admit(
                    &myself,
                    Submission {
                        key,
                        id,
                        input,
                        trigger,
                        scheduled: None,
                        skip: false,
                    },
                ) {
                    Ok(receiver) => {
                        // response is owned by the execution task, not a detached waiter
                        let _ = receiver.send(reply);
                    }
                    Err(error) => {
                        let _ = reply.send(Err(error));
                    }
                }
            }
            Msg::Watch(id, reply) => {
                let _ = reply.send(state.active.get(&id).map(|a| a.result.subscribe()));
            }
            Msg::Cancel(id, reply) => {
                let answer = state.active.get_mut(&id).map(|a| {
                    if a.record.completion().is_some()
                        || matches!(a.record.state, RunState::Finalizing)
                    {
                        CancelResult::AlreadyFinished
                    } else if a.admitting {
                        CancelResult::AdmissionPending
                    } else if a.capability == Cancellation::Unsupported {
                        CancelResult::Unsupported
                    } else {
                        a.cancel.cancel();
                        a.record.state = RunState::Cancelling;
                        CancelResult::Requested
                    }
                });
                let _ = reply.send(answer);
            }
            Msg::Inspect(reply) => {
                let _ = reply.send(state.inspect());
            }
            Msg::Shutdown(reply) => {
                state.shutting_down = true;
                state.timer_cancel.cancel();
                for a in state.active.values() {
                    if a.capability == Cancellation::Cooperative {
                        a.cancel.cancel();
                    }
                }
                let _ = reply.send(());
            }
            Msg::Tick(reply) => {
                state.tick(&myself);
                let _ = reply.send(state.next_tick());
            }
            Msg::Started(id, record, reply) => {
                let start = !state.shutting_down;
                if let Some(active) = state.active.get_mut(&id) {
                    active.record = record;
                    active.admitting = false;
                    if start {
                        active.record.state = RunState::Running;
                    }
                }
                let _ = reply.send(start);
            }
            Msg::Fault(id) => {
                state.faults.insert(id);
            }
            Msg::Progress(id, record, uncertain) => {
                if let Some(active) = state.active.get_mut(&id) {
                    active.record = record.clone();
                    active.uncertain = uncertain;
                    if record.completion().is_some() {
                        active.result.send_replace(Some(record));
                    }
                }
            }
            Msg::MaintenanceDone(id) => {
                if let Some(task) = state.maintenance.take() {
                    let _ = task.await;
                }
                state.faults.remove(&id);
            }
            Msg::Done(id) => {
                if let Some(active) = state.active.remove(&id) {
                    let _ = active.task.await;
                }
                state.faults.remove(&id);
                state.logs.remove(id);
            }
        }
        if state.shutting_down && state.active.is_empty() && state.maintenance.is_none() {
            myself.stop(None);
        }
        Ok(())
    }
    async fn post_stop(
        &self,
        _: ActorRef<Msg>,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        state.timer_cancel.cancel();
        if let Some(timer) = state.timer.take() {
            let _ = timer.await;
        }
        // Also drain on unexpected actor termination. Client clones never expose
        // stop/abort, but an actor failure must not close the store under workers.
        for (_, active) in std::mem::take(&mut state.active) {
            let _ = active.task.await;
        }
        if let Some(task) = state.maintenance.take() {
            let _ = task.await;
        }
        state.closed.send_replace(true);
        Ok(())
    }
}
impl State {
    fn reconcile(&mut self, scope: String, revision: u64, jobs: Vec<Job>) -> Result<(), Error> {
        if self.shutting_down {
            return Err(Error::ShuttingDown);
        }
        if self
            .scopes
            .get(&scope)
            .is_some_and(|s| revision < s.source_revision)
        {
            return Err(Error::StaleRevision);
        }
        let validate = (|| {
            let mut keys = BTreeSet::new();
            for job in &jobs {
                job.validate_definition(self.limits.input_bytes, self.clock.now())?;
                if job.definition.scope != scope || !keys.insert(job.definition.key.clone()) {
                    return Err(Error::Invalid("scope/key mismatch".into()));
                }
                if self
                    .jobs
                    .get(&job.definition.key)
                    .is_some_and(|j| j.job.definition.scope != scope)
                {
                    return Err(Error::Invalid("job belongs to another scope".into()));
                }
            }
            if self
                .scopes
                .get(&scope)
                .is_some_and(|s| s.applied_revision == Some(revision))
            {
                let existing: Vec<_> = self
                    .jobs
                    .values()
                    .filter(|j| j.job.definition.scope == scope)
                    .collect();
                if existing.len() != jobs.len()
                    || jobs.iter().any(|j| {
                        !self
                            .jobs
                            .get(&j.definition.key)
                            .is_some_and(|r| r.job.same(j))
                    })
                {
                    return Err(Error::Invalid("same revision changed contents".into()));
                }
            }
            Ok(())
        })();
        let snapshot = self.scopes.entry(scope.clone()).or_default();
        snapshot.source_revision = revision;
        if let Err(error) = validate {
            snapshot.last_error = Some(error.to_string());
            return Err(error);
        }
        let keys: BTreeSet<_> = jobs.iter().map(|j| j.definition.key.clone()).collect();
        self.jobs
            .retain(|key, reg| reg.job.definition.scope != scope || keys.contains(key));
        for job in jobs {
            if self
                .jobs
                .get(&job.definition.key)
                .is_some_and(|r| r.job.same(&job))
            {
                continue;
            }
            self.generation += 1;
            let (deadline, next) = initial_due(&job.definition.schedule, self.clock.now());
            self.jobs.insert(
                job.definition.key.clone(),
                Registration {
                    job,
                    generation: self.generation,
                    deadline,
                    next,
                },
            );
        }
        let snapshot = self.scopes.get_mut(&scope).unwrap();
        snapshot.applied_revision = Some(revision);
        snapshot.last_error = None;
        self.wake.notify_one();
        Ok(())
    }
    fn inspect(&self) -> Inspection {
        Inspection {
            jobs: self
                .jobs
                .values()
                .map(|r| JobSnapshot {
                    definition: r.job.definition.clone(),
                    generation: r.generation,
                    next_run_at: r.next,
                    active_runs: self
                        .active
                        .iter()
                        .filter(|(_, a)| a.record.job == r.job.definition.key)
                        .map(|(id, _)| *id)
                        .collect(),
                })
                .collect(),
            scopes: self.scopes.clone(),
            active: self.active.values().map(|a| a.record.clone()).collect(),
            execution_uncertain: self
                .active
                .iter()
                .filter(|(_, a)| a.uncertain)
                .map(|(id, _)| *id)
                .collect(),
            journal_degraded: !self.faults.is_empty(),
            shutting_down: self.shutting_down,
            dropped_trigger_count: self.dropped,
            missed_trigger_count: self.missed,
        }
    }
    fn admit(
        &mut self,
        myself: &ActorRef<Msg>,
        submission: Submission,
    ) -> Result<ReplyHandoff, Error> {
        let Submission {
            key,
            id,
            input,
            trigger,
            scheduled,
            skip,
        } = submission;
        if self.active.contains_key(&id) {
            return Err(Error::AlreadySubmitted(id));
        }
        if self.shutting_down {
            return Err(Error::ShuttingDown);
        }
        if !self.faults.is_empty() {
            return Err(Error::Journal("admission closed".into()));
        }
        let job = self.jobs.get(&key).ok_or(Error::NotFound)?.job.clone();
        if self.active.len() >= self.limits.concurrency
            || (!skip && self.active.values().any(|a| a.record.job == key))
        {
            return Err(Error::Busy);
        }
        let input = input.unwrap_or_else(|| job.input.clone());
        if input.len() > self.limits.input_bytes {
            return Err(Error::Invalid("input limit".into()));
        }
        (job.validate)(&input)?;
        let record = RunRecord {
            id,
            job: key,
            definition_version: job.definition.version,
            trigger,
            scheduled_at: scheduled,
            admitted_at: self.clock.now(),
            admission_sequence: 0,
            finished_at: None,
            state: RunState::Admitted,
            last_log_sequence: 0,
            dropped_log_count: 0,
        };
        let cancel = CancellationToken::new();
        let (result, _) = watch::channel(None);
        let (send, receive) = oneshot::channel();
        let task = tokio::spawn(
            crate::runner::RunExecution {
                myself: myself.clone(),
                journal: self.journal.clone(),
                logs: self.logs.clone(),
                limits: self.limits.clone(),
                clock: self.clock.clone(),
                job: job.clone(),
                record: record.clone(),
                input,
                cancel: cancel.clone(),
                stopping: self.timer_cancel.clone(),
                reply: receive,
                skip,
            }
            .run()
            .with_subscriber(self.dispatch.clone()),
        );
        self.active.insert(
            id,
            Active {
                record,
                cancel,
                capability: job.definition.cancellation,
                result,
                task,
                admitting: true,
                uncertain: false,
            },
        );
        Ok(send)
    }
    fn next_tick(&self) -> Duration {
        let now = self.clock.now();
        let instant = Instant::now();
        self.jobs
            .values()
            .filter_map(|r| {
                r.deadline
                    .map(|d| d.saturating_duration_since(instant))
                    .or_else(|| {
                        r.next
                            .map(|d| Duration::try_from(d.duration_since(now)).unwrap_or_default())
                    })
            })
            .min()
            .unwrap_or(Duration::from_secs(5))
            .min(Duration::from_secs(5))
    }
    fn tick(&mut self, myself: &ActorRef<Msg>) {
        if self.shutting_down {
            return;
        }
        if self.maintenance.is_none() && Instant::now() >= self.maintenance_due {
            self.maintenance_due = Instant::now() + Duration::from_secs(60);
            let journal = self.journal.clone();
            let retention = self.limits.retention.clone();
            let now = self.clock.now();
            let myself = myself.clone();
            let timeout = self.limits.finalization_timeout;
            self.maintenance = Some(tokio::spawn(async move {
                crate::runner::maintain(myself, journal, retention, now, timeout).await;
            }));
        }
        let now = self.clock.now();
        let monotonic = Instant::now();
        let mut due = Vec::new();
        for (key, reg) in &mut self.jobs {
            let ready = reg
                .deadline
                .map(|d| d <= monotonic)
                .unwrap_or_else(|| reg.next.is_some_and(|d| d <= now));
            if !ready {
                continue;
            }
            let late = reg
                .deadline
                .map(|d| monotonic.saturating_duration_since(d))
                .unwrap_or_else(|| {
                    Duration::try_from(now.duration_since(reg.next.unwrap())).unwrap_or_default()
                });
            let scheduled = reg.next;
            let trigger = match reg.job.definition.schedule {
                Schedule::Once { .. } => Trigger::Once,
                Schedule::Interval { .. } => Trigger::Interval,
                Schedule::Cron { .. } => Trigger::Cron,
                Schedule::Manual => continue,
            };
            match &reg.job.definition.schedule {
                Schedule::Interval { every_ms } => {
                    let period = Duration::from_millis(*every_ms);
                    let remainder =
                        Duration::from_nanos((late.as_nanos() % period.as_nanos()) as u64);
                    let next = monotonic + (period - remainder);
                    reg.deadline = Some(next);
                    reg.next = Some(
                        now.saturating_add(next.saturating_duration_since(monotonic))
                            .unwrap_or(Timestamp::MAX),
                    );
                }
                Schedule::Cron { .. } => {
                    match reg
                        .job
                        .definition
                        .schedule
                        .next_cron(now.max(scheduled.unwrap()))
                    {
                        Ok(next) => reg.next = Some(next),
                        Err(error) => {
                            reg.next = None;
                            if let Some(s) = self.scopes.get_mut(&reg.job.definition.scope) {
                                s.last_error = Some(error.to_string());
                            }
                        }
                    }
                }
                _ => {
                    reg.deadline = None;
                    reg.next = None;
                }
            }
            if late > self.limits.lateness {
                self.missed = self.missed.saturating_add(1);
            } else {
                due.push((key.clone(), trigger, scheduled));
            }
        }
        for (key, trigger, scheduled) in due {
            let busy = self.active.values().any(|a| a.record.job == key);
            match self.admit(
                myself,
                Submission {
                    key,
                    id: RunId::new_v4(),
                    input: None,
                    trigger,
                    scheduled,
                    skip: busy,
                },
            ) {
                Ok(sender) => drop(sender),
                Err(_) => self.dropped = self.dropped.saturating_add(1),
            }
        }
    }
}
fn initial_due(schedule: &Schedule, now: Timestamp) -> (Option<Instant>, Option<Timestamp>) {
    match schedule {
        Schedule::Manual => (None, None),
        Schedule::Once { delay_ms } | Schedule::Interval { every_ms: delay_ms } => (
            Some(Instant::now() + Duration::from_millis(*delay_ms)),
            Some(
                now.saturating_add(Duration::from_millis(*delay_ms))
                    .unwrap_or(Timestamp::MAX),
            ),
        ),
        Schedule::Cron { .. } => (None, schedule.next_cron(now).ok()),
    }
}
