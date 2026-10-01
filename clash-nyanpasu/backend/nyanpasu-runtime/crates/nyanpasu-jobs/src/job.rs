use crate::{Schedule, model::*};
use futures_util::FutureExt;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tokio_util::{
    sync::CancellationToken,
    task::{TaskTracker, task_tracker::TaskTrackerToken},
};
use tracing::{Instrument, instrument::WithSubscriber};

struct Scope {
    tracker: TaskTracker,
    closed: bool,
}
#[derive(Clone)]
pub struct JobContext {
    pub run_id: RunId,
    pub trigger: Trigger,
    cancellation: CancellationToken,
    dispatch: tracing::Dispatch,
    // Narrow ownership gate for child/delegation creation racing handler return.
    // Business state remains inside its owning actor.
    scope: Arc<Mutex<Scope>>,
}
impl JobContext {
    pub(crate) fn new(run_id: RunId, trigger: Trigger, cancellation: CancellationToken) -> Self {
        Self {
            run_id,
            trigger,
            cancellation,
            dispatch: tracing::dispatcher::get_default(Clone::clone),
            scope: Arc::new(Mutex::new(Scope {
                tracker: TaskTracker::new(),
                closed: false,
            })),
        }
    }
    pub async fn cancelled(&self) {
        self.cancellation.cancelled().await;
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }
    pub fn span(&self) -> tracing::Span {
        tracing::dispatcher::with_default(
            &self.dispatch,
            || tracing::info_span!("job_run",job_run_id=%self.run_id),
        )
    }
    pub fn in_scope<T>(&self, f: impl FnOnce() -> T) -> T {
        tracing::dispatcher::with_default(&self.dispatch, || self.span().in_scope(f))
    }
    /// Explicitly propagate this context through actor messages, then instrument
    /// the actual processing future. Dropping a wait is not cancellation.
    pub async fn instrument<F: Future>(&self, future: F) -> F::Output {
        future
            .instrument(self.span())
            .with_subscriber(self.dispatch.clone())
            .await
    }
    pub fn spawn<F>(&self, future: F) -> Result<tokio::task::JoinHandle<F::Output>, Error>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let scope = self.scope.lock().unwrap();
        if scope.closed {
            return Err(Error::Stopped);
        }
        Ok(scope.tracker.spawn(
            future
                .instrument(self.span())
                .with_subscriber(self.dispatch.clone()),
        ))
    }
    /// Transfer this token to an external actor owning delegated work. It must
    /// retain the token until side effects have stopped, even if its RPC is lost.
    pub fn delegation(&self) -> Result<TaskTrackerToken, Error> {
        let scope = self.scope.lock().unwrap();
        if scope.closed {
            return Err(Error::Stopped);
        }
        Ok(scope.tracker.token())
    }
    pub(crate) async fn drain(&self) {
        let tracker = {
            let mut s = self.scope.lock().unwrap();
            s.closed = true;
            s.tracker.close();
            s.tracker.clone()
        };
        tracker.wait().await;
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct JobDefinition {
    pub key: JobKey,
    pub scope: String,
    pub managed_by: String,
    pub name: String,
    /// Change when handler behavior/default input changes. Compared by value;
    /// closures themselves are never compared or persisted.
    pub version: u64,
    pub schedule: Schedule,
    pub cancellation: Cancellation,
}
impl JobDefinition {
    pub fn manual(key: impl Into<String>) -> Self {
        let key = key.into();
        Self {
            name: key.clone(),
            key,
            scope: "default".into(),
            managed_by: "application".into(),
            version: 1,
            schedule: Schedule::Manual,
            cancellation: Cancellation::Unsupported,
        }
    }
}
pub(crate) type Execution = Pin<Box<dyn Future<Output = Completion> + Send>>;
type Validator = Arc<dyn Fn(&str) -> Result<(), Error> + Send + Sync>;
type Handler = Arc<dyn Fn(JobContext, String, usize) -> Execution + Send + Sync>;
/// Per-job journal capture; ordinary tracing subscribers remain unaffected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LogCaptureMode {
    /// Use the injected LogCapture allowlist and limits.
    #[default]
    Inherit,
    /// Retain run history and context without capturing tracing events.
    Disabled,
}
#[derive(Clone)]
pub struct Job {
    pub definition: JobDefinition,
    pub(crate) log_capture: LogCaptureMode,
    pub(crate) input: String,
    pub(crate) validate: Validator,
    pub(crate) handler: Handler,
}
impl Job {
    pub fn new<I, O, F, Fut>(
        definition: JobDefinition,
        default_input: I,
        handler: F,
    ) -> Result<Self, Error>
    where
        I: Serialize + DeserializeOwned + Send + 'static,
        O: Serialize + Send + 'static,
        F: Fn(JobContext, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, JobError>> + Send + 'static,
    {
        let input = crate::encoding::json(&default_input, 64 * 1024)
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let handler = Arc::new(handler);
        Ok(Self {
            definition,
            log_capture: LogCaptureMode::default(),
            input,
            validate: Arc::new(|text| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    serde_json::from_str::<I>(text)
                }))
                .map_err(|_| Error::Invalid("input decoder panicked".into()))?
                .map(|_| ())
                .map_err(|e| Error::Invalid(e.to_string()))
            }),
            handler: Arc::new(move |ctx, text, limit| {
                let handler = handler.clone();
                Box::pin(async move {
                    let operation = async {
                        let input = serde_json::from_str(&text)
                            .map_err(|_| JobError::new("input_invalid", "Input schema changed"))?;
                        handler(ctx, input).await
                    };
                    match std::panic::AssertUnwindSafe(operation).catch_unwind().await {
                        Ok(Ok(output)) => Completion {
                            outcome: Outcome::Succeeded,
                            output: match crate::encoding::json(&output, limit) {
                                Ok(text) => Output::Json(text),
                                Err(Error::OutputUnavailable(code)) => Output::Unavailable(code),
                                Err(_) => Output::Unavailable("encoding_failed".into()),
                            },
                            journal: JournalState::Durable,
                        },
                        Ok(Err(error)) => Completion {
                            outcome: if error.code == "cancelled" {
                                Outcome::Cancelled
                            } else {
                                Outcome::Failed(error.bounded())
                            },
                            output: Output::None,
                            journal: JournalState::Durable,
                        },
                        Err(_) => Completion {
                            outcome: Outcome::Failed(JobError::new("panicked", "Handler panicked")),
                            output: Output::None,
                            journal: JournalState::Durable,
                        },
                    }
                })
            }),
        })
    }
    pub fn with_log_capture(mut self, mode: LogCaptureMode) -> Self {
        self.log_capture = mode;
        self
    }
    pub fn blocking<I, O, F>(
        definition: JobDefinition,
        default_input: I,
        handler: F,
    ) -> Result<Self, Error>
    where
        I: Serialize + DeserializeOwned + Send + 'static,
        O: Serialize + Send + 'static,
        F: Fn(JobContext, I) -> Result<O, JobError> + Send + Sync + 'static,
    {
        let handler = Arc::new(handler);
        Self::new(definition, default_input, move |ctx, input| {
            let handler = handler.clone();
            let context = ctx.clone();
            async move {
                tokio::task::spawn_blocking(move || context.in_scope(|| handler(ctx, input)))
                    .await
                    .map_err(|_| JobError::new("panicked", "Blocking handler panicked"))?
            }
        })
    }
    pub(crate) fn validate_definition(
        &self,
        limit: usize,
        now: jiff::Timestamp,
    ) -> Result<(), Error> {
        if self.definition.key.is_empty()
            || self.definition.scope.is_empty()
            || self.definition.key.len() > 512
            || self.definition.scope.len() > 128
            || self.definition.name.len() > 1024
            || self.definition.managed_by.len() > 512
        {
            return Err(Error::Invalid("invalid job identity/metadata".into()));
        }
        if self.input.len() > limit {
            return Err(Error::Invalid("input limit".into()));
        }
        (self.validate)(&self.input)?;
        self.definition.schedule.validate(now)
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.definition == other.definition
            && self.input == other.input
            && self.log_capture == other.log_capture
    }
}
