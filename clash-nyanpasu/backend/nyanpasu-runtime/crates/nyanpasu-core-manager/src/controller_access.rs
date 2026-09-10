//! Host authorization of core-created controller endpoints.
use crate::{HealthProbe, Host, ProbeContext, ProbeFuture, ProbeHandle, ProbeResult};
use std::sync::Arc;

pub trait ControllerAccess: Send + Sync + 'static {
    fn supports_local_ipc(&self) -> bool;
    /// Called before every readiness attempt, including after process respawn.
    fn authorize(&self, host: &Host) -> std::io::Result<()>;
}

pub(crate) struct AuthorizedProbe {
    pub access: Arc<dyn ControllerAccess>,
    pub inner: ProbeHandle,
}
impl HealthProbe for AuthorizedProbe {
    fn check<'a>(&'a self, context: ProbeContext) -> ProbeFuture<'a> {
        Box::pin(async move {
            if let Err(error) = self.access.authorize(&context.controller.host) {
                return ProbeResult::Unhealthy {
                    detail: Some(format!("controller authorization failed: {error}")),
                };
            }
            self.inner.check(context).await
        })
    }
}
