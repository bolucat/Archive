//! Graceful child-process termination.

pub trait ChildExt {
    /// In windows, it should send CTRL_EVENT_CLOSE to the child process, and wait 5 seconds for it to exit. If not, it will call TerminateProcess to kill it.
    /// In unix, it should send SIGTERM to the child process, and wait 5 seconds for it to exit. If not, it will send SIGKILL to kill it.
    fn gracefully_kill(&self) -> std::io::Result<()>;
}

#[cfg(windows)]
fn gracefully_kill(pid: u32) -> std::io::Result<()> {
    use windows::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
    unsafe {
        GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pid).map_err(|e| {
            std::io::Error::other(format!("GenerateConsoleCtrlEvent failed: {e:?}"))
        })?;
    }
    Ok(())
}

#[cfg(unix)]
fn gracefully_kill(pid: u32) -> std::io::Result<()> {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("kill failed: {:?}", e))
    })?;
    Ok(())
}

macro_rules! impl_child_ext {
    ($($t:ty),*) => {
        $(impl ChildExt for $t {
            fn gracefully_kill(&self) -> std::io::Result<()> {
                if matches!(self.try_wait(), Ok(Some(_))) {
                    return Ok(());
                }
                let pid = self.id();
                gracefully_kill(pid)?;
                for _ in 0..30 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if let Ok(Some(_)) = self.try_wait() {
                        return Ok(());
                    }
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to gracefully kill child process {}", pid),
                ))
            }
        })*
    }
}

// impl_child_ext!(std::process::Child);

impl_child_ext!(shared_child::SharedChild);
