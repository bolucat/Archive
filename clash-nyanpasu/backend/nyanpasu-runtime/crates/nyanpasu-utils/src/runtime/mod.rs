//! Access to a shared Tokio runtime.

use std::{future::Future, sync::OnceLock, thread};

use tokio::runtime::Builder;
use tokio::runtime::Handle;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
pub static RUNTIME: OnceLock<Runtime> = OnceLock::new();

/// The most worker threads the default runtime starts, however many CPUs the
/// machine has.
const MAX_WORKER_THREADS: usize = 4;

/// Builds a multi-thread runtime with one worker per available CPU, at most
/// `MAX_WORKER_THREADS`. The explicit count takes precedence over the
/// `TOKIO_WORKER_THREADS` environment variable.
pub fn default_runtime() -> Runtime {
    let workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(MAX_WORKER_THREADS);
    Builder::new_multi_thread()
        .worker_threads(workers)
        .enable_all()
        .build()
        .unwrap()
}

pub fn get_runtime_handle() -> Handle {
    match Handle::try_current() {
        Ok(handle) => handle,
        Err(_) => {
            let runtime = RUNTIME.get_or_init(default_runtime);
            runtime.handle().clone()
        }
    }
}

/// Runs a future to completion on runtime.
pub fn block_on<F: Future>(task: F) -> F::Output {
    let handle = get_runtime_handle();
    handle.block_on(task)
}

pub fn spawn<F>(task: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    let handle = get_runtime_handle();
    handle.spawn(task)
}

/// spawn a task in current thread, it not require the task to be Send.
pub async fn spawn_current_thread<F: Future + 'static>(task: F) -> JoinHandle<F::Output> {
    let local = tokio::task::LocalSet::new();
    local.spawn_local(task)
}

/// block run a async task in current thread, it not require the task to be Send.
pub fn block_on_current_thread<F: Future>(task: F) -> F::Output {
    let handle = tokio::runtime::Handle::current();
    handle.block_on(async move {
        let local = tokio::task::LocalSet::new();
        local.run_until(task).await
    })
}

/// run a async task in current thread, it not require the task to be Send.
pub async fn run_until<F: Future>(task: F) -> F::Output {
    let local = tokio::task::LocalSet::new();
    local.run_until(task).await
}

/// check if the current thread is a tokio context
pub fn is_tokio_context() -> bool {
    tokio::runtime::Handle::try_current().is_ok()
}

/// block on a future, if the current thread is a tokio context, it will mark this thread as a blocking work thread, then block on the future.
/// otherwise, it will block on the current thread directly.
pub fn block_on_anywhere<F: Future>(task: F) -> F::Output {
    if is_tokio_context() {
        tokio::task::block_in_place(|| block_on(task))
    } else {
        block_on(task)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_runtime_caps_its_workers() {
        let workers = default_runtime().metrics().num_workers();
        assert!((1..=MAX_WORKER_THREADS).contains(&workers));
    }
}
