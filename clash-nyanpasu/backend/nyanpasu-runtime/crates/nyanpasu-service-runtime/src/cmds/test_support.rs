use std::{collections::VecDeque, io};

use parking_lot::Mutex;
use service_manager::{
    ServiceInstallCtx, ServiceLabel, ServiceLevel, ServiceManager, ServiceStartCtx, ServiceStatus,
    ServiceStatusCtx, ServiceStopCtx, ServiceUninstallCtx,
};

use crate::consts::SERVICE_LABEL;

#[derive(Debug, PartialEq)]
pub(crate) enum Call {
    Status(ServiceStatusCtx),
    Install(ServiceInstallCtx),
    Uninstall(ServiceUninstallCtx),
    Start(ServiceStartCtx),
    Stop(ServiceStopCtx),
}

pub(crate) struct MockServiceManager {
    statuses: Mutex<VecDeque<ServiceStatus>>,
    calls: Mutex<Vec<Call>>,
}

impl MockServiceManager {
    /// `statuses` are handed out in order, one per `status()` call.
    pub(crate) fn with_statuses(statuses: impl IntoIterator<Item = ServiceStatus>) -> Self {
        Self {
            statuses: Mutex::new(statuses.into_iter().collect()),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn calls(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.lock())
    }
}

impl ServiceManager for MockServiceManager {
    fn available(&self) -> io::Result<bool> {
        Ok(true)
    }

    fn install(&self, ctx: ServiceInstallCtx) -> io::Result<()> {
        self.calls.lock().push(Call::Install(ctx));
        Ok(())
    }

    fn uninstall(&self, ctx: ServiceUninstallCtx) -> io::Result<()> {
        self.calls.lock().push(Call::Uninstall(ctx));
        Ok(())
    }

    fn start(&self, ctx: ServiceStartCtx) -> io::Result<()> {
        self.calls.lock().push(Call::Start(ctx));
        Ok(())
    }

    fn stop(&self, ctx: ServiceStopCtx) -> io::Result<()> {
        self.calls.lock().push(Call::Stop(ctx));
        Ok(())
    }

    fn level(&self) -> ServiceLevel {
        ServiceLevel::System
    }

    fn set_level(&mut self, _level: ServiceLevel) -> io::Result<()> {
        Ok(())
    }

    fn status(&self, ctx: ServiceStatusCtx) -> io::Result<ServiceStatus> {
        self.calls.lock().push(Call::Status(ctx));
        Ok(self
            .statuses
            .lock()
            .pop_front()
            .expect("the engine asked for more statuses than the test scripted"))
    }
}

pub(crate) fn label() -> ServiceLabel {
    SERVICE_LABEL.parse().unwrap()
}

pub(crate) fn status_call() -> Call {
    Call::Status(ServiceStatusCtx { label: label() })
}
