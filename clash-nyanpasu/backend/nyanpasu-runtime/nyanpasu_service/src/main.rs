#[cfg(windows)]
mod win_service;

use std::future::Future;

use tokio::sync::mpsc::Receiver;

use nyanpasu_service::{
    consts::ExitCode,
    handler,
    utils::{os::register_ctrlc_handler, register_panic_hook},
};
use nyanpasu_utils::runtime::block_on;

fn main() -> ExitCode {
    let mut rx = register_ctrlc_handler();
    register_panic_hook();
    #[cfg(windows)]
    {
        let args = std::env::args_os().any(|arg| &arg == "--service");
        if args {
            crate::win_service::run().unwrap();
            return ExitCode::Normal;
        }
    }

    block_on(run_until_signal(&mut rx, handler(), || {
        nyanpasu_service::server_shutdown_token()
            .map(|token| token.cancel())
            .is_some()
    }))
}

/// Runs `command` until it ends or a signal arrives. On a signal a running
/// server is asked to stop and still awaited, so it stops its core and drains
/// its clients before the process exits; a second signal ends that wait, and
/// any other command ends at once. `stop_server` cancels the server, and
/// reports whether there was one.
async fn run_until_signal(
    signals: &mut Receiver<()>,
    command: impl Future<Output = ExitCode>,
    stop_server: impl FnOnce() -> bool,
) -> ExitCode {
    tokio::pin!(command);
    tokio::select! {
        biased;
        Some(()) = signals.recv() => {}
        exit_code = &mut command => return exit_code,
    }
    if !stop_server() {
        return ExitCode::Normal;
    }
    tokio::select! {
        biased;
        exit_code = &mut command => exit_code,
        Some(()) = signals.recv() => {
            eprintln!("Stopping without waiting for the service to finish");
            ExitCode::Normal
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use tokio::sync::{Notify, mpsc};

    use super::*;

    /// A server that, once its token is cancelled, stops its core after
    /// `release` is notified.
    async fn server(
        cancelled: Arc<Notify>,
        release: Arc<Notify>,
        core_stopped: Arc<AtomicBool>,
    ) -> ExitCode {
        cancelled.notified().await;
        release.notified().await;
        core_stopped.store(true, Ordering::SeqCst);
        ExitCode::Other
    }

    fn signals() -> (mpsc::Sender<()>, mpsc::Receiver<()>) {
        mpsc::channel(1)
    }

    #[tokio::test]
    async fn a_signal_waits_for_the_server_to_stop_its_core() {
        let (cancelled, release) = (Arc::new(Notify::new()), Arc::new(Notify::new()));
        let core_stopped = Arc::new(AtomicBool::new(false));
        let (signal, mut rx) = signals();
        let stop = cancelled.clone();
        let running = tokio::spawn({
            let (release, core_stopped) = (release.clone(), core_stopped.clone());
            async move {
                run_until_signal(
                    &mut rx,
                    server(cancelled, release, core_stopped),
                    move || {
                        stop.notify_one();
                        true
                    },
                )
                .await
            }
        });

        signal.send(()).await.unwrap();
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert!(!running.is_finished(), "still waiting for the server");

        release.notify_one();
        let exit_code = running.await.unwrap();
        assert!(core_stopped.load(Ordering::SeqCst));
        assert!(
            matches!(exit_code, ExitCode::Other),
            "the server's own exit code"
        );
    }

    #[tokio::test]
    async fn a_second_signal_ends_the_wait_for_a_stuck_server() {
        let (signal, mut rx) = signals();
        let running = tokio::spawn(async move {
            run_until_signal(&mut rx, std::future::pending(), || true).await
        });

        signal.send(()).await.unwrap();
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert!(!running.is_finished(), "the first signal waits");

        signal.send(()).await.unwrap();
        assert!(matches!(running.await.unwrap(), ExitCode::Normal));
    }

    #[tokio::test]
    async fn a_signal_ends_any_other_command_at_once() {
        let (signal, mut rx) = signals();
        signal.send(()).await.unwrap();

        let exit_code = run_until_signal(&mut rx, std::future::pending(), || false).await;

        assert!(matches!(exit_code, ExitCode::Normal));
    }

    #[tokio::test]
    async fn a_command_that_ends_first_keeps_its_exit_code() {
        let (_signal, mut rx) = signals();

        let exit_code = run_until_signal(&mut rx, async { ExitCode::PermissionDenied }, || {
            unreachable!("no signal arrived")
        })
        .await;

        assert!(matches!(exit_code, ExitCode::PermissionDenied));
    }
}
