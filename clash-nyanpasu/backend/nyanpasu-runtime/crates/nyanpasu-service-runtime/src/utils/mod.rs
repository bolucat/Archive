use anyhow::Context;
use service_manager::ServiceManager;
use tracing_panic::panic_hook;

#[cfg(windows)]
pub mod acl;
pub mod dirs;
pub mod os;
pub mod service;

pub fn must_check_elevation() -> bool {
    #[cfg(windows)]
    {
        use check_elevation::is_elevated;
        is_elevated().unwrap()
    }
    #[cfg(not(windows))]
    {
        use whoami::username;
        username().is_ok_and(|username| username == "root")
    }
}

pub fn get_service_manager() -> Result<Box<dyn ServiceManager>, anyhow::Error> {
    let manager = <dyn ServiceManager>::native()?;
    if !manager.available().context(
        "service manager is not available, please make sure you are running as root or administrator",
    )? {
        anyhow::bail!("service manager not available");
    }
    Ok(manager)
}

pub fn deadlock_detection() {
    #[cfg(feature = "deadlock_detection")]
    {
        // only for #[cfg]
        use parking_lot::deadlock;
        use std::{thread, time::Duration};

        // Create a background thread which checks for deadlocks every 10s
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(10));
                let deadlocks = deadlock::check_deadlock();
                if deadlocks.is_empty() {
                    continue;
                }

                eprintln!("{} deadlocks detected", deadlocks.len());
                tracing::error!("{} deadlocks detected", deadlocks.len());
                for (i, threads) in deadlocks.iter().enumerate() {
                    eprintln!("Deadlock #{}", i);
                    tracing::error!("Deadlock #{}", i);
                    for t in threads {
                        eprintln!("Thread Id {:#?}", t.thread_id());
                        eprintln!("{:#?}", t.backtrace());
                        tracing::error!("Thread Id {:#?}", t.thread_id());
                        tracing::error!("{:#?}", t.backtrace());
                    }
                }
            }
        });
    } // only for #[cfg]
}

/// Register a panic hook to log the panic message and location, then exit the process.
pub fn register_panic_hook() {
    std::panic::set_hook(Box::new(panic_hook));
}
