use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
    time::Duration,
};

use super::pid_file::EpochPidFile;

pub(crate) enum PidFile {
    Legacy(PathBuf),
    Epoch(EpochPidFile),
}

/// Builder for spawning a managed child process. See module docs for the contract.
pub struct Command {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) envs: Vec<(OsString, OsString)>,
    pub(crate) current_dir: Option<PathBuf>,
    pub(crate) encoding: Option<&'static encoding_rs::Encoding>,
    pub(crate) hide_window: bool,
    pub(crate) kill_grace: Duration,
    pub(crate) event_channel_capacity: usize,
    pub(crate) timeout: Option<Duration>,
    pub(crate) pipe_stdin: bool,
    pub(crate) pid_file: Option<PidFile>,
}

impl Command {
    /// Creates a command for `program` with process-module defaults.
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            envs: Vec::new(),
            current_dir: None,
            encoding: None,
            hide_window: true,
            kill_grace: Duration::from_secs(5),
            event_channel_capacity: 64,
            timeout: None,
            pipe_stdin: false,
            pid_file: None,
        }
    }

    /// Appends one command-line argument.
    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_os_string());
        self
    }

    /// Appends command-line arguments in iteration order.
    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|a| a.as_ref().to_os_string()));
        self
    }

    /// Sets an environment variable for the child.
    pub fn env(mut self, k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) -> Self {
        self.envs
            .push((k.as_ref().to_os_string(), v.as_ref().to_os_string()));
        self
    }

    /// Sets the child's working directory.
    pub fn current_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(dir.into());
        self
    }

    /// Sets the encoding used to decode stdout and stderr lines.
    pub fn encoding(mut self, enc: Option<&'static encoding_rs::Encoding>) -> Self {
        self.encoding = enc;
        self
    }

    /// Controls whether the child window is hidden on Windows.
    pub fn hide_window(mut self, hide: bool) -> Self {
        self.hide_window = hide;
        self
    }

    /// Sets the grace period between graceful termination and a hard kill.
    pub fn kill_grace(mut self, d: Duration) -> Self {
        self.kill_grace = d;
        self
    }

    /// Sets the process-event channel capacity.
    ///
    /// A full channel pauses the event pump and therefore pipe reads. Receivers
    /// should drain promptly: once the engine's 256 KiB output ring fills, its
    /// oldest lines are silently dropped by design.
    pub fn event_channel_capacity(mut self, cap: usize) -> Self {
        self.event_channel_capacity = cap.max(1);
        self
    }

    /// Sets the maximum process lifetime before the whole process tree is killed.
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = Some(d);
        self
    }

    /// Enables or disables a writable stdin pipe for the child.
    pub fn pipe_stdin(mut self, pipe: bool) -> Self {
        self.pipe_stdin = pipe;
        self
    }

    /// Records the child pid at `path` in the legacy numeric format.
    ///
    /// Numeric records cannot prove epoch/start identity and therefore do not
    /// authorize residual-process killing. Use [`Command::epoch_pid_file`] when
    /// post-crash orphan reaping is required.
    pub fn pid_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.pid_file = Some(PidFile::Legacy(path.into()));
        self
    }

    /// Records full per-epoch ownership for validated residual-process cleanup.
    pub fn epoch_pid_file(mut self, spec: EpochPidFile) -> Self {
        self.pid_file = Some(PidFile::Epoch(spec));
        self
    }

    /// Spawns the child. `ProcessEvent::Terminated`, when delivered, is the
    /// final event on the channel; use [`super::handle::ProcessHandle::wait`]
    /// for the authoritative termination signal.
    ///
    /// Event delivery applies backpressure: a full channel pauses the pump and
    /// pipe reads, and output beyond the engine's 256 KiB ring silently drops
    /// the oldest lines. Drain the receiver promptly. If a receiver stops
    /// draining during termination, buffered output — including the terminal
    /// events — is dropped after five seconds so
    /// [`super::handle::ProcessHandle::kill`],
    /// [`super::handle::ProcessHandle::graceful_kill`], and
    /// [`super::handle::ProcessHandle::wait`] remain live.
    pub async fn spawn(
        self,
    ) -> Result<
        (
            super::handle::ProcessHandle,
            tokio::sync::mpsc::Receiver<super::event::ProcessEvent>,
        ),
        super::error::ProcessError,
    > {
        let parts = super::engine::spawn(self).await?;
        let handle = super::handle::ProcessHandle {
            pid: parts.pid,
            containment: parts.containment,
            ctrl: parts.ctrl_tx,
            terminated: parts.terminated_rx,
        };
        Ok((handle, parts.events_rx))
    }

    /// One-shot run capturing stdout/stderr. A non-zero exit is data, not an error;
    /// launch failures, timeouts, and execution-engine failures are `Err`.
    pub async fn output(self) -> Result<super::error::ProcessOutput, super::error::ProcessError> {
        super::engine::run_capture(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn defaults_match_design() {
        let c = Command::new("prog");
        assert_eq!(c.event_channel_capacity, 64);
        assert_eq!(c.kill_grace, Duration::from_secs(5));
        assert!(c.hide_window);
        assert!(!c.pipe_stdin);
        assert!(c.encoding.is_none());
        assert!(c.pid_file.is_none());
        assert!(c.timeout.is_none());
    }

    #[test]
    fn builder_chain_sets_fields() {
        let c = Command::new("prog")
            .arg("-v")
            .args(["a", "b"])
            .env("K", "V")
            .current_dir("C:/tmp")
            .kill_grace(Duration::from_secs(1))
            .event_channel_capacity(8)
            .pipe_stdin(true)
            .hide_window(false);
        assert_eq!(c.args.len(), 3);
        assert_eq!(c.envs.len(), 1);
        assert_eq!(c.event_channel_capacity, 8);
        assert!(c.pipe_stdin);
        assert!(!c.hide_window);
    }
}
