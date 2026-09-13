//! The desktop execution mechanism: supervised child processes with PID-file
//! death confirmation, fronted by the [`RuntimeBackend`] boundary.

use std::time::Duration;

use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::{
    epoch::Epoch,
    error::Error,
    instance::Instance,
    probe::{ProbeHandle, ProbePhase, ProbeResult},
    spec::{InstanceSpec, ResolvedController},
    state::InstanceStatus,
};

use super::{BoxFuture, RuntimeBackend, RuntimeInstance, RuntimeLaunchRequest};

/// Probe wiring for instances the default backend launches. Collected by the
/// manager builder; a custom backend owns its own probing instead.
#[derive(Clone, Default)]
pub(crate) struct ProbePlan {
    pub(crate) controller_access: Option<std::sync::Arc<dyn crate::ControllerAccess>>,
    pub(crate) readiness: Option<ProbeHandle>,
    pub(crate) liveness: Option<ProbeHandle>,
    pub(crate) liveness_with_readiness: bool,
}

pub(crate) struct ProcessRuntimeBackend {
    probes: ProbePlan,
    cancel_token: CancellationToken,
}

impl ProcessRuntimeBackend {
    pub(crate) fn new(probes: ProbePlan, cancel_token: CancellationToken) -> Self {
        Self {
            probes,
            cancel_token,
        }
    }
}

impl RuntimeBackend for ProcessRuntimeBackend {
    fn launch(
        &self,
        request: RuntimeLaunchRequest,
    ) -> BoxFuture<'_, Result<Box<dyn RuntimeInstance>, Error>> {
        Box::pin(async move {
            let RuntimeLaunchRequest {
                effective_spec,
                capabilities,
                epoch,
                controller,
                log_tx,
            } = request;
            let descriptor = if matches!(controller.host, crate::Host::NamedPipe(_))
                && capabilities.contains(crate::Feature::NamedPipeSecurityDescriptor)
            {
                self.probes
                    .controller_access
                    .as_ref()
                    .and_then(|access| access.pipe_security_descriptor())
                    .map(str::to_owned)
            } else {
                None
            };
            let mode = if descriptor.is_some() {
                crate::ControllerAuthorization::Core
            } else {
                crate::ControllerAuthorization::Host
            };
            let mut builder = Instance::builder(
                effective_spec,
                epoch,
                controller.clone(),
                self.cancel_token.clone(),
            )
            .log_sender(log_tx)
            .pipe_security_descriptor(descriptor);
            if let Some(access) = &self.probes.controller_access {
                let readiness = match self.probes.readiness.clone() {
                    Some(probe) => probe,
                    None => ProbeHandle::new(
                        "controller-version",
                        crate::ControllerVersionProbe::new(&controller)?,
                    ),
                };
                let liveness = if self.probes.liveness_with_readiness {
                    readiness.clone()
                } else {
                    self.probes.liveness.clone().unwrap_or_else(|| {
                        ProbeHandle::from_fn("process-alive", |_| async { ProbeResult::Healthy })
                    })
                };
                let wrap = |inner| {
                    ProbeHandle::new(
                        "authorized-controller",
                        crate::controller_access::AuthorizedProbe {
                            access: access.clone(),
                            inner,
                            mode,
                        },
                    )
                };
                builder = builder
                    .readiness_probe(wrap(readiness))
                    .liveness_probe(wrap(liveness));
            } else {
                if let Some(probe) = self.probes.readiness.clone() {
                    builder = builder.readiness_probe(probe);
                }
                if let Some(probe) = self.probes.liveness.clone() {
                    builder = builder.liveness_probe(probe);
                }
                if self.probes.liveness_with_readiness {
                    builder = builder.liveness_with_readiness_probe();
                }
            }
            let instance = builder.spawn().await?;
            Ok(Box::new(instance) as Box<dyn RuntimeInstance>)
        })
    }

    fn check_config<'a>(&'a self, spec: &'a InstanceSpec) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(crate::kind::check_config(spec))
    }
}

impl RuntimeInstance for Instance {
    fn epoch(&self) -> Epoch {
        Instance::epoch(self)
    }

    fn spec(&self) -> &InstanceSpec {
        Instance::spec(self)
    }

    fn controller(&self) -> &ResolvedController {
        Instance::controller(self)
    }

    fn pid(&self) -> Option<u32> {
        Instance::pid(self)
    }

    fn state(&self) -> watch::Receiver<InstanceStatus> {
        Instance::state(self)
    }

    fn wait_ready<'a>(&'a self) -> BoxFuture<'a, Result<(), Error>> {
        Box::pin(Instance::wait_ready(self))
    }

    fn probe_now<'a>(&'a self, phase: ProbePhase) -> BoxFuture<'a, ProbeResult> {
        Box::pin(Instance::probe_now(self, phase))
    }

    fn stop_and_confirm_dead(
        self: Box<Self>,
        timeout: Duration,
    ) -> BoxFuture<'static, Result<(), Error>> {
        Box::pin(Instance::stop_and_confirm_dead(*self, timeout))
    }
}
