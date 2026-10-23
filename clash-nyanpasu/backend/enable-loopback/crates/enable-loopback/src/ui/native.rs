//! Small native presentation helpers, kept outside the pure model.
use super::Error;
use windows::{
    Win32::datetimeapi::GetTimeFormatEx,
    core::{PCWSTR, PWSTR},
};

pub(super) fn time() -> Result<String, Error> {
    // SAFETY: null locale selects LOCALE_NAME_USER_DEFAULT, null time selects now,
    // and null format with flags zero selects the user's long time format.
    let length = unsafe { GetTimeFormatEx(PCWSTR::null(), 0, None, PCWSTR::null(), None, 0) };
    if length == 0 {
        return Err(failure("GetTimeFormatEx"));
    }
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: buffer holds the character count returned by the sizing call.
    let written = unsafe {
        GetTimeFormatEx(
            PCWSTR::null(),
            0,
            None,
            PCWSTR::null(),
            Some(PWSTR(buffer.as_mut_ptr())),
            length,
        )
    };
    if written == 0 {
        return Err(failure("GetTimeFormatEx"));
    }
    Ok(String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

pub(super) fn failure(operation: &'static str) -> Error {
    Error::Native {
        operation,
        source: windows::core::Error::from_thread(),
    }
}

/// Transfer an owned movable UTF-16 allocation to the clipboard only on success.
pub(super) fn clipboard(hwnd: usize, text: &str) -> Result<(), Error> {
    use windows::Win32::{
        HGLOBAL,
        winbase::{GMEM_MOVEABLE, GlobalAlloc, GlobalFree, GlobalLock, GlobalUnlock},
        winuser::{
            CF_UNICODETEXT, CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        },
    };
    struct Allocation(HGLOBAL);
    impl Drop for Allocation {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: until SetClipboardData succeeds this guard owns the allocation.
                unsafe {
                    GlobalFree(self.0);
                }
            }
        }
    }
    struct Clipboard;
    impl Drop for Clipboard {
        fn drop(&mut self) {
            // SAFETY: constructed only after this UI thread opens the clipboard.
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }
    let wide: Vec<u16> = text.encode_utf16().chain([0]).collect();
    // SAFETY: nonzero checked UTF-16 buffer size; movable memory is required by Win32.
    let mut allocation = Allocation(unsafe {
        GlobalAlloc(GMEM_MOVEABLE as u32, std::mem::size_of_val(wide.as_slice()))
    });
    if allocation.0.is_null() {
        return Err(failure("GlobalAlloc"));
    }
    // SAFETY: the guard owns live global memory and no other thread uses it.
    let pointer = unsafe { GlobalLock(allocation.0) }.cast::<u16>();
    if pointer.is_null() {
        return Err(failure("GlobalLock"));
    }
    // SAFETY: allocation has exactly wide.len() u16 elements; buffers do not overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer, wide.len());
        // Returning zero when the lock count reaches zero is normal.
        let _ = GlobalUnlock(allocation.0);
    }
    // SAFETY: hwnd was supplied by run_window and is live on this UI thread.
    if !unsafe { OpenClipboard(Some(hwnd as _)) }.as_bool() {
        return Err(failure("OpenClipboard"));
    }
    let _clipboard = Clipboard;
    // SAFETY: clipboard is held open by this thread; hwnd becomes its owner.
    if !unsafe { EmptyClipboard() }.as_bool() {
        return Err(failure("EmptyClipboard"));
    }
    // SAFETY: unlocked movable memory contains NUL-terminated UTF-16. Success transfers
    // ownership to Windows; failure leaves ownership with the allocation guard.
    if unsafe { SetClipboardData(CF_UNICODETEXT as u32, Some(allocation.0)) }.is_null() {
        return Err(failure("SetClipboardData"));
    }
    allocation.0 = std::ptr::null_mut();
    Ok(())
}

/// PointerEventInfo is in window-relative DIPs (declaration.rs); ScreenPoint is in
/// physical pixels (transient_menu.rs::position_host uses PhysicalCoordinates).
pub(super) fn screen_point(
    hwnd: usize,
    event: windows_reactor::PointerEventInfo,
) -> Result<windows_reactor::ScreenPoint, Error> {
    use windows::Win32::{
        POINT,
        winuser::{ClientToScreen, GetDpiForWindow},
    };
    // SAFETY: hwnd comes from run_window on this UI thread and remains live.
    let dpi = unsafe { GetDpiForWindow(hwnd as _) };
    if dpi == 0 {
        return Err(failure("GetDpiForWindow"));
    }
    let scale = f64::from(dpi) / 96.0;
    let mut point = POINT {
        x: (event.window_x * scale).round() as i32,
        y: (event.window_y * scale).round() as i32,
    };
    // SAFETY: writable physical client point and live per-monitor-DPI-aware HWND.
    if !unsafe { ClientToScreen(hwnd as _, &mut point) }.as_bool() {
        return Err(failure("ClientToScreen"));
    }
    Ok(windows_reactor::ScreenPoint::new(point.x, point.y))
}

/// Top-left that centers a window with this DIP client size in the work area of the
/// monitor under the cursor, where the user launched it. ScreenPoint is in physical pixels.
pub(super) fn centered_position(
    client_width: f64,
    client_height: f64,
) -> Result<windows_reactor::ScreenPoint, Error> {
    use windows::Win32::{
        POINT, RECT,
        shellscalingapi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
        winuser::{
            AdjustWindowRectExForDpi, GetCursorPos, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST,
            MONITORINFO, MonitorFromPoint, WS_OVERLAPPEDWINDOW,
        },
    };
    let mut cursor = POINT::default();
    // SAFETY: writable POINT.
    if !unsafe { GetCursorPos(&mut cursor) }.as_bool() {
        return Err(failure("GetCursorPos"));
    }
    // SAFETY: NEAREST always returns a monitor.
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST as u32) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: live monitor handle and a sized MONITORINFO.
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return Err(failure("GetMonitorInfoW"));
    }
    let (mut dpi, mut unused) = (0, 0);
    // SAFETY: live monitor handle and writable outputs.
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut unused) }
        .ok()
        .map_err(|source| Error::Native {
            operation: "GetDpiForMonitor",
            source,
        })?;
    let scale = f64::from(dpi) / 96.0;
    let mut outer = RECT {
        left: 0,
        top: 0,
        right: (client_width * scale).round() as i32,
        bottom: (client_height * scale).round() as i32,
    };
    // SAFETY: writable RECT; the style matches WinUI's overlapped window frame.
    if !unsafe { AdjustWindowRectExForDpi(&mut outer, WS_OVERLAPPEDWINDOW as u32, false, 0, dpi) }
        .as_bool()
    {
        return Err(failure("AdjustWindowRectExForDpi"));
    }
    let work = info.rcWork;
    let width = outer.right - outer.left;
    let height = outer.bottom - outer.top;
    Ok(windows_reactor::ScreenPoint::new(
        work.left + (work.right - work.left - width) / 2,
        // A window taller than the work area keeps its caption reachable.
        (work.top + (work.bottom - work.top - height) / 2).max(work.top),
    ))
}

/// All four flags are shared only by UI-thread code. Rc makes accidental Send impossible.
#[derive(Clone, Default)]
pub(super) struct CloseState {
    pub dirty: std::rc::Rc<std::cell::Cell<bool>>,
    pub saving: std::rc::Rc<std::cell::Cell<bool>>,
    pub confirmed: std::rc::Rc<std::cell::Cell<bool>>,
    pub requested: std::rc::Rc<std::cell::Cell<bool>>,
}

struct CloseSubclass {
    state: CloseState,
    sender: windows_reactor::LocalSender<super::Message>,
}
const CLOSE_SUBCLASS_ID: usize = 1;

pub(super) fn install_close(
    window: &windows_reactor::WindowHandle<'_>,
    state: CloseState,
    sender: windows_reactor::LocalSender<super::Message>,
) -> Result<(), Error> {
    use windows::Win32::commctrl::SetWindowSubclass;
    let data = Box::into_raw(Box::new(CloseSubclass { state, sender }));
    // SAFETY: run_window provides a live HWND on its UI thread. The box contains only
    // UI-thread state and is owned by this subclass until WM_NCDESTROY. It is never global.
    if !unsafe {
        SetWindowSubclass(
            window.as_raw().cast(),
            Some(close_proc),
            CLOSE_SUBCLASS_ID,
            data as usize,
        )
    }
    .as_bool()
    {
        let error = failure("SetWindowSubclass");
        // SAFETY: installation failed, so native code never took ownership of the box.
        unsafe {
            drop(Box::from_raw(data));
        }
        return Err(error);
    }
    Ok(())
}

unsafe extern "system" fn close_proc(
    hwnd: windows::Win32::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
    id: usize,
    data: usize,
) -> isize {
    use windows::Win32::{
        commctrl::{DefSubclassProc, RemoveWindowSubclass},
        winuser::{WM_CLOSE, WM_NCDESTROY},
    };
    if message == WM_NCDESTROY as u32 {
        // SAFETY: data is our unique live box. Remove the callback before releasing it;
        // subsequent destruction messages cannot access the freed UI-thread Rc values.
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(close_proc), id);
            drop(Box::from_raw(data as *mut CloseSubclass));
            return DefSubclassProc(hwnd, message, wparam, lparam);
        }
    }
    // SAFETY: successful installation retains this box until WM_NCDESTROY on this thread.
    let subclass = unsafe { &*(data as *const CloseSubclass) };
    if message == WM_CLOSE as u32 && !subclass.state.confirmed.get() {
        if subclass.state.saving.get() {
            return 0;
        }
        if subclass.state.dirty.get() {
            if !subclass.state.requested.replace(true)
                && !subclass.sender.send(super::Message::CloseRequested)
            {
                subclass.state.requested.set(false);
            }
            return 0;
        }
    }
    // SAFETY: all unconsumed messages retain their original parameters and subclass chain.
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn version() -> Result<String, Error> {
    use windows::Win32::{sysinfoapi::GetVersionExW, winnt::OSVERSIONINFOW};
    let mut version = OSVERSIONINFOW {
        dwOSVersionInfoSize: size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: writable version structure has its required size field set. The executable's
    // supportedOS Windows 10 manifest enables the real Windows 10/11 version and build.
    if !unsafe { GetVersionExW(&mut version) }.as_bool() {
        return Err(failure("GetVersionExW"));
    }
    Ok(format!(
        "enable-loopback v{} {} Windows {}.{}.{}",
        env!("CARGO_PKG_VERSION"),
        if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "64bit"
        },
        version.dwMajorVersion,
        version.dwMinorVersion,
        version.dwBuildNumber
    ))
}

pub(super) fn open_help(hwnd: usize, url: &str) -> Result<(), Error> {
    use windows::{
        Win32::{
            shellapi::{SE_ERR_DLLNOTFOUND, ShellExecuteW},
            winuser::SW_SHOWNORMAL,
        },
        core::HSTRING,
    };
    let action = HSTRING::from("open");
    let url = HSTRING::from(url);
    // SAFETY: live UI-thread HWND, terminated verb and fixed URL, no parameters/directory.
    let result = unsafe {
        ShellExecuteW(
            Some(hwnd as _),
            &action,
            &url,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    // ShellExecuteW's documented error range ends at SE_ERR_DLLNOTFOUND (shellapi.h).
    if result <= SE_ERR_DLLNOTFOUND as isize {
        return Err(Error::OpenHelp {
            code: result as usize,
        });
    }
    Ok(())
}
