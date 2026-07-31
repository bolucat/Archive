//! Generic child-process management: spawn, event stream, kill, supervise.
//!
//! `processkit` is the internal engine and must not appear in any public signature
//! (only `engine.rs` in the library imports it; the `containment_probe` example
//! also uses it). Event ordering: [`ProcessEvent::Terminated`], when delivered,
//! is the final event on the channel, but delivery is best-effort under a
//! non-draining receiver — use [`ProcessHandle::wait`] as the authoritative
//! termination signal.
//!
//! ```no_run
//! use nyanpasu_utils::process::{Command, ProcessEvent};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let (handle, mut events) = Command::new("mihomo")
//!     .args(["-d", "/etc/mihomo"])
//!     .pid_file("/run/mihomo.pid")
//!     .spawn()
//!     .await?;
//! while let Some(event) = events.recv().await {
//!     match event {
//!         ProcessEvent::Stdout(line) => eprintln!("stdout: {line}"),
//!         ProcessEvent::Stderr(line) => eprintln!("stderr: {line}"),
//!         ProcessEvent::Error(error) => eprintln!("pump: {error}"),
//!         ProcessEvent::Terminated(payload) => {
//!             eprintln!("exited: {payload:?}");
//!             break;
//!         }
//!         _ => {}
//!     }
//! }
//! handle.graceful_kill().await.ok();
//! # Ok(())
//! # }
//! ```
//!
//! Design: docs/superpowers/specs/2026-07-16-nyanpasu-utils-process-module-design.md

mod command;
mod engine;
mod error;
mod event;
mod handle;
mod pid_file;
mod supervisor;

pub use command::Command;
pub use error::{ProcessError, ProcessOutput};
pub use event::{ProcessEvent, TerminatedPayload};
pub use handle::{Containment, ProcessHandle};
pub use pid_file::{
    EpochPidFile, EpochPidRecord, OrphanReapOutcome, read_epoch_pid_file, reap_epoch_pid_file,
};
pub use supervisor::{
    Backoff, ReadinessProbe, RestartPolicy, RestartStormPolicy, Supervisor, SupervisorBuilder,
    SupervisorEvent,
};
