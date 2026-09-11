//! Bounded, process-local log indexing. No Tauri or global service dependencies.
mod index;
pub use index::*;
mod protocol;
pub use protocol::*;
mod files;
pub use files::{FileRead, FsLogFiles, LogFiles};
mod session;
pub use session::{Clock, LogsClient, MonotonicClock};
