use std::{
    borrow::Cow,
    sync::{Arc, OnceLock},
};

use bounded_vec_deque::BoundedVecDeque;
use parking_lot::Mutex;
use tracing_subscriber::fmt::MakeWriter;

const LOG_BUFFER_CAPACITY: usize = 100;

/// The in-memory tail `/logs/retrieve` and `/logs/inspect` serve, and the
/// `MakeWriter` that fills it.
///
/// It used to fan every tracing event out to the event hub as well, which is
/// how the service's own logs reached the socket. That path is gone: the logs
/// are files, and `/status` reports where.
pub struct Logger<'n> {
    buffer: Arc<Mutex<BoundedVecDeque<Cow<'n, str>>>>,
}

impl Clone for Logger<'_> {
    fn clone(&self) -> Self {
        Logger {
            buffer: self.buffer.clone(),
        }
    }
}

impl<'n> Logger<'n> {
    pub fn new() -> Self {
        Logger {
            buffer: Arc::new(Mutex::new(BoundedVecDeque::new(LOG_BUFFER_CAPACITY))),
        }
    }

    pub fn global() -> &'static Logger<'static> {
        static INSTANCE: OnceLock<Logger> = OnceLock::new();
        INSTANCE.get_or_init(Logger::new)
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
        self.buffer.lock().push_back(Cow::Owned(msg.into_owned()));
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
        }
    }
}
