use std::cell::Cell;
use std::ffi::c_void;
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use napi::bindgen_prelude::{BigInt, Env, Object};
use napi::{Error, JsDeferred, Result as NapiResult, Status};
use napi_derive::napi;
use windows::ApplicationModel::DataTransfer::{
    DataPackageOperation, DataRequest, DataRequestedEventArgs, DataTransferManager,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::{IStorageItem, StorageFile};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::System::WinRT::{
    RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RO_INIT_TYPE, RoInitialize, RoUninitialize,
};
use windows::Win32::UI::Shell::IDataTransferManagerInterop;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows::core::{HSTRING, Interface, Ref, factory};
use windows_collections::IIterable;

const SHARE_TIMEOUT: Duration = Duration::from_secs(5);

type ShareDeferred = JsDeferred<(), fn(Env) -> NapiResult<()>>;

struct Apartment;

impl Apartment {
    fn initialize(kind: RO_INIT_TYPE) -> windows::core::Result<Self> {
        unsafe {
            RoInitialize(kind)?;
        }
        Ok(Self)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}

struct ShareState {
    deferred: Mutex<Option<ShareDeferred>>,
    data_requested: AtomicBool,
    settled: AtomicBool,
}

fn deferred_access(state: &ShareState) -> MutexGuard<'_, Option<ShareDeferred>> {
    match state.deferred.lock() {
        Ok(access) => access,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn resolve_share(_: Env) -> NapiResult<()> {
    Ok(())
}

fn settle(state: &Arc<ShareState>, result: Result<(), String>) {
    if state.settled.swap(true, Ordering::AcqRel) {
        return;
    }
    let deferred = deferred_access(state).take();
    let Some(deferred) = deferred else {
        return;
    };
    match result {
        Ok(()) => deferred.resolve(resolve_share),
        Err(message) => deferred.reject(Error::new(Status::GenericFailure, message)),
    }
}

fn windows_error(operation: &str, error: windows::core::Error) -> String {
    format!("{operation}: {error}")
}

fn populate_request(request: &DataRequest, path: &str, title: &str) -> windows::core::Result<()> {
    let _apartment = Apartment::initialize(RO_INIT_MULTITHREADED)?;
    let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path))?.join()?;
    let item: IStorageItem = file.cast()?;
    let items: IIterable<IStorageItem> = vec![Some(item)].into();
    let data = request.Data()?;
    data.Properties()?.SetTitle(&HSTRING::from(title))?;
    data.SetStorageItemsReadOnly(&items)?;
    data.SetRequestedOperation(DataPackageOperation::Copy)
}

fn complete_request(
    request: DataRequest,
    deferral: windows::ApplicationModel::DataTransfer::DataRequestDeferral,
    path: String,
    title: String,
    state: Arc<ShareState>,
) {
    let populate_result = populate_request(&request, &path, &title)
        .map_err(|error| windows_error("prepare shared file", error));
    if let Err(message) = &populate_result {
        let _ = request.FailWithDisplayText(&HSTRING::from(message));
    }
    let result = match deferral.Complete() {
        Ok(()) => populate_result,
        Err(error) => Err(windows_error("complete Windows share request", error)),
    };
    settle(&state, result);
}

fn start_data_request(
    arguments: Ref<'_, DataRequestedEventArgs>,
    path: String,
    title: String,
    state: Arc<ShareState>,
) -> Result<(), String> {
    let arguments = arguments
        .ok()
        .map_err(|error| windows_error("read Windows share request", error))?;
    let request = arguments
        .Request()
        .map_err(|error| windows_error("read Windows share data request", error))?;
    let deferral = request
        .GetDeferral()
        .map_err(|error| windows_error("defer Windows share data request", error))?;
    let worker_request = request.clone();
    let worker_deferral = deferral.clone();
    let worker_state = Arc::clone(&state);
    let spawn_result = thread::Builder::new()
        .name("windows-share-data".to_owned())
        .spawn(move || complete_request(worker_request, worker_deferral, path, title, worker_state));
    if let Err(error) = spawn_result {
        let message = format!("start Windows share data worker: {error}");
        let _ = request.FailWithDisplayText(&HSTRING::from(&message));
        let _ = deferral.Complete();
        return Err(message);
    }
    Ok(())
}

fn window_handle(value: BigInt) -> NapiResult<HWND> {
    let (negative, handle, lossless) = value.get_u64();
    if negative || !lossless || handle == 0 {
        return Err(Error::new(
            Status::InvalidArg,
            "invalid sharing window handle".to_owned(),
        ));
    }
    Ok(HWND(handle as usize as *mut c_void))
}

fn validate_window(window: HWND) -> NapiResult<()> {
    let mut process_identifier = 0;
    let thread_identifier = unsafe {
        GetWindowThreadProcessId(window, Some(&mut process_identifier))
    };
    if thread_identifier == 0 || process_identifier != unsafe { GetCurrentProcessId() } {
        return Err(Error::new(
            Status::InvalidArg,
            "sharing window does not belong to Electron".to_owned(),
        ));
    }
    Ok(())
}

#[napi(js_name = "shareFile")]
pub fn share_file<'env>(
    env: &'env Env,
    window: BigInt,
    path: String,
    title: String,
) -> NapiResult<Object<'env>> {
    let window = window_handle(window)?;
    validate_window(window)?;
    let apartment = Apartment::initialize(RO_INIT_SINGLETHREADED)
        .map_err(|error| Error::from_reason(windows_error("initialize Windows Runtime", error)))?;
    let interop = factory::<DataTransferManager, IDataTransferManagerInterop>()
        .map_err(|error| Error::from_reason(windows_error("activate Windows sharing", error)))?;
    let manager: DataTransferManager = unsafe { interop.GetForWindow(window) }
        .map_err(|error| Error::from_reason(windows_error("get sharing manager for window", error)))?;
    let (mut deferred, promise) = env.create_deferred::<(), fn(Env) -> NapiResult<()>>()?;
    let registration_token = Rc::new(Cell::new(None));
    let cleanup_token = Rc::clone(&registration_token);
    let cleanup_manager = manager.clone();
    deferred.set_finalize_callback(Some(Box::new(move |_| {
        if let Some(token) = cleanup_token.take() {
            let _ = cleanup_manager.RemoveDataRequested(token);
        }
        unsafe {
            RoUninitialize();
        }
    })));
    mem::forget(apartment);

    let state = Arc::new(ShareState {
        deferred: Mutex::new(Some(deferred)),
        data_requested: AtomicBool::new(false),
        settled: AtomicBool::new(false),
    });
    let handler_state = Arc::clone(&state);
    let handler = TypedEventHandler::new(move |_, arguments| {
        if handler_state.data_requested.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let result = start_data_request(
            arguments,
            path.clone(),
            title.clone(),
            Arc::clone(&handler_state),
        );
        if let Err(message) = result {
            settle(&handler_state, Err(message));
        }
        Ok(())
    });
    let token = match manager.DataRequested(&handler) {
        Ok(token) => token,
        Err(error) => {
            settle(
                &state,
                Err(windows_error("register Windows share data handler", error)),
            );
            return Ok(promise);
        }
    };
    registration_token.set(Some(token));
    let show_result = unsafe { interop.ShowShareUIForWindow(window) };
    if let Err(error) = show_result {
        settle(
            &state,
            Err(windows_error("show Windows share UI", error)),
        );
        return Ok(promise);
    }
    let timeout_state = Arc::clone(&state);
    let timeout_result = thread::Builder::new()
        .name("windows-share-timeout".to_owned())
        .spawn(move || {
            thread::sleep(SHARE_TIMEOUT);
            settle(
                &timeout_state,
                Err("Windows share request timed out".to_owned()),
            );
        });
    if let Err(error) = timeout_result {
        settle(
            &state,
            Err(format!("start Windows share timeout: {error}")),
        );
    }
    Ok(promise)
}
