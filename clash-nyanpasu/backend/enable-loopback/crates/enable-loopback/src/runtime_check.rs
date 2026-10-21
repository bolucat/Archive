//! Check the framework runtime before reactor bootstrap. When it is missing, offer to
//! install it through the Microsoft Store, or the official download (§8.2).

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
};

use nyanpasu_windows_dependencies::{
    FrameworkDependency, Version,
    store::{InstallState, StoreInstaller},
};
use snafu::Snafu;
use windows::{
    Win32::{
        HWND, LPARAM, WPARAM,
        commctrl::*,
        shellapi::ShellExecuteW,
        winnt::{PROCESSOR_ARCHITECTURE_AMD64, PROCESSOR_ARCHITECTURE_ARM64},
        winuser::*,
    },
    core::{HRESULT, HSTRING, PCWSTR, w},
};

pub const DOWNLOAD_URL: &str = "https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads";
const RUNTIME: FrameworkDependency = FrameworkDependency::WindowsAppRuntime2;
// windows-rs a29e064, crates/libs/reactor/src/native/bindings.rs:32212–32216,
// WINDOWSAPPSDK_RUNTIME_VERSION_{MAJOR,MINOR,BUILD,REVISION,UINT64}.
const REQUIRED_VERSION: Version = Version::new(2, 5, 1, 0);
/// Names this tool in the Microsoft Store's install log.
const STORE_CLIENT_ID: &str = "enable-loopback";
#[cfg(target_arch = "x86_64")]
const PROCESS_ARCHITECTURE: u32 = PROCESSOR_ARCHITECTURE_AMD64 as u32;
#[cfg(target_arch = "aarch64")]
const PROCESS_ARCHITECTURE: u32 = PROCESSOR_ARCHITECTURE_ARM64 as u32;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("enable-loopback supports Windows x64 and arm64 only");

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("ShellExecuteW failed for {url} with status {status}"))]
    OpenDownload { url: String, status: isize },
    #[snafu(display("Displaying the runtime dialog failed: {source}"))]
    ShowDialog { source: windows::core::Error },
}

fn satisfied(installed: Option<Version>) -> bool {
    installed.is_some_and(|version| version >= REQUIRED_VERSION)
}

/// Build the official installer URL for a supported process architecture.
fn installer_url(architecture: u32) -> String {
    let architecture = match architecture as i32 {
        PROCESSOR_ARCHITECTURE_AMD64 => "x64",
        PROCESSOR_ARCHITECTURE_ARM64 => "arm64",
        _ => unreachable!("only x64 and arm64 processes are supported"),
    };
    let Version {
        major,
        minor,
        build,
        ..
    } = REQUIRED_VERSION;
    format!(
        "https://aka.ms/windowsappsdk/{major}.{minor}/{major}.{minor}.{build}/windowsappruntimeinstall-{architecture}.exe"
    )
}

/// Returns false when the runtime is still missing after the prompt. An
/// inconclusive check lets reactor bootstrap.
pub fn ensure_available() -> Result<bool, Error> {
    match RUNTIME.diagnose() {
        Ok(installed) if satisfied(installed) => return Ok(true),
        Ok(_) => {}
        Err(error) => {
            crate::debug(&describe(&error));
            return Ok(true);
        }
    }
    let mut store = store_available();
    loop {
        let url = match prompt(store)? {
            Choice::Store => match install_from_store()? {
                StoreOutcome::Installed => return Ok(true),
                StoreOutcome::StoppedWaiting => return Ok(false),
                StoreOutcome::Failed(detail) => {
                    crate::show_message(
                        &rust_i18n::t!("runtime.store_failed", detail = detail),
                        MB_ICONERROR as u32,
                    );
                    store = false;
                    continue;
                }
            },
            Choice::DownloadInstaller => installer_url(PROCESS_ARCHITECTURE),
            Choice::OpenDownloadPage => DOWNLOAD_URL.to_owned(),
            Choice::Cancel => return Ok(false),
        };
        open(url)?;
        return Ok(false);
    }
}

/// Windows without the Store app, such as LTSC, cannot install from the Store.
fn store_available() -> bool {
    match StoreInstaller::new(STORE_CLIENT_ID).and_then(|store| store.is_available()) {
        Ok(available) => available,
        Err(error) => {
            crate::debug(&describe(&error));
            false
        }
    }
}

enum Choice {
    Store,
    DownloadInstaller,
    OpenDownloadPage,
    Cancel,
}

fn prompt(store: bool) -> Result<Choice, Error> {
    const INSTALL_FROM_STORE: i32 = 100;
    const DOWNLOAD_INSTALLER: i32 = 101;
    const OPEN_DOWNLOAD_PAGE: i32 = 102;
    let text = HSTRING::from(
        rust_i18n::t!(
            "runtime.missing",
            version = REQUIRED_VERSION.to_string(),
            architecture = std::env::consts::ARCH
        )
        .as_ref(),
    );
    let title = HSTRING::from(rust_i18n::t!("runtime.title").as_ref());
    let from_store = HSTRING::from(rust_i18n::t!("runtime.install_from_store").as_ref());
    let installer = HSTRING::from(rust_i18n::t!("runtime.download_installer").as_ref());
    let page = HSTRING::from(rust_i18n::t!("runtime.open_download_page").as_ref());
    let cancel = HSTRING::from(rust_i18n::t!("runtime.cancel").as_ref());
    let button = |id, text: &HSTRING| TASKDIALOG_BUTTON {
        nButtonID: id,
        pszButtonText: PCWSTR(text.as_ptr()),
    };
    let mut buttons = Vec::with_capacity(4);
    if store {
        buttons.push(button(INSTALL_FROM_STORE, &from_store));
    }
    buttons.extend([
        button(DOWNLOAD_INSTALLER, &installer),
        button(OPEN_DOWNLOAD_PAGE, &page),
        button(IDCANCEL, &cancel),
    ]);
    let config = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION | TDF_SIZE_TO_CONTENT,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszContent: PCWSTR(text.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        nDefaultButton: if store { INSTALL_FROM_STORE } else { IDCANCEL },
        ..Default::default()
    };
    let mut answer = IDCANCEL;
    // SAFETY: config, button array and all backing strings live through the modal call.
    unsafe { TaskDialogIndirect(&config, Some(&mut answer), None, None) }
        .ok()
        .map_err(|source| Error::ShowDialog { source })?;
    Ok(match answer {
        INSTALL_FROM_STORE => Choice::Store,
        DOWNLOAD_INSTALLER => Choice::DownloadInstaller,
        OPEN_DOWNLOAD_PAGE => Choice::OpenDownloadPage,
        _ => Choice::Cancel,
    })
}

enum StoreOutcome {
    Installed,
    /// The user closed the progress dialog; the Store keeps installing.
    StoppedWaiting,
    Failed(String),
}

/// Shared between the installing thread and the progress dialog's timer.
#[derive(Default)]
struct StoreProgress {
    percent: AtomicU32,
    /// The bar starts as a marquee until the Store reports a percentage.
    determinate: AtomicBool,
    outcome: Mutex<Option<Result<(), String>>>,
}

/// Installs the runtime on a worker thread while a progress dialog runs here.
fn install_from_store() -> Result<StoreOutcome, Error> {
    let progress = Arc::new(StoreProgress::default());
    let worker = Arc::clone(&progress);
    thread::spawn(move || {
        let product = RUNTIME
            .store_product_id()
            .expect("Windows App Runtime 2 has a Store product");
        let result = StoreInstaller::new(STORE_CLIENT_ID)
            .map_err(|error| describe(&error))
            .and_then(|store| {
                store
                    .install(product, |update| {
                        if matches!(
                            update.state,
                            Some(InstallState::Downloading | InstallState::Installing)
                        ) {
                            worker
                                .percent
                                .store(update.percent.round() as u32, Ordering::Relaxed);
                        }
                    })
                    .map_err(|error| describe(&error))
            });
        *worker.outcome.lock().expect("no holder panics") = Some(result);
    });

    let title = HSTRING::from(rust_i18n::t!("runtime.title").as_ref());
    let text = HSTRING::from(rust_i18n::t!("runtime.installing").as_ref());
    let close = HSTRING::from(rust_i18n::t!("runtime.close").as_ref());
    let buttons = [TASKDIALOG_BUTTON {
        nButtonID: IDCANCEL,
        pszButtonText: PCWSTR(close.as_ptr()),
    }];
    let config = TASKDIALOGCONFIG {
        cbSize: size_of::<TASKDIALOGCONFIG>() as u32,
        dwFlags: TDF_ALLOW_DIALOG_CANCELLATION
            | TDF_SIZE_TO_CONTENT
            | TDF_SHOW_MARQUEE_PROGRESS_BAR
            | TDF_CALLBACK_TIMER,
        pszWindowTitle: PCWSTR(title.as_ptr()),
        pszContent: PCWSTR(text.as_ptr()),
        cButtons: buttons.len() as u32,
        pButtons: buttons.as_ptr(),
        pfCallback: Some(progress_callback),
        lpCallbackData: Arc::as_ptr(&progress) as isize,
        ..Default::default()
    };
    // SAFETY: config, buttons, strings and the progress the callback reads live
    // through the modal call.
    unsafe { TaskDialogIndirect(&config, None, None, None) }
        .ok()
        .map_err(|source| Error::ShowDialog { source })?;

    let outcome = progress.outcome.lock().expect("no holder panics").take();
    Ok(match outcome {
        None => StoreOutcome::StoppedWaiting,
        Some(Err(detail)) => StoreOutcome::Failed(detail),
        // The Store may carry an older runtime than this tool needs.
        Some(Ok(())) => match RUNTIME.diagnose() {
            Ok(installed) if satisfied(installed) => StoreOutcome::Installed,
            Ok(_) => StoreOutcome::Failed(
                rust_i18n::t!(
                    "runtime.store_incomplete",
                    version = REQUIRED_VERSION.to_string()
                )
                .into_owned(),
            ),
            Err(error) => StoreOutcome::Failed(describe(&error)),
        },
    })
}

/// Mirrors the worker's progress into the dialog and closes it once the worker is done.
unsafe extern "system" fn progress_callback(
    hwnd: HWND,
    msg: u32,
    _: WPARAM,
    _: LPARAM,
    data: isize,
) -> HRESULT {
    // SAFETY: data points to the StoreProgress that outlives the modal dialog.
    let progress = unsafe { &*(data as *const StoreProgress) };
    // SAFETY: hwnd is the live task dialog that sent this notification.
    let send = |message: i32, wparam: usize, lparam: isize| unsafe {
        SendMessageW(hwnd, message as u32, wparam, lparam);
    };
    match msg as i32 {
        TDN_CREATED => send(TDM_SET_PROGRESS_BAR_MARQUEE, 1, 0),
        TDN_TIMER => {
            if progress.outcome.lock().expect("no holder panics").is_some() {
                send(TDM_CLICK_BUTTON, IDCANCEL as usize, 0);
                return HRESULT(0);
            }
            let percent = progress.percent.load(Ordering::Relaxed);
            if percent > 0 {
                if !progress.determinate.swap(true, Ordering::Relaxed) {
                    send(TDM_SET_PROGRESS_BAR_MARQUEE, 0, 0);
                    send(TDM_SET_MARQUEE_PROGRESS_BAR, 0, 0);
                }
                send(TDM_SET_PROGRESS_BAR_POS, percent.min(100) as usize, 0);
            }
        }
        _ => {}
    }
    HRESULT(0)
}

fn open(url: String) -> Result<(), Error> {
    let target = HSTRING::from(&url);
    // SAFETY: URL is a derived official HTTPS address; no untrusted command or arguments.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            &target,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    let status = result as isize;
    if status <= 32 {
        return Err(Error::OpenDownload { url, status });
    }
    Ok(())
}

/// An error with the causes its display leaves out.
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_urls_match_required_version_and_both_architectures() {
        assert_eq!(
            installer_url(PROCESSOR_ARCHITECTURE_AMD64 as u32),
            "https://aka.ms/windowsappsdk/2.5/2.5.1/windowsappruntimeinstall-x64.exe"
        );
        assert_eq!(
            installer_url(PROCESSOR_ARCHITECTURE_ARM64 as u32),
            "https://aka.ms/windowsappsdk/2.5/2.5.1/windowsappruntimeinstall-arm64.exe"
        );
    }

    #[test]
    fn requires_at_least_the_reactor_runtime_version() {
        assert!(satisfied(Some(REQUIRED_VERSION)));
        assert!(satisfied(Some(Version::new(2, 6, 0, 0))));
        assert!(!satisfied(Some(Version::new(2, 5, 0, 9))));
        assert!(!satisfied(None));
    }
}
