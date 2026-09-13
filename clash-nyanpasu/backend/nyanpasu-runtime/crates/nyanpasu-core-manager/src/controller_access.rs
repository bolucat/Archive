//! Host authorization of core-created controller endpoints.
use crate::{HealthProbe, Host, ProbeContext, ProbeFuture, ProbeHandle, ProbeResult};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControllerAuthorization {
    /// The core receives the descriptor at creation; the host only verifies it.
    Core,
    /// The host sets permissions on the core-created endpoint before use.
    Host,
}

pub trait ControllerAccess: Send + Sync + 'static {
    fn supports_local_ipc(&self) -> bool;
    /// Optional creation policy, supplied by the host rather than global env.
    fn pipe_security_descriptor(&self) -> Option<&str> {
        None
    }
    /// Called for readiness and liveness, including after process respawn.
    fn authorize(
        &self,
        host: &Host,
        pid: u32,
        mode: ControllerAuthorization,
    ) -> std::io::Result<()>;
}

pub(crate) struct AuthorizedProbe {
    pub access: Arc<dyn ControllerAccess>,
    pub inner: ProbeHandle,
    pub mode: ControllerAuthorization,
}
impl HealthProbe for AuthorizedProbe {
    fn check<'a>(&'a self, context: ProbeContext) -> ProbeFuture<'a> {
        Box::pin(async move {
            if let Err(error) =
                self.access
                    .authorize(&context.controller.host, context.pid, self.mode)
            {
                return ProbeResult::Unhealthy {
                    detail: Some(format!("controller authorization failed: {error}")),
                };
            }
            self.inner.check(context).await
        })
    }
}
