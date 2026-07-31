#![allow(dead_code)]
use std::{ffi::OsString, io::Result, time::Duration};

use nyanpasu_utils::runtime::block_on;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
    service_dispatcher,
};

use nyanpasu_service::consts::SERVICE_LABEL;

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

pub fn run() -> Result<()> {
    service_dispatcher::start(SERVICE_LABEL, ffi_service_main).map_err(std::io::Error::other)
}

define_windows_service!(ffi_service_main, service_main);

pub fn service_main(args: Vec<OsString>) {
    if let Err(e) = run_service(args) {
        panic!("Error starting service: {e:?}");
    }
}

struct ServiceHandleGuard(ServiceStatusHandle);
impl Drop for ServiceHandleGuard {
    fn drop(&mut self) {
        let _ = self.0.set_service_status(ServiceStatus {
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
            service_type: SERVICE_TYPE,
        });
    }
}

const STOP_PENDING_TICK: Duration = Duration::from_secs(2);

/// Upper bound for legitimate shutdown work (in-flight start 30s, core stop 10s,
/// pipe drain 5s, with margin). Past this, checkpoint reporting stops so the
/// SCM can detect a hung service instead of seeing fake progress forever.
const STOP_PENDING_DEADLINE: Duration = Duration::from_secs(90);

fn report_stop_pending(
    status_handle: &ServiceStatusHandle,
    checkpoint: u32,
) -> windows_service::Result<()> {
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StopPending,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint,
        wait_hint: Duration::from_secs(10),
        process_id: Some(std::process::id()),
        controls_accepted: ServiceControlAccept::empty(),
    })
}

pub fn run_service(_arguments: Vec<OsString>) -> windows_service::Result<()> {
    let shutdown_token = tokio_util::sync::CancellationToken::new();
    let shutdown_token_clone = shutdown_token.clone();
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                tracing::info!("Received stop event. shutting down...");
                shutdown_token_clone.cancel();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Preshutdown => {
                tracing::info!("Received shutdown event. shutting down...");
                shutdown_token_clone.cancel();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    // Register system service event handler
    let status_handle = service_control_handler::register(SERVICE_LABEL, event_handler)?;

    let pid = std::process::id();
    let next_status = ServiceStatus {
        // Should match the one from system service registry
        service_type: SERVICE_TYPE,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::PRESHUTDOWN,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
        process_id: Some(pid),
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let guard = ServiceHandleGuard(status_handle);
    let handle = std::thread::spawn(move || {
        block_on(nyanpasu_service::handler());
    });

    // Wait for shutdown signal
    block_on(shutdown_token.cancelled());

    let mut checkpoint = 1;
    report_stop_pending(&status_handle, checkpoint)?;

    // cancel the server handle
    if let Some(token) = nyanpasu_service::server_shutdown_token() {
        tracing::info!("Cancelling server shutdown token");
        token.cancel();
    }
    // The shutdown chain (core stop + pipe drain) is internally bounded but can
    // exceed a single wait hint; keep the SCM informed while real progress is
    // still plausible, then go quiet so a genuine hang becomes detectable.
    let stop_started = std::time::Instant::now();
    while !handle.is_finished() {
        if stop_started.elapsed() >= STOP_PENDING_DEADLINE {
            tracing::error!(
                "shutdown exceeded {STOP_PENDING_DEADLINE:?}; ceasing SCM progress reports"
            );
            break;
        }
        std::thread::sleep(STOP_PENDING_TICK);
        checkpoint += 1;
        if let Err(error) = report_stop_pending(&status_handle, checkpoint) {
            tracing::warn!("failed to refresh STOP_PENDING checkpoint: {error}");
        }
    }
    handle.join().unwrap();

    tracing::info!("Service stopped.");

    // drop the guard to set the service status to stopped
    drop(guard);
    Ok(())
}
