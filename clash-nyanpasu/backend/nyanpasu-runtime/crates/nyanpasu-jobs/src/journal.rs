use crate::{Error, storage::JobStore};
use std::time::Duration;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

type StoreOperation = Box<dyn FnOnce(&mut dyn JobStore) + Send>;
enum Work {
    Call(StoreOperation),
    Stop,
}
#[derive(Clone)]
pub(crate) struct Journal {
    sender: mpsc::Sender<Work>,
}
pub(crate) struct JournalWorker {
    pub client: Journal,
    pub task: JoinHandle<()>,
}
impl JournalWorker {
    pub fn start(mut store: Box<dyn JobStore>) -> Self {
        let (sender, mut receiver) = mpsc::channel::<Work>(128);
        // Narrow infrastructure worker: one owned blocking thread, bounded queue,
        // and explicit join. No actor-owned state is shared with the adapter.
        let task = tokio::task::spawn_blocking(move || {
            while let Some(work) = receiver.blocking_recv() {
                match work {
                    Work::Call(work) => work(store.as_mut()),
                    Work::Stop => break,
                }
            }
        });
        Self {
            client: Journal { sender },
            task,
        }
    }
}
impl Journal {
    pub async fn close(&self) {
        let _ = self.sender.send(Work::Stop).await;
    }
    pub async fn call<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut dyn JobStore) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let (send, recv) = oneshot::channel();
        self.sender
            .send(Work::Call(Box::new(move |store| {
                let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(store)))
                    .unwrap_or_else(|_| Err(Error::Journal("store_panicked".into())));
                let _ = send.send(value);
            })))
            .await
            .map_err(|_| Error::Stopped)?;
        recv.await.map_err(|_| Error::Stopped)?
    }
    pub async fn query<T: Send + 'static>(
        &self,
        timeout: Duration,
        f: impl FnOnce(&mut dyn JobStore) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        tokio::time::timeout(timeout, self.call(f))
            .await
            .map_err(|_| Error::TimedOut)?
    }
}
