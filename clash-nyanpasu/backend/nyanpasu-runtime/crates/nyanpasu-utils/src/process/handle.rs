use tokio::sync::{mpsc, oneshot, watch};

use super::{error::ProcessError, event::TerminatedPayload};

/// The kernel containment mechanism actually in effect (mirrors the engine's report).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Containment {
    JobObject,
    CgroupV2,
    ProcessGroup,
}

pub(crate) enum Ctrl {
    GracefulKill(oneshot::Sender<Result<(), ProcessError>>),
    Kill(oneshot::Sender<Result<(), ProcessError>>),
    WriteStdin(Vec<u8>, oneshot::Sender<Result<(), ProcessError>>),
}

/// Cloneable handle to a spawned child. Dropping all handles and the event
/// receiver kills the whole process tree.
#[derive(Clone)]
pub struct ProcessHandle {
    pub(crate) pid: u32,
    pub(crate) containment: Containment,
    pub(crate) ctrl: mpsc::Sender<Ctrl>,
    pub(crate) terminated: watch::Receiver<Option<Result<TerminatedPayload, String>>>,
}

impl ProcessHandle {
    /// Returns the operating-system process identifier of the direct child.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Returns the whole-tree containment mechanism used for this child.
    pub fn containment(&self) -> Containment {
        self.containment
    }

    /// Waits until the child terminates; returns immediately if it already has.
    pub async fn wait(&self) -> Result<TerminatedPayload, ProcessError> {
        let mut rx = self.terminated.clone();
        loop {
            if let Some(result) = rx.borrow().clone() {
                return result.map_err(ProcessError::Engine);
            }
            rx.changed()
                .await
                .map_err(|_| ProcessError::Engine("process pump task dropped".into()))?;
        }
    }

    pub async fn graceful_kill(&self) -> Result<(), ProcessError> {
        self.send_ctrl(Ctrl::GracefulKill).await?;
        self.wait().await?;
        Ok(())
    }

    pub async fn kill(&self) -> Result<(), ProcessError> {
        self.send_ctrl(Ctrl::Kill).await?;
        self.wait().await?;
        Ok(())
    }

    /// Writes and flushes bytes to the child's stdin pipe on a dedicated task,
    /// so output draining never stalls. `Ok(())` means the bytes were written
    /// and flushed successfully. The queue is bounded at 64 in-flight writes;
    /// if the child stops reading, additional writes fail fast with
    /// [`ProcessError::StdinUnavailable`].
    pub async fn write_stdin(&self, data: &[u8]) -> Result<(), ProcessError> {
        let data = data.to_vec();
        self.send_ctrl(move |reply| Ctrl::WriteStdin(data, reply))
            .await
    }

    pub(crate) async fn send_ctrl(
        &self,
        make: impl FnOnce(oneshot::Sender<Result<(), ProcessError>>) -> Ctrl,
    ) -> Result<(), ProcessError> {
        let (tx, rx) = oneshot::channel();
        let ctrl = make(tx);
        let idempotent_kill = matches!(&ctrl, Ctrl::GracefulKill(_) | Ctrl::Kill(_));
        if self.ctrl.send(ctrl).await.is_err() {
            return if idempotent_kill && self.terminated.borrow().is_some() {
                Ok(())
            } else if idempotent_kill {
                Err(ProcessError::AlreadyExited)
            } else {
                Err(ProcessError::StdinUnavailable)
            };
        }
        match rx.await {
            Ok(result) => result,
            Err(_) if idempotent_kill && self.terminated.borrow().is_some() => Ok(()),
            Err(_) if idempotent_kill => Err(ProcessError::AlreadyExited),
            Err(_) => Err(ProcessError::StdinUnavailable),
        }
    }
}
