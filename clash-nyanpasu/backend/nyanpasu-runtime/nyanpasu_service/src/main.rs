#[cfg(windows)]
mod win_service;

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

    block_on(async {
        tokio::select! {
            biased;
            Some(_) = rx.recv() => {
                ExitCode::Normal
            }
            exit_code = handler() => {
                exit_code
            }
        }
    })
}
