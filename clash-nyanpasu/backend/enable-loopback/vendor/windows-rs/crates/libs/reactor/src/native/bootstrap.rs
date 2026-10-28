use super::*;
use bindings::*;
use windows_core::*;

const FRAMEWORK_FAMILY: PCWSTR = w!("Microsoft.WindowsAppRuntime.2_8wekyb3d8bbwe");
const BOOTSTRAP_DLL: &str = "Microsoft.WindowsAppRuntime.Bootstrap.dll";
const MDD_BOOTSTRAP_INITIALIZE_OPTIONS_NONE: i32 = 0;

static BOOTSTRAPPED: Mutex<bool> = Mutex::new(false);

type TryCreatePackageDependencyFn = unsafe extern "system" fn(
    PSID,
    PCWSTR,
    PACKAGE_VERSION,
    PackageDependencyProcessorArchitectures,
    PackageDependencyLifetimeKind,
    PCWSTR,
    CreatePackageDependencyOptions,
    *mut PWSTR,
) -> HRESULT;
type AddPackageDependencyFn = unsafe extern "system" fn(
    PCWSTR,
    i32,
    AddPackageDependencyOptions,
    *mut PACKAGEDEPENDENCY_CONTEXT,
    *mut PWSTR,
) -> HRESULT;
type MddBootstrapInitialize2Fn =
    unsafe extern "system" fn(u32, PCWSTR, PACKAGE_VERSION, i32) -> HRESULT;

pub(super) fn bootstrap() -> Result<()> {
    let mut bootstrapped = BOOTSTRAPPED.lock().unwrap();
    if *bootstrapped {
        return Ok(());
    }
    unsafe { bootstrap_inner()? };
    *bootstrapped = true;
    Ok(())
}

unsafe fn bootstrap_inner() -> Result<()> {
    let hr = match os_package_dependency_api() {
        Some(api) => unsafe { add_framework_dependency(api) },
        None => unsafe { initialize_bootstrapper()? },
    };
    if hr == STATEREPOSITORY_E_DEPENDENCY_NOT_RESOLVED {
        show_install_dialog();
        return Err(Error::new(
            hr,
            "Microsoft.WindowsAppRuntime.2 framework package is not installed.",
        ));
    }
    hr.ok()
}

/// Windows App SDK's own bootstrapper calls the Windows 11 dynamic dependency API only from
/// 24H1, the first release that exports `TryCreatePackageDependency2`; earlier builds need the
/// runtime's lifetime manager instead. Resolving the exports at run time keeps them out of the
/// import table, where Windows 10 would stop the loader before `main`.
fn os_package_dependency_api() -> Option<(TryCreatePackageDependencyFn, AddPackageDependencyFn)> {
    unsafe {
        let kernelbase = GetModuleHandleW(w!("kernelbase.dll"));
        if kernelbase.is_null() {
            return None;
        }
        GetProcAddress(kernelbase, s!("TryCreatePackageDependency2"))?;
        let try_create: TryCreatePackageDependencyFn = core::mem::transmute(GetProcAddress(
            kernelbase,
            s!("TryCreatePackageDependency"),
        )?);
        let add: AddPackageDependencyFn =
            core::mem::transmute(GetProcAddress(kernelbase, s!("AddPackageDependency"))?);
        Some((try_create, add))
    }
}

unsafe fn add_framework_dependency(
    (try_create, add): (TryCreatePackageDependencyFn, AddPackageDependencyFn),
) -> HRESULT {
    let mut dependency_id = PWSTR::null();
    let hr = unsafe {
        try_create(
            std::ptr::null_mut(),
            FRAMEWORK_FAMILY,
            minimum_version(),
            process_architecture().flags | PackageDependencyProcessorArchitectures_Neutral,
            PackageDependencyLifetimeKind_Process,
            PCWSTR::null(),
            CreatePackageDependencyOptions_None,
            &mut dependency_id,
        )
    };
    if hr.is_err() {
        return hr;
    }

    let mut handle = std::ptr::null_mut();
    let mut package_full_name = PWSTR::null();
    let hr = unsafe {
        add(
            PCWSTR(dependency_id.0),
            0,
            AddPackageDependencyOptions_None,
            &mut handle,
            &mut package_full_name,
        )
    };
    unsafe {
        _ = HeapFree(
            GetProcessHeap(),
            0,
            dependency_id.0.cast::<std::ffi::c_void>(),
        );
        _ = HeapFree(
            GetProcessHeap(),
            0,
            package_full_name.0.cast::<std::ffi::c_void>(),
        );
    }
    hr
}

/// The bootstrapper is app-local: load it from the executable's directory, never the DLL search
/// path. It stays loaded for the life of the process, as does the framework it adds.
unsafe fn initialize_bootstrapper() -> Result<HRESULT> {
    let path = std::env::current_exe()
        .map_err(|error| Error::new(E_FAIL, error.to_string()))?
        .with_file_name(BOOTSTRAP_DLL);
    let path = HSTRING::from(path.as_path());
    let module = unsafe {
        LoadLibraryExW(
            PCWSTR(path.as_ptr()),
            std::ptr::null_mut(),
            (LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32) as u32,
        )
    };
    if module.is_null() {
        return Err(Error::new(
            HRESULT::from_thread(),
            format!(
                "Could not load {BOOTSTRAP_DLL} beside the executable; Windows builds before \
                 Windows 11 24H1 need it to use the framework package."
            ),
        ));
    }
    let Some(initialize) = (unsafe { GetProcAddress(module, s!("MddBootstrapInitialize2")) })
    else {
        return Err(Error::new(
            HRESULT::from_thread(),
            format!("{BOOTSTRAP_DLL} does not export MddBootstrapInitialize2."),
        ));
    };
    let initialize: MddBootstrapInitialize2Fn = unsafe { core::mem::transmute(initialize) };
    // Release 2.0 and later select the framework by major version alone.
    Ok(unsafe {
        initialize(
            WINDOWSAPPSDK_RUNTIME_VERSION_MAJOR << 16 | WINDOWSAPPSDK_RUNTIME_VERSION_MINOR,
            PCWSTR::null(),
            minimum_version(),
            MDD_BOOTSTRAP_INITIALIZE_OPTIONS_NONE,
        )
    })
}

fn minimum_version() -> PACKAGE_VERSION {
    PACKAGE_VERSION {
        Anonymous: PACKAGE_VERSION_0 {
            Version: WINDOWSAPPSDK_RUNTIME_VERSION_UINT64,
        },
    }
}

#[derive(Clone, Copy)]
struct ProcessArchitecture {
    display: &'static str,
    flags: PackageDependencyProcessorArchitectures,
}

fn process_architecture() -> ProcessArchitecture {
    match std::env::consts::ARCH {
        "x86_64" => ProcessArchitecture {
            display: "x64",
            flags: PackageDependencyProcessorArchitectures_X64,
        },
        "x86" => ProcessArchitecture {
            display: "x86",
            flags: PackageDependencyProcessorArchitectures_X86,
        },
        "aarch64" => ProcessArchitecture {
            display: "arm64",
            flags: PackageDependencyProcessorArchitectures_Arm64,
        },
        "arm" => ProcessArchitecture {
            display: "arm",
            flags: PackageDependencyProcessorArchitectures_Arm,
        },
        _ => ProcessArchitecture {
            display: std::env::consts::ARCH,
            flags: PackageDependencyProcessorArchitectures_None,
        },
    }
}

fn process_caption() -> HSTRING {
    std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .map_or_else(
            || HSTRING::from("This application could not be started"),
            HSTRING::from,
        )
}

fn show_install_dialog() {
    let caption = process_caption();
    let text = HSTRING::from(format!(
        "You must install Windows App Runtime \
         ({WINDOWSAPPSDK_RUNTIME_VERSION_MAJOR}.{WINDOWSAPPSDK_RUNTIME_VERSION_MINOR}.\
         {WINDOWSAPPSDK_RUNTIME_VERSION_BUILD}.{WINDOWSAPPSDK_RUNTIME_VERSION_REVISION}, {}) \
         to run this application.\n\nDo you want to download it now?",
        process_architecture().display
    ));
    let result = unsafe {
        MessageBoxW(
            HWND::default(),
            PCWSTR::from_raw(text.as_ptr()),
            PCWSTR::from_raw(caption.as_ptr()),
            (MB_YESNO | MB_ICONERROR) as u32,
        )
    };
    if result == IDYES {
        unsafe {
            ShellExecuteW(
                HWND::default(),
                w!("open"),
                w!("https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads"),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}
