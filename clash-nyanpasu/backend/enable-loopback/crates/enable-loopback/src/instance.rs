//! GUI-only single instance: legacy mutex plus a locale-independent HWND mapping (§3.1/§4.1).

use snafu::Snafu;
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};
use windows::{
    Win32::{
        HANDLE, HWND, PAGE_READWRITE,
        errhandlingapi::GetLastError,
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        memoryapi::{
            CreateFileMappingW, FILE_MAP_READ, FILE_MAP_WRITE, MapViewOfFile, OpenFileMappingW,
            UnmapViewOfFile,
        },
        processthreadsapi::GetCurrentProcessId,
        synchapi::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
        winbase::{WAIT_ABANDONED, WAIT_OBJECT_0},
        winerror::{ERROR_INVALID_WINDOW_HANDLE, WAIT_TIMEOUT},
        winuser::{
            GetWindowThreadProcessId, IsIconic, IsWindow, SW_RESTORE, SetForegroundWindow,
            ShowWindow,
        },
    },
    core::HSTRING,
};

const NAME: &str = "EnableLoopbackTool";
pub const MUTEX_FAILURE: i32 = -2;
pub const SWITCHED: i32 = -3;
pub const CANNOT_SWITCH: i32 = -5;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Creating mutex {name:?} failed: 0x{code:x}"))]
    CreateMutex { name: String, code: u32 },
    #[snafu(display("Waiting for mutex {name:?} failed: 0x{code:x}"))]
    WaitMutex { name: String, code: u32 },
    #[snafu(display("Creating window mapping {name:?} failed: 0x{code:x}"))]
    CreateWindowMapping { name: String, code: u32 },
    #[snafu(display("Mapping shared window storage failed: 0x{code:x}"))]
    MapWindowStorage { code: u32 },
    #[snafu(display("Publishing window 0x{hwnd:x} failed: 0x{code:x}"))]
    PublishWindow { hwnd: usize, code: u32 },
}

fn last_code() -> u32 {
    // SAFETY: GetLastError has no preconditions; read before other native calls.
    unsafe { GetLastError() }
}

#[derive(Debug)]
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: Handle wraps only successfully created/opened kernel handles.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct MutexOwner {
    handle: Handle,
    // A Win32 mutex must be released on the acquiring thread.
    _thread: PhantomData<Rc<()>>,
}
impl Drop for MutexOwner {
    fn drop(&mut self) {
        // SAFETY: this guard is constructed only after acquiring the mutex on this thread.
        unsafe {
            let _ = ReleaseMutex(self.handle.0);
        }
    }
}

#[repr(C)]
struct SharedWindow {
    hwnd: AtomicUsize,
    process_id: AtomicU32,
}

#[derive(Debug)]
struct Mapping {
    view: *mut SharedWindow,
    _handle: Handle,
}
impl Mapping {
    fn map(handle: Handle, access: u32) -> Result<Self, Error> {
        // SAFETY: handle is a live mapping with at least SharedWindow bytes; offset is aligned.
        let view: *mut SharedWindow =
            unsafe { MapViewOfFile(handle.0, access, 0, 0, size_of::<SharedWindow>()) }.cast();
        if view.is_null() {
            return Err(Error::MapWindowStorage { code: last_code() });
        }
        Ok(Self {
            view,
            _handle: handle,
        })
    }

    fn shared(&self) -> &SharedWindow {
        // SAFETY: the mapped view stays live through self. Windows page alignment exceeds
        // atomic alignment; zero-filled page-file memory is a valid initial atomic value.
        // Both supported architectures provide lock-free, cross-process atomics here.
        unsafe { &*self.view }
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: view is the unmodified result of MapViewOfFile and is unmapped once.
        unsafe {
            let _ = UnmapViewOfFile(self.view.cast());
        }
    }
}

/// Clone its Rc into run_window. This owns the mapping, with no process-global state.
#[derive(Debug)]
pub struct WindowPublisher {
    mapping: Mapping,
}
impl WindowPublisher {
    /// Publish only after the native window exists.
    ///
    /// # Safety
    /// `hwnd` must be a live HWND on the calling UI thread, belonging to this process.
    pub unsafe fn publish_window(&self, hwnd: HWND) -> Result<(), Error> {
        let mut process_id = 0;
        // SAFETY: caller supplies a live HWND; the output counter is writable.
        let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
        // SAFETY: querying this process ID has no preconditions.
        if hwnd.is_null() || thread == 0 || process_id != unsafe { GetCurrentProcessId() } {
            return Err(Error::PublishWindow {
                hwnd: hwnd as usize,
                code: ERROR_INVALID_WINDOW_HANDLE as u32,
            });
        }
        let shared = self.mapping.shared();
        shared.process_id.store(process_id, Ordering::Relaxed);
        shared.hwnd.store(hwnd as usize, Ordering::Release);
        Ok(())
    }

    fn clear(&self) {
        self.mapping.shared().hwnd.store(0, Ordering::Release);
        self.mapping.shared().process_id.store(0, Ordering::Relaxed);
    }
}

pub struct Primary {
    publisher: Rc<WindowPublisher>,
    _mutex: MutexOwner,
}
impl Primary {
    pub fn publisher(&self) -> Rc<WindowPublisher> {
        self.publisher.clone()
    }
}
impl Drop for Primary {
    fn drop(&mut self) {
        self.publisher.clear();
    }
}

pub enum Outcome {
    Primary(Primary),
    /// A valid HWND was found and restoration/foreground activation was requested.
    Secondary {
        switched: bool,
    },
}

pub fn acquire() -> Result<Outcome, Error> {
    acquire_named(NAME)
}

fn acquire_named(name: &str) -> Result<Outcome, Error> {
    let wide = HSTRING::from(name);
    // SAFETY: live terminated name, default security, initially unowned like the original.
    let handle = unsafe { CreateMutexW(None, false, &wide) };
    if handle.is_null() {
        let code = last_code();
        return Err(Error::CreateMutex {
            name: name.into(),
            code,
        });
    }
    let handle = Handle(handle);
    // SAFETY: handle is a live mutex; retain the original 250ms wait.
    let wait = unsafe { WaitForSingleObject(handle.0, 250) };
    if wait != WAIT_OBJECT_0.0 as u32
        && wait != WAIT_ABANDONED.0 as u32
        && wait != WAIT_TIMEOUT as u32
    {
        let code = last_code();
        return Err(Error::WaitMutex {
            name: name.into(),
            code,
        });
    }
    let mapping_name = format!("{name}.Window"); // Mutex and mappings share a namespace: names must differ.
    match wait {
        value if value == WAIT_OBJECT_0.0 as u32 || value == WAIT_ABANDONED.0 as u32 => {
            // WAIT_OBJECT_0 or WAIT_ABANDONED: both transfer mutex ownership.
            let mutex = MutexOwner {
                handle,
                _thread: PhantomData,
            };
            let wide = HSTRING::from(&mapping_name);
            // SAFETY: page-file backed mapping, valid size, live terminated name.
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    None,
                    PAGE_READWRITE as u32,
                    0,
                    size_of::<SharedWindow>() as u32,
                    &wide,
                )
            };
            if handle.is_null() {
                return Err(Error::CreateWindowMapping {
                    name: mapping_name,
                    code: last_code(),
                });
            }
            let mapping = Mapping::map(Handle(handle), FILE_MAP_WRITE as u32)?;
            let publisher = Rc::new(WindowPublisher { mapping });
            publisher.clear(); // Clear stale data after abandoned-mutex recovery too.
            Ok(Outcome::Primary(Primary {
                publisher,
                _mutex: mutex,
            }))
        }
        value if value == WAIT_TIMEOUT as u32 => Ok(Outcome::Secondary {
            switched: switch_to_window(&mapping_name),
        }),
        _ => unreachable!("wait status checked before allocating the mapping name"),
    }
}

fn read_window(name: &str) -> Option<(HWND, u32)> {
    let wide = HSTRING::from(name);
    // SAFETY: live terminated mapping name; second instances only read shared storage.
    let handle = unsafe { OpenFileMappingW(FILE_MAP_READ as u32, false, &wide) };
    if handle.is_null() {
        return None;
    } // Also covers the old tool, which publishes no HWND.
    let mapping = Mapping::map(Handle(handle), FILE_MAP_READ as u32).ok()?;
    let shared = mapping.shared();
    let hwnd = shared.hwnd.load(Ordering::Acquire);
    if hwnd == 0 {
        return None;
    }
    Some((hwnd as HWND, shared.process_id.load(Ordering::Relaxed)))
}

fn switch_to_window(name: &str) -> bool {
    let Some((hwnd, expected_process)) = read_window(name) else {
        return false;
    };
    let mut process_id = 0;
    // SAFETY: IsWindow accepts opaque handles and validates this shared value.
    let valid = unsafe { IsWindow(Some(hwnd)) }.as_bool();
    // SAFETY: the window API validates the opaque handle; the counter is writable.
    let thread = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };
    if !valid || thread == 0 || process_id != expected_process {
        return false;
    }
    // The newly launched second instance calls SetForegroundWindow directly. It must
    // already have foreground permission. AllowSetForegroundWindow grants permission
    // to ANOTHER process; it cannot grant permission to ourselves or bypass the OS lock.
    // SAFETY: validated HWND is used only by window APIs, never dereferenced by Rust.
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        if !SetForegroundWindow(hwnd).as_bool() {
            crate::debug(
                "Existing EnableLoopback window found; Windows declined foreground activation",
            );
        }
    }
    true // Like the original, -3 means a window was found even if foreground is restricted.
}

#[cfg(test)]
pub(crate) fn acquire_for_test(suffix: &str) -> Primary {
    let name = format!("EnableLoopbackTool.UiTest.{}.{suffix}", std::process::id());
    let Outcome::Primary(primary) = acquire_named(&name).unwrap() else {
        panic!("isolated test mutex must be free");
    };
    primary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mutex_mapping_wait_and_cleanup() {
        // Isolated names ensure this test never interacts with a running GUI or old tool.
        let name = format!("EnableLoopbackTool.Test.{}", std::process::id());
        let Outcome::Primary(primary) = acquire_named(&name).unwrap() else {
            panic!("expected ownership");
        };
        assert!(read_window(&format!("{name}.Window")).is_none());
        let second_name = name.clone();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            assert!(matches!(
                acquire_named(&second_name).unwrap(),
                Outcome::Secondary { switched: false }
            ));
            assert!(start.elapsed() >= std::time::Duration::from_millis(200));
        })
        .join()
        .unwrap();
        drop(primary);
        assert!(matches!(acquire_named(&name).unwrap(), Outcome::Primary(_)));
    }

    #[test]
    fn mapping_publishes_without_truncating_hwnd_and_clears_before_release() {
        let name = format!("EnableLoopbackTool.MappingTest.{}", std::process::id());
        let Outcome::Primary(primary) = acquire_named(&name).unwrap() else {
            panic!("expected ownership");
        };
        let publisher = primary.publisher();
        let shared = publisher.mapping.shared();
        shared.process_id.store(42, Ordering::Relaxed);
        shared.hwnd.store(0x0012_3456_78ab_cdef, Ordering::Release);
        assert_eq!(
            read_window(&format!("{name}.Window")),
            Some((0x0012_3456_78ab_cdef_usize as HWND, 42))
        );
        drop(primary);
        assert!(read_window(&format!("{name}.Window")).is_none());
    }
}
