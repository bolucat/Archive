//! Custom readiness and liveness probes.

use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use tokio_util::sync::CancellationToken;

use crate::{Error, ResolvedController};

/// The boxed future returned by an object-safe [`HealthProbe`].
pub type ProbeFuture<'a> = Pin<Box<dyn Future<Output = ProbeResult> + Send + 'a>>;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeResult {
    Healthy,
    Unhealthy { detail: Option<String> },
}

impl ProbeResult {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbePhase {
    Readiness,
    Liveness,
    Reconcile,
}

/// Context for one probe attempt.
///
/// This type intentionally does not implement `Debug`: a resolved controller
/// may carry an authentication secret.
#[derive(Clone)]
pub struct ProbeContext {
    pub epoch: u64,
    pub pid: u32,
    pub phase: ProbePhase,
    pub controller: Arc<ResolvedController>,
    pub cancel: CancellationToken,
}

/// One readiness, liveness, or reconciliation check.
///
/// Implementations must be cancellation-safe: dropping the returned future
/// must not leave a detached task behind. External-command probes must pass
/// arguments directly to [`tokio::process::Command`] rather than concatenate a
/// shell command, and must call `kill_on_drop(true)` so the child is killed
/// when the future is dropped.
pub trait HealthProbe: Send + Sync + 'static {
    fn check<'a>(&'a self, context: ProbeContext) -> ProbeFuture<'a>;
}

/// Cheaply cloneable, debug-safe handle to a custom probe.
#[derive(Clone)]
pub struct ProbeHandle {
    label: Arc<str>,
    inner: Arc<dyn HealthProbe>,
}

impl ProbeHandle {
    pub fn new(label: impl Into<Arc<str>>, probe: impl HealthProbe) -> Self {
        Self {
            label: label.into(),
            inner: Arc::new(probe),
        }
    }

    pub fn from_fn<F, Fut>(label: impl Into<Arc<str>>, function: F) -> Self
    where
        F: Fn(ProbeContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ProbeResult> + Send + 'static,
    {
        Self::new(label, FnProbe(function))
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn check(&self, context: ProbeContext) -> ProbeFuture<'_> {
        self.inner.check(context)
    }
}

impl std::fmt::Debug for ProbeHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProbeHandle")
            .field("label", &self.label)
            .finish()
    }
}

struct FnProbe<F>(F);

impl<F, Fut> HealthProbe for FnProbe<F>
where
    F: Fn(ProbeContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ProbeResult> + Send + 'static,
{
    fn check<'a>(&'a self, context: ProbeContext) -> ProbeFuture<'a> {
        Box::pin((self.0)(context))
    }
}

/// Default readiness probe: healthy iff `GET /version` succeeds.
pub struct ControllerVersionProbe {
    client: clash_api::Client,
}

impl ControllerVersionProbe {
    pub fn new(controller: &ResolvedController) -> Result<Self, Error> {
        let mut builder = clash_api::Client::builder(controller.host.clone())
            .configure_reqwest(|builder| builder.timeout(Duration::from_secs(1)));
        if let Some(secret) = &controller.secret {
            builder = builder.secret(secret.as_str());
        }
        Ok(Self {
            client: builder.build()?,
        })
    }
}

impl HealthProbe for ControllerVersionProbe {
    fn check<'a>(&'a self, _context: ProbeContext) -> ProbeFuture<'a> {
        Box::pin(async move {
            match self.client.version().await {
                Ok(_) => ProbeResult::Healthy,
                Err(error) => ProbeResult::Unhealthy {
                    detail: Some(error.to_string()),
                },
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(port: u16) -> ResolvedController {
        ResolvedController {
            host: clash_api::Host::http(format!("127.0.0.1:{port}")).unwrap(),
            secret: None,
        }
    }

    fn context(controller: ResolvedController) -> ProbeContext {
        ProbeContext {
            epoch: 1,
            pid: 1,
            phase: ProbePhase::Readiness,
            controller: Arc::new(controller),
            cancel: CancellationToken::new(),
        }
    }

    struct SecretProbe;

    impl HealthProbe for SecretProbe {
        fn check<'a>(&'a self, _context: ProbeContext) -> ProbeFuture<'a> {
            Box::pin(async { ProbeResult::Healthy })
        }
    }

    #[test]
    fn handle_debug_prints_only_the_label() {
        let handle = ProbeHandle::new("safe-label", SecretProbe);
        let debug = format!("{handle:?}");
        assert_eq!(debug, "ProbeHandle { label: \"safe-label\" }");
        assert!(!debug.contains("SecretProbe"));
    }

    #[tokio::test]
    async fn controller_version_probe_matches_version_endpoint_health() {
        let closed_port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        let closed_controller = controller(closed_port);
        let probe = ControllerVersionProbe::new(&closed_controller).unwrap();
        assert!(!probe.check(context(closed_controller)).await.is_healthy());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0_u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = r#"{"meta":true,"version":"t"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        let controller = controller(port);
        let probe = ControllerVersionProbe::new(&controller).unwrap();
        assert!(probe.check(context(controller)).await.is_healthy());
    }
}
