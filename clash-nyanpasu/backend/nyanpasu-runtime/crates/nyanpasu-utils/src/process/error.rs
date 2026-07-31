use std::time::Duration;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ProcessError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to spawn `{program}`: {message}")]
    Spawn { program: String, message: String },
    #[error("process timed out after {after:?}")]
    Timeout { after: Duration },
    #[error("process already exited")]
    AlreadyExited,
    #[error("stdin is not piped (enable Command::pipe_stdin) or already closed")]
    StdinUnavailable,
    /// Engine-internal failures that have no dedicated variant. The engine maps
    /// processkit errors to strings here so processkit types never leak.
    #[error("process engine error: {0}")]
    Engine(String),
}

/// Result of [`crate::process::Command::output`] — one-shot capture.
#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl ProcessOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_output_success_only_on_zero() {
        let mk = |code| ProcessOutput {
            code,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(mk(Some(0)).success());
        assert!(!mk(Some(1)).success());
        assert!(!mk(None).success());
    }

    #[test]
    fn error_display_is_stable() {
        let e = ProcessError::Spawn {
            program: "mihomo".into(),
            message: "not found".into(),
        };
        assert_eq!(e.to_string(), "failed to spawn `mihomo`: not found");
    }
}
