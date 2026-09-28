use crate::*;
use serde::{Serialize, de::DeserializeOwned};
use std::{future::Future, marker::PhantomData};

/// Keeps input/output types attached to a definition before type erasure at the
/// actor boundary. Clone `registration()` into a scope snapshot, then bind it.
pub struct TypedJob<I, O> {
    job: Job,
    types: PhantomData<fn(I) -> O>,
}
impl<I, O> TypedJob<I, O>
where
    I: Serialize + DeserializeOwned + Send + 'static,
    O: Serialize + DeserializeOwned + Send + 'static,
{
    pub fn new<F, Fut>(definition: JobDefinition, input: I, handler: F) -> Result<Self, Error>
    where
        F: Fn(JobContext, I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, JobError>> + Send + 'static,
    {
        Ok(Self {
            job: Job::new(definition, input, handler)?,
            types: PhantomData,
        })
    }
    pub fn registration(&self) -> Job {
        self.job.clone()
    }
    pub fn bind(&self, client: JobsClient) -> JobHandle<I, O> {
        JobHandle {
            key: self.job.definition.key.clone(),
            client,
            types: PhantomData,
        }
    }
}
pub struct JobHandle<I, O> {
    key: JobKey,
    client: JobsClient,
    types: PhantomData<fn(I) -> O>,
}
impl<I: Serialize, O: DeserializeOwned> JobHandle<I, O> {
    pub async fn run_with(&self, input: &I) -> Result<TypedRunHandle<O>, Error> {
        Ok(TypedRunHandle {
            run: self.client.run_with(&self.key, input).await?,
            output: PhantomData,
        })
    }
    pub async fn run_now(&self) -> Result<TypedRunHandle<O>, Error> {
        Ok(TypedRunHandle {
            run: self.client.run_now(&self.key).await?,
            output: PhantomData,
        })
    }
}
pub struct TypedRunHandle<O> {
    run: RunHandle,
    output: PhantomData<fn() -> O>,
}
impl<O: DeserializeOwned> TypedRunHandle<O> {
    pub fn id(&self) -> RunId {
        self.run.id
    }
    pub async fn completion(&self) -> Result<Completion, Error> {
        self.run.wait().await
    }
    pub async fn output(&self) -> Result<O, Error> {
        self.run.wait_output().await
    }
    pub async fn cancel(&self) -> Result<CancelResult, Error> {
        self.run.cancel().await
    }
}
