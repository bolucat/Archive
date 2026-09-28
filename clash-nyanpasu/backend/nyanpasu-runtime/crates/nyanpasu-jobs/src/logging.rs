use crate::{RunId, RunLog};
use jiff::Timestamp;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicUsize, Ordering},
    },
};
use tracing::{
    Event, Subscriber,
    field::{Field, Visit},
    span::{Attributes, Id},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

#[derive(Debug, Clone)]
pub struct LogPolicy {
    /// Exact target -> allowed field names. Empty by default: deny all events.
    /// Allow `message` only if every event at this target uses safe fixed text.
    pub targets: BTreeMap<String, BTreeSet<String>>,
    pub level: tracing::Level,
    pub queue_entries: usize,
    pub entry_bytes: usize,
    pub run_entries: usize,
    pub run_bytes: usize,
}
impl Default for LogPolicy {
    fn default() -> Self {
        Self {
            targets: BTreeMap::new(),
            level: tracing::Level::INFO,
            queue_entries: 1024,
            entry_bytes: 4096,
            run_entries: 4096,
            run_bytes: 1024 * 1024,
        }
    }
}
#[derive(Default)]
pub(crate) struct Buffer {
    queue: VecDeque<RunLog>,
    budget: Arc<AtomicUsize>,
    sequence: u64,
    dropped: u64,
    accepted: usize,
    bytes: usize,
    sealed: bool,
}
impl Buffer {
    pub fn drain(&mut self) -> Vec<RunLog> {
        let count = self.queue.len().min(128);
        self.budget.fetch_sub(count, Ordering::Relaxed);
        self.queue.drain(..count).collect()
    }
    pub fn seal(&mut self) -> (Vec<RunLog>, u64, u64) {
        self.sealed = true;
        self.budget.fetch_sub(self.queue.len(), Ordering::Relaxed);
        (self.queue.drain(..).collect(), self.sequence, self.dropped)
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        self.budget.fetch_sub(self.queue.len(), Ordering::Relaxed);
    }
}
/// Synchronization is restricted to bounded tracing ingress, not JobsActor state.
/// Span callbacks run on arbitrary producer threads; the owner drains each buffer.
#[derive(Clone)]
pub struct LogCapture {
    policy: Arc<LogPolicy>,
    budget: Arc<AtomicUsize>,
    runs: Arc<Mutex<BTreeMap<RunId, Weak<Mutex<Buffer>>>>>,
}
impl LogCapture {
    pub fn new(policy: LogPolicy) -> Self {
        Self {
            policy: Arc::new(policy),
            budget: Arc::default(),
            runs: Arc::default(),
        }
    }
    pub fn layer(&self) -> JobJournalLayer {
        JobJournalLayer {
            capture: self.clone(),
        }
    }
    pub(crate) fn register(&self, id: RunId) -> Arc<Mutex<Buffer>> {
        let buffer = Arc::new(Mutex::new(Buffer {
            budget: self.budget.clone(),
            queue: VecDeque::new(),
            sequence: 0,
            dropped: 0,
            accepted: 0,
            bytes: 0,
            sealed: false,
        }));
        self.runs
            .lock()
            .unwrap()
            .insert(id, Arc::downgrade(&buffer));
        buffer
    }
    pub(crate) fn remove(&self, id: RunId) {
        self.runs.lock().unwrap().remove(&id);
    }
}
pub struct JobJournalLayer {
    capture: LogCapture,
}
#[derive(Clone)]
struct SpanRun(RunId);
struct RunVisitor(Option<RunId>);
impl Visit for RunVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "job_run_id" {
            self.0 = format!("{value:?}").trim_matches('"').parse().ok();
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "job_run_id" {
            self.0 = value.parse().ok();
        }
    }
}
struct Fields<'a> {
    allow: &'a BTreeSet<String>,
    values: BTreeMap<String, String>,
    max: usize,
    overflow: bool,
}
impl Visit for Fields<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if self.allow.contains(field.name()) {
            // Bound formatting itself; a Debug implementation may emit huge data.
            struct Limited {
                text: String,
                remaining: usize,
                overflow: bool,
            }
            impl std::fmt::Write for Limited {
                fn write_str(&mut self, s: &str) -> std::fmt::Result {
                    if s.len() > self.remaining {
                        self.overflow = true;
                        return Err(std::fmt::Error);
                    }
                    self.remaining -= s.len();
                    self.text.push_str(s);
                    Ok(())
                }
            }
            let mut out = Limited {
                text: String::new(),
                remaining: self.max,
                overflow: false,
            };
            let _ = std::fmt::write(&mut out, format_args!("{value:?}"));
            self.overflow |= out.overflow;
            self.values.insert(field.name().into(), out.text);
        }
    }
}
impl<S> Layer<S> for JobJournalLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut visitor = RunVisitor(None);
        attrs.record(&mut visitor);
        if let (Some(run), Some(span)) = (visitor.0, ctx.span(id)) {
            span.extensions_mut().insert(SpanRun(run));
        }
    }
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let policy = &self.capture.policy;
        if *event.metadata().level() > policy.level {
            return;
        }
        let Some(allow) = policy.targets.get(event.metadata().target()) else {
            return;
        };
        let Some(mut scope) = ctx.event_scope(event) else {
            return;
        };
        let Some(id) = scope.find_map(|s| s.extensions().get::<SpanRun>().map(|r| r.0)) else {
            return;
        };
        let Some(buffer) = self
            .capture
            .runs
            .lock()
            .unwrap()
            .get(&id)
            .and_then(Weak::upgrade)
        else {
            return;
        };
        let mut fields = Fields {
            allow,
            values: BTreeMap::new(),
            max: policy.entry_bytes,
            overflow: false,
        };
        event.record(&mut fields);
        let mut buffer = buffer.lock().unwrap();
        if buffer.sealed {
            return;
        }
        buffer.sequence = buffer.sequence.saturating_add(1);
        let log = RunLog {
            run_id: id,
            sequence: buffer.sequence,
            time: Timestamp::now(),
            level: event.metadata().level().to_string(),
            target: event.metadata().target().into(),
            fields: fields.values,
        };
        let bytes = serde_json::to_vec(&log)
            .map(|v| v.len())
            .unwrap_or(usize::MAX);
        if fields.overflow
            || bytes > policy.entry_bytes
            || buffer.accepted >= policy.run_entries
            || buffer.bytes.saturating_add(bytes) > policy.run_bytes
        {
            buffer.dropped = buffer.dropped.saturating_add(1);
            return;
        }
        if buffer
            .budget
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                (n < policy.queue_entries).then_some(n + 1)
            })
            .is_err()
        {
            buffer.dropped = buffer.dropped.saturating_add(1);
            return;
        }
        buffer.bytes += bytes;
        buffer.accepted += 1;
        buffer.queue.push_back(log);
    }
}
