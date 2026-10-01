use crate::{
    actor::Msg,
    job::{Job, JobContext, LogCaptureMode},
    journal::Journal,
    logging::LogCapture,
    model::*,
};
use ractor::{ActorRef, RpcReplyPort};
use std::{sync::Arc, time::Duration};
use tokio::{sync::oneshot, time::Instant};
use tokio_util::sync::CancellationToken;
use tracing::instrument::WithSubscriber;

async fn fault(myself: &ActorRef<Msg>, id: RunId) {
    let _ = myself.cast(Msg::Fault(id));
}
async fn retry_health(journal: &Journal) {
    let mut delay = 1;
    loop {
        tokio::time::sleep(Duration::from_secs(delay)).await;
        if journal.call(|s| s.health()).await.is_ok() {
            return;
        }
        delay = (delay * 2).min(30);
    }
}
pub(crate) struct RunExecution {
    pub myself: ActorRef<Msg>,
    pub journal: Journal,
    pub logs: LogCapture,
    pub limits: Arc<Limits>,
    pub clock: Arc<dyn crate::Clock>,
    pub job: Job,
    pub record: RunRecord,
    pub input: String,
    pub cancel: CancellationToken,
    pub stopping: CancellationToken,
    pub reply: oneshot::Receiver<RpcReplyPort<Result<(), Error>>>,
    pub skip: bool,
}
impl RunExecution {
    pub async fn run(self) {
        let Self {
            myself,
            journal,
            logs,
            limits,
            clock,
            job,
            mut record,
            input,
            cancel,
            stopping,
            reply,
            skip,
        } = self;
        let id = record.id;
        let mut reply = reply.await.ok();
        let pending = journal.call({
            let record = record.clone();
            move |s| s.admit(record)
        });
        tokio::pin!(pending);
        let admission = tokio::select! {
            result=&mut pending=>result,
            _=tokio::time::sleep(limits.control_timeout)=>{
                fault(&myself,id).await;
                if let Some(reply)=reply.take(){let _=reply.send(Err(Error::AdmissionUnknown(id)));}
                pending.await
            }
        };
        match admission {
            Ok(saved) => record = saved,
            Err(error) => {
                if !matches!(error, Error::AlreadySubmitted(_)) {
                    fault(&myself, id).await;
                }
                if let Some(reply) = reply.take() {
                    let response = match &error {
                        Error::AlreadySubmitted(_) | Error::AdmissionFailed(_) => error.clone(),
                        _ => Error::AdmissionUnknown(id),
                    };
                    let _ = reply.send(Err(response));
                }
                if matches!(error, Error::AlreadySubmitted(_)) {
                    let _ = myself.cast(Msg::Done(id));
                    return;
                }
                retry_health(&journal).await;
                if matches!(error, Error::AdmissionFailed(_)) {
                    let _ = myself.cast(Msg::Done(id));
                    return;
                }
                // A commit error may still have reached durable storage. Read
                // back this exact ID; never submit a second admission transaction.
                loop {
                    match journal.call(move |s| s.get(id)).await {
                        Ok(Some(saved)) => {
                            record = saved;
                            break;
                        }
                        Ok(None) => {
                            let _ = myself.cast(Msg::Done(id));
                            return;
                        }
                        Err(_) => retry_health(&journal).await,
                    }
                }
                if record.completion().is_some() {
                    let _ = myself.cast(Msg::Progress(id, record, false));
                    let _ = myself.cast(Msg::Done(id));
                    return;
                }
            }
        }
        let (start, decision) = oneshot::channel();
        let _ = myself.cast(Msg::Started(id, record.clone(), start));
        let start = tokio::select! {
            _=stopping.cancelled()=>false,
            decision=decision=>decision.unwrap_or(false),
        };
        if let Some(reply) = reply.take() {
            let _ = reply.send(Ok(()));
        }
        let buffer = (job.log_capture == LogCaptureMode::Inherit).then(|| logs.register(id));
        let context = JobContext::new(id, record.trigger.clone(), cancel);
        let mut pending_logs = Vec::new();
        let mut append: Option<futures_util::future::BoxFuture<'static, Result<(), Error>>> = None;
        let mut append_failed = false;
        let mut append_deadline = Instant::now();
        let mut append_timed_out = false;
        let completion = if !start || skip {
            Completion {
                outcome: if skip {
                    Outcome::Skipped
                } else {
                    Outcome::Cancelled
                },
                output: Output::None,
                journal: JournalState::Durable,
            }
        } else {
            record.state = RunState::Running;
            let child_context = context.clone();
            let owner = myself.clone();
            let running = record.clone();
            let output_limit = limits.output_bytes;
            let mut task = tokio::spawn(
                async move {
                    let completion = child_context
                        .instrument((job.handler)(child_context.clone(), input, output_limit))
                        .await;
                    let uncertain =
                        matches!(&completion.outcome,Outcome::Failed(e) if e.code=="panicked");
                    let _ = owner.cast(Msg::Progress(id, running, uncertain));
                    child_context.drain().await;
                    completion
                }
                .with_current_subscriber(),
            );
            loop {
                tokio::select! {
                    result=&mut task=>break result.unwrap_or_else(|_|Completion {outcome:Outcome::Failed(JobError::new("panicked","Execution wrapper panicked")),output:Output::None,journal:JournalState::Durable}),
                    _=tokio::time::sleep_until(append_deadline),if append.is_some()&&!append_timed_out=>{append_timed_out=true;fault(&myself,id).await;}
                    result=async {append.as_mut().unwrap().await},if append.is_some()=>{
                        append=None;
                        match result {Ok(())=>pending_logs.clear(),Err(_)=>{append_failed=true;fault(&myself,id).await;}}
                    }
                    _=tokio::time::sleep(Duration::from_millis(100)),if buffer.is_some()&&append.is_none()&&!append_failed=>{
                        pending_logs.extend(buffer.as_ref().unwrap().lock().unwrap().drain());
                        if !pending_logs.is_empty(){
                            let batch=pending_logs.clone();let journal=journal.clone();
                            append_deadline=Instant::now()+limits.finalization_timeout;append_timed_out=false;
                            append=Some(Box::pin(async move {journal.call(move|s|s.append(id,&batch)).await}));
                        }
                    }
                }
            }
        };
        context.drain().await;
        record.state = RunState::Finalizing;
        let _ = myself.cast(Msg::Progress(id, record.clone(), false));
        let (tail, sequence, dropped) = buffer
            .map(|buffer| buffer.lock().unwrap().seal())
            .unwrap_or_default();
        pending_logs.extend(tail);
        record.last_log_sequence = sequence;
        record.dropped_log_count = dropped;
        record.finished_at = Some(clock.now());
        record.state = RunState::Finished(completion);
        // Include an outstanding log write in the finalization budget. Never drop
        // an uncertain write or enqueue a duplicate while the original is blocked.
        if let Some(mut write) = append {
            tokio::select! {
                _=&mut write=>{},
                _=tokio::time::sleep(limits.finalization_timeout)=>{
                    publish_degraded(&myself,&record,"log_flush_timeout");let _=write.await;
                }
            }
        }
        let mut delay = 1;
        loop {
            let saved = record.clone();
            let batch = pending_logs.clone();
            let write = journal.call(move |s| s.finish(&saved, &batch));
            tokio::pin!(write);
            let result = tokio::select! {
                result=&mut write=>result,
                _=tokio::time::sleep(limits.finalization_timeout)=>{
                    publish_degraded(&myself,&record,"finalization_timeout");write.await
                }
            };
            match result {
                Ok(()) => break,
                Err(_) => {
                    publish_degraded(&myself, &record, "commit_failed");
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    delay = (delay * 2).min(30);
                }
            }
        }
        let _ = myself.cast(Msg::Progress(id, record, false));
        // Retention is bounded by committed records; never prune an active Run.
        let retention = limits.retention.clone();
        let now = clock.now();
        if journal
            .call(move |s| s.prune(now, &retention))
            .await
            .is_err()
        {
            fault(&myself, id).await;
            retry_health(&journal).await;
        }
        let _ = myself.cast(Msg::Done(id));
    }
}

fn publish_degraded(myself: &ActorRef<Msg>, record: &RunRecord, code: &str) {
    let mut record = record.clone();
    if let RunState::Finished(c) = &mut record.state {
        c.journal = JournalState::Degraded(code.into());
    }
    let _ = myself.cast(Msg::Fault(record.id));
    let _ = myself.cast(Msg::Progress(record.id, record, false));
}

/// Idle retention has its own bounded system owner, independent of job permits.
pub(crate) async fn maintain(
    myself: ActorRef<Msg>,
    journal: Journal,
    retention: Retention,
    now: jiff::Timestamp,
    timeout: Duration,
) {
    let id = RunId::new_v4();
    let mut delay = 1;
    loop {
        let retention = retention.clone();
        let write = journal.call(move |s| s.prune(now, &retention));
        tokio::pin!(write);
        let result = tokio::select! {
            result=&mut write=>result,
            _=tokio::time::sleep(timeout)=>{fault(&myself,id).await;write.await}
        };
        match result {
            Ok(()) => break,
            Err(_) => {
                fault(&myself, id).await;
                tokio::time::sleep(Duration::from_secs(delay)).await;
                delay = (delay * 2).min(30);
            }
        }
    }
    let _ = myself.cast(Msg::MaintenanceDone(id));
}
