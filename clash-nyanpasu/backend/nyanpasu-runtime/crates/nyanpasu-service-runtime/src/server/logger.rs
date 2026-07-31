use std::{
    borrow::Cow,
    sync::{Arc, OnceLock},
};

use bounded_vec_deque::BoundedVecDeque;
use indexmap::IndexMap;
use parking_lot::Mutex;
use tracing_subscriber::fmt::MakeWriter;

const LOG_BUFFER_CAPACITY: usize = 100;

pub type LoggerSubscriber = Box<dyn Fn(TracingLogging) + Send + Sync + 'static>;

pub struct Logger<'n> {
    buffer: Arc<Mutex<BoundedVecDeque<Cow<'n, str>>>>,
    subscriber: Arc<OnceLock<LoggerSubscriber>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct TracingLogging {
    pub level: String,
    pub timestamp: String,
    #[serde(flatten)]
    pub fields: IndexMap<String, serde_json::Value>,
}

impl Clone for Logger<'_> {
    fn clone(&self) -> Self {
        Logger {
            buffer: self.buffer.clone(),
            subscriber: self.subscriber.clone(),
        }
    }
}

impl<'n> Logger<'n> {
    pub fn new() -> Self {
        Logger {
            buffer: Arc::new(Mutex::new(BoundedVecDeque::new(LOG_BUFFER_CAPACITY))),
            subscriber: Arc::new(OnceLock::new()),
        }
    }

    pub fn global() -> &'static Logger<'static> {
        static INSTANCE: OnceLock<Logger> = OnceLock::new();
        INSTANCE.get_or_init(Logger::new)
    }

    pub fn set_subscriber(
        &self,
        subscriber: Box<dyn Fn(TracingLogging) + Send + Sync + 'static>,
    ) -> bool {
        self.subscriber.set(subscriber).is_ok()
    }

    /// Retrieve all logs in the buffer
    /// It should clear the buffer after retrieve
    pub fn retrieve_logs(&self) -> Vec<Cow<'n, str>> {
        let mut buffer = self.buffer.lock();
        buffer.drain(..).collect()
    }

    /// Inspect all logs in the buffer
    /// It should not clear the buffer after inspect
    pub fn inspect_logs(&self) -> Vec<Cow<'n, str>> {
        let buffer = self.buffer.lock();
        buffer.iter().cloned().collect()
    }
}

impl<'n> Default for Logger<'n> {
    fn default() -> Self {
        Self::new()
    }
}

impl std::io::Write for Logger<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let msg = String::from_utf8_lossy(buf);
        let forward = self.subscriber.get().and_then(|subscriber| {
            serde_json::from_str::<TracingLogging>(&msg)
                .ok()
                .map(|logging| (subscriber, logging))
        });
        self.buffer.lock().push_back(Cow::Owned(msg.into_owned()));
        // Outside the lock: the subscriber now fans out to the event hub inline
        // instead of spawning, so holding the buffer mutex across it would block
        // every other log writer — and would deadlock outright if it ever logged.
        if let Some((subscriber, logging)) = forward {
            subscriber(logging);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Logger<'static> {
    type Writer = Logger<'static>;

    fn make_writer(&'a self) -> Self::Writer {
        Logger {
            buffer: self.buffer.clone(),
            subscriber: self.subscriber.clone(),
        }
    }
}
