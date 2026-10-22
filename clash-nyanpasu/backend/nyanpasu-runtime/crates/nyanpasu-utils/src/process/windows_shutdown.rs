//! Console operations belong in a short-lived helper, never in the GUI/service host.

use std::{io, path::PathBuf};

use windows::Win32::System::{
    Console::{
        AttachConsole, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler,
    },
    Threading::CREATE_NO_WINDOW,
};

const HELPER_ARG: &str = "--nyanpasu-process-shutdown";

/// Dispatches the private shutdown mode before the executable initializes its
/// console handlers or service runtime. `None` means normal application startup.
pub fn windows_shutdown_helper_entry() -> Option<io::Result<()>> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(HELPER_ARG)) {
        return None;
    }
    let pid = args
        .next()
        .and_then(|arg| arg.to_str()?.parse().ok())
        .filter(|_| args.next().is_none());
    Some(match pid {
        Some(pid) => send_ctrl_c(pid),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid shutdown target",
        )),
    })
}

fn send_ctrl_c(pid: u32) -> io::Result<()> {
    // SAFETY: this entry runs only in a disposable helper. No host console or
    // signal-handler state is changed; all arguments are plain Win32 values.
    unsafe {
        let _ = FreeConsole();
        AttachConsole(pid)?;
        let result = SetConsoleCtrlHandler(None, true)
            .and_then(|()| GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0));
        let _ = FreeConsole();
        result?;
    }
    Ok(())
}

/// Requests Ctrl+C for one child. The engine drops it once the child is
/// reaped: until then the engine's process handle keeps the PID from reuse.
pub(super) struct Shutdown {
    pub(super) helper: PathBuf,
    pub(super) pid: u32,
    pub(super) requested: bool,
}

impl Shutdown {
    /// Returns at most one request per process; repeated stops must not resend
    /// Ctrl+C (some cores interpret a second Ctrl+C as an immediate exit).
    pub(super) fn request(&mut self) -> Option<tokio::process::Command> {
        if std::mem::replace(&mut self.requested, true) {
            return None;
        }
        let mut command = tokio::process::Command::new(&self.helper);
        command
            .args([HELPER_ARG, &self.pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW.0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        Some(command)
    }
}
