//! Own FirewallAPI data and allocations; no native pointers escape this module (§2/§2.1).

use snafu::Snafu;
use std::{collections::BTreeMap, ffi::c_void, fmt, ptr, str::FromStr};
use windows::{
    Win32::{
        KEY_READ, PSID, SID_AND_ATTRIBUTES,
        errhandlingapi::GetLastError,
        heapapi::{GetProcessHeap, HeapFree},
        netfw::*,
        processenv::ExpandEnvironmentStringsW,
        sddl::{ConvertSidToStringSidW, ConvertStringSidToSidW},
        secext::{GetUserNameExW, NameSamCompatible},
        securitybaseapi::{GetLengthSid, IsValidSid},
        shlwapi::SHLoadIndirectString,
        winbase::{GetComputerNameW, LocalFree, LookupAccountSidW},
        winerror::{
            ERROR_ACCESS_DENIED, ERROR_FILE_NOT_FOUND, ERROR_INVALID_DATA, ERROR_INVALID_PARAMETER,
            ERROR_INVALID_SID, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, S_OK,
        },
        winnt::{REG_EXPAND_SZ, REG_SZ},
        winreg::*,
    },
    core::{HSTRING, PCWSTR, PWSTR},
};

/// Validated, owned SID bytes. Construction always copies out of native storage.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sid(Vec<u8>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppContainer {
    pub sid: Sid,
    pub users: Vec<String>,
    pub name: Option<String>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub package_full_name: Option<String>,
    pub binaries: Vec<String>,
    pub exempt_at_last_check: bool,
}

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display(
        "Enumerating AppContainers failed (force_binaries={force_binaries}): 0x{code:x}"
    ))]
    Enumerate { force_binaries: bool, code: u32 },
    #[snafu(display("Reading loopback exemptions failed: 0x{code:x}"))]
    ReadExemptions { code: u32 },
    #[snafu(display(
        "Writing {sid_count} loopback exemptions requires an administrator: 0x{code:x}"
    ))]
    SetExemptionsAccessDenied { sid_count: usize, code: u32 },
    #[snafu(display("Writing {sid_count} loopback exemptions failed: 0x{code:x}"))]
    SetExemptions { sid_count: usize, code: u32 },
    #[snafu(display("Validating a native SID failed: 0x{code:x}"))]
    ValidateSid { code: u32 },
    #[snafu(display("Parsing SID {text:?} failed: 0x{code:x}"))]
    ParseSid { text: String, code: u32 },
    #[snafu(display("Formatting a SID failed: 0x{code:x}"))]
    FormatSid { code: u32 },
    #[snafu(display(r"Reading the current DOMAIN\user failed: 0x{code:x}"))]
    ReadCurrentUser { code: u32 },
    #[snafu(display("Registry step {step} at {path:?} failed: 0x{code:x}"))]
    ReadRegistry {
        step: &'static str,
        path: String,
        code: u32,
    },
}

fn last_code() -> u32 {
    // SAFETY: GetLastError has no preconditions; read before any further native call.
    unsafe { GetLastError() }
}

struct LocalAllocation(*mut c_void);
impl Drop for LocalAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: only LocalAlloc-owned SID/string results are wrapped here.
            unsafe { LocalFree(self.0) };
        }
    }
}

impl Sid {
    /// Copy a SID from native storage.
    ///
    /// # Safety
    /// A non-null `sid` must point to readable SID storage through this call.
    pub unsafe fn from_psid(sid: PSID) -> Result<Self, Error> {
        // SAFETY: caller guarantees readable storage; null is checked before validation.
        if sid.is_null() || !unsafe { IsValidSid(sid) }.as_bool() {
            return Err(Error::ValidateSid {
                code: ERROR_INVALID_SID as u32,
            });
        }
        // SAFETY: IsValidSid succeeded and the caller keeps the allocation alive.
        let length = unsafe { GetLengthSid(sid) } as usize;
        // SAFETY: GetLengthSid gives the readable byte extent of this validated SID.
        Ok(Self(
            unsafe { std::slice::from_raw_parts(sid.cast::<u8>(), length) }.to_vec(),
        ))
    }

    fn as_psid(&self) -> PSID {
        self.0.as_ptr().cast_mut().cast()
    }

    pub fn to_sid_string(&self) -> Result<String, Error> {
        let mut text = PWSTR::null();
        // SAFETY: self owns a validated SID; text receives a LocalAlloc-owned string.
        if !unsafe { ConvertSidToStringSidW(self.as_psid(), &mut text) }.as_bool() {
            return Err(Error::FormatSid { code: last_code() });
        }
        let _allocation = LocalAllocation(text.0.cast());
        // SAFETY: the API returned a NUL-terminated string, kept alive by the guard.
        Ok(unsafe { text.to_string() }.unwrap_or_default())
    }
}

impl FromStr for Sid {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        let wide = HSTRING::from(text);
        let mut sid = ptr::null_mut();
        // SAFETY: wide is terminated and live; sid receives a LocalAlloc-owned SID.
        if text.contains('\0') || !unsafe { ConvertStringSidToSidW(&wide, &mut sid) }.as_bool() {
            let code = if text.contains('\0') {
                ERROR_INVALID_PARAMETER as u32
            } else {
                last_code()
            };
            return Err(Error::ParseSid {
                text: text.into(),
                code,
            });
        }
        let _allocation = LocalAllocation(sid);
        // SAFETY: successful conversion returned live SID storage guarded above.
        unsafe { Self::from_psid(sid) }
    }
}

impl fmt::Display for Sid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_sid_string().map_err(|_| fmt::Error)?)
    }
}

struct ContainersAllocation(PINET_FIREWALL_APP_CONTAINER);
impl Drop for ContainersAllocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: pointer is the unmodified enumeration result; this frees all nested data.
            unsafe { NetworkIsolationFreeAppContainers(self.0) };
        }
    }
}

/// Copy an optional native string without conflating null and the empty string.
unsafe fn optional_string(text: PWSTR) -> Option<String> {
    if text.is_null() {
        None
    } else {
        // SAFETY: caller guarantees a live, terminated native string.
        Some(String::from_utf16_lossy(unsafe { text.as_wide() }))
    }
}

fn resolve_indirect(text: Option<String>) -> Option<String> {
    let raw = text?;
    if !raw.starts_with('@') {
        return Some(raw);
    }
    let source = HSTRING::from(&raw);
    let mut output = [0u16; 1024]; // Original LoadIndirectString buffer size.
    // SAFETY: owned source and writable output live throughout the call; reserved is null.
    let result = unsafe {
        SHLoadIndirectString(
            &source,
            PWSTR(output.as_mut_ptr()),
            output.len() as u32,
            None,
        )
    };
    if result == S_OK {
        Some(String::from_utf16_lossy(
            &output[..output.iter().position(|c| *c == 0).unwrap_or(output.len())],
        ))
    } else {
        Some(raw)
    }
}

fn computer_prefix() -> Option<String> {
    let mut buffer = [0u16; 256];
    let mut length = buffer.len() as u32;
    // SAFETY: buffer is writable and length describes its capacity.
    if unsafe { GetComputerNameW(Some(PWSTR(buffer.as_mut_ptr())), &mut length) }.as_bool() {
        Some(format!(
            "{}\\",
            String::from_utf16_lossy(&buffer[..length as usize])
        ))
    } else {
        None
    }
}

fn account_name(sid: &Sid, machine: Option<&str>) -> String {
    let (mut name_length, mut domain_length, mut usage) = (0, 0, 0);
    // SAFETY: SID is owned; null output buffers request sizes.
    unsafe {
        let _ = LookupAccountSidW(
            PCWSTR::null(),
            sid.as_psid(),
            None,
            &mut name_length,
            None,
            &mut domain_length,
            &mut usage,
        );
    };
    let mut name = vec![0u16; name_length as usize];
    let mut domain = vec![0u16; domain_length as usize];
    // SAFETY: buffers have the requested capacities and SID remains live.
    if name.is_empty()
        || !unsafe {
            LookupAccountSidW(
                PCWSTR::null(),
                sid.as_psid(),
                Some(PWSTR(name.as_mut_ptr())),
                &mut name_length,
                Some(PWSTR(domain.as_mut_ptr())),
                &mut domain_length,
                &mut usage,
            )
        }
        .as_bool()
    {
        return sid.to_string();
    }
    let name = String::from_utf16_lossy(&name[..name_length as usize]);
    let domain = String::from_utf16_lossy(&domain[..domain_length as usize]);
    let full = if domain.is_empty() {
        name
    } else {
        format!(r"{domain}\{name}")
    };
    full.strip_prefix(machine.unwrap_or("\0"))
        .unwrap_or(&full)
        .to_owned()
}

/// Current account in SAM-compatible DOMAIN\user form, without machine-prefix stripping.
pub fn current_user() -> Result<String, Error> {
    let mut length = 0;
    // SAFETY: null output buffer requests the required character count.
    unsafe { GetUserNameExW(NameSamCompatible, None, &mut length) };
    for _ in 0..3 {
        let mut buffer = vec![0u16; length as usize];
        // SAFETY: writable buffer has the capacity given by length.
        if unsafe {
            GetUserNameExW(
                NameSamCompatible,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
            )
        } != 0
        {
            return Ok(String::from_utf16_lossy(&buffer[..length as usize]));
        }
        let code = last_code();
        if code != ERROR_MORE_DATA as u32 {
            return Err(Error::ReadCurrentUser { code });
        }
    }
    Err(Error::ReadCurrentUser {
        code: ERROR_MORE_DATA as u32,
    })
}

/// Keep the first record's metadata and enumeration position, appending all users.
pub(crate) fn merge_records(records: impl IntoIterator<Item = AppContainer>) -> Vec<AppContainer> {
    let mut positions = BTreeMap::new();
    let mut result: Vec<AppContainer> = Vec::new();
    for mut record in records {
        if let Some(&index) = positions.get(&record.sid) {
            let existing: &mut AppContainer = &mut result[index];
            existing.users.append(&mut record.users);
            existing
                .users
                .sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        } else {
            record
                .users
                .sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            positions.insert(record.sid.clone(), result.len());
            result.push(record);
        }
    }
    result
}

pub fn enumerate(force_binaries: bool) -> Result<Vec<AppContainer>, Error> {
    let (mut count, mut pointer) = (0, ptr::null_mut());
    let flags = if force_binaries {
        NETISO_FLAG_FORCE_COMPUTE_BINARIES as u32
    } else {
        0
    };
    // SAFETY: valid out-pointers; the returned allocation is immediately guarded.
    let code = unsafe { NetworkIsolationEnumAppContainers(flags, &mut count, &mut pointer) };
    let _allocation = ContainersAllocation(pointer);
    if code != ERROR_SUCCESS as u32 {
        return Err(Error::Enumerate {
            force_binaries,
            code,
        });
    }
    let machine = computer_prefix();
    let mut records = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        // SAFETY: successful enumeration returned count entries, all live under the guard.
        let native = unsafe { &*pointer.add(index) };
        // SAFETY: native SIDs remain live until the enumeration guard drops.
        let sid = unsafe { Sid::from_psid(native.appContainerSid.cast()) }?;
        // SAFETY: native user SID remains live; invalid/missing users leave an empty list.
        let users = unsafe { Sid::from_psid(native.userSid.cast()) }
            .ok()
            .map(|sid| account_name(&sid, machine.as_deref()))
            .into_iter()
            .collect();
        let mut binaries = Vec::new();
        if !native.binaries.binaries.is_null() {
            for index in 0..native.binaries.count as usize {
                // SAFETY: the native binary array and its strings are owned by the guard.
                binaries.push(
                    unsafe { optional_string(*native.binaries.binaries.add(index)) }
                        .unwrap_or_default(),
                );
            }
        }
        // SAFETY: native optional strings are terminated and owned by the enumeration guard.
        let (name, display_name, description, package_full_name) = unsafe {
            (
                optional_string(native.appContainerName),
                optional_string(native.displayName),
                optional_string(native.description),
                optional_string(native.packageFullName),
            )
        };
        records.push(AppContainer {
            sid,
            users,
            name,
            display_name: resolve_indirect(display_name),
            description: resolve_indirect(description),
            package_full_name,
            binaries,
            exempt_at_last_check: false,
        });
    }
    Ok(merge_records(records))
}

struct ConfigAllocation {
    pointer: *mut SID_AND_ATTRIBUTES,
    count: u32,
}
impl Drop for ConfigAllocation {
    fn drop(&mut self) {
        if self.pointer.is_null() {
            return;
        }
        // Per netfw.h / Microsoft's FreeAppContainerConfig example:
        // https://learn.microsoft.com/windows/win32/api/netfw/nf-netfw-networkisolationgetappcontainerconfig
        // Every SID AND the array belong to the process heap. The original FreeHGlobal
        // (LocalFree) only freed the array; follow the documented complete cleanup instead.
        // SAFETY: successful API allocation contains count individually heap-allocated SIDs.
        unsafe {
            let heap = GetProcessHeap();
            for index in 0..self.count as usize {
                let _ = HeapFree(heap, 0, (*self.pointer.add(index)).Sid);
            }
            let _ = HeapFree(heap, 0, self.pointer.cast());
        }
    }
}

pub fn exempted_sids() -> Result<Vec<Sid>, Error> {
    let (mut count, mut pointer) = (0, ptr::null_mut());
    // SAFETY: valid out-pointers; no elevation is required for this read.
    let code = unsafe { NetworkIsolationGetAppContainerConfig(&mut count, &mut pointer) };
    let allocation = ConfigAllocation { pointer, count };
    if code != ERROR_SUCCESS as u32 {
        return Err(Error::ReadExemptions { code });
    }
    (0..count as usize)
        .map(|index| {
            // SAFETY: array and SID storage remain live through allocation's Drop.
            unsafe { Sid::from_psid((*allocation.pointer.add(index)).Sid) }
        })
        .collect()
}

/// Replaces the entire exemption table, including orphan removal (§3.5).
pub fn set_exempted(sids: &[Sid]) -> Result<(), Error> {
    let attributes: Vec<_> = sids
        .iter()
        .map(|sid| SID_AND_ATTRIBUTES {
            Sid: sid.as_psid(),
            Attributes: 0,
        })
        .collect();
    // SAFETY: attributes reference validated owned SIDs live through this synchronous call.
    let code = unsafe { NetworkIsolationSetAppContainerConfig(&attributes) };
    match code as i32 {
        ERROR_SUCCESS => Ok(()),
        ERROR_ACCESS_DENIED => Err(Error::SetExemptionsAccessDenied {
            sid_count: sids.len(),
            code,
        }),
        _ => Err(Error::SetExemptions {
            sid_count: sids.len(),
            code,
        }),
    }
}

const MAPPINGS: &str = r"Software\Classes\Local Settings\Software\Microsoft\Windows\CurrentVersion\AppContainer\Mappings";
struct RegistryKey(windows::Win32::HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: this guard owns a successfully opened registry handle.
        unsafe { RegCloseKey(self.0) };
    }
}
fn open_key(parent: windows::Win32::HKEY, path: &str) -> Result<RegistryKey, Error> {
    let wide = HSTRING::from(path);
    let mut key = ptr::null_mut();
    // SAFETY: live terminated path and valid out-handle; read-only access.
    let code = unsafe { RegOpenKeyExW(parent, &wide, None, KEY_READ as u32, &mut key) };
    if code != ERROR_SUCCESS {
        return Err(Error::ReadRegistry {
            step: "open",
            path: path.into(),
            code: code as u32,
        });
    }
    Ok(RegistryKey(key))
}
fn registry_string(key: &RegistryKey, name: &str) -> Result<Option<String>, Error> {
    let wide = HSTRING::from(name);
    let (mut size, mut kind) = (0, 0);
    // SAFETY: valid handle and counters; null buffer requests size.
    let code = unsafe { RegQueryValueExW(key.0, &wide, None, Some(&mut kind), None, &mut size) };
    if code == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if code != ERROR_SUCCESS {
        return Err(Error::ReadRegistry {
            step: "query size",
            path: name.into(),
            code: code as u32,
        });
    }
    for _ in 0..3 {
        let mut buffer = vec![0u16; (size as usize).div_ceil(2) + 1];
        let mut capacity = (buffer.len() * 2) as u32;
        // SAFETY: buffer has capacity bytes, aligned for UTF-16 and kept live.
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                &wide,
                None,
                Some(&mut kind),
                Some(buffer.as_mut_ptr().cast()),
                &mut capacity,
            )
        };
        if code == ERROR_MORE_DATA {
            size = capacity;
            continue;
        }
        if code != ERROR_SUCCESS || !matches!(kind, REG_SZ | REG_EXPAND_SZ) {
            return Err(Error::ReadRegistry {
                step: "read string",
                path: name.into(),
                code: if code == ERROR_SUCCESS {
                    ERROR_INVALID_DATA as u32
                } else {
                    code as u32
                },
            });
        }
        let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        let value = String::from_utf16_lossy(&buffer[..length]);
        return Ok(Some(if kind == REG_EXPAND_SZ {
            expand_registry_string(&value)?
        } else {
            value
        }));
    }
    Err(Error::ReadRegistry {
        step: "read changing value",
        path: name.into(),
        code: ERROR_MORE_DATA as u32,
    })
}

/// RegistryKey.GetValue expands REG_EXPAND_SZ by default in the original tool.
fn expand_registry_string(value: &str) -> Result<String, Error> {
    let wide = HSTRING::from(value);
    // SAFETY: source is live and terminated; null output requests the required size.
    let mut length = unsafe { ExpandEnvironmentStringsW(&wide, None, 0) };
    for _ in 0..3 {
        if length == 0 {
            let code = last_code();
            return Err(Error::ReadRegistry {
                step: "expand string",
                path: value.into(),
                code,
            });
        }
        let mut buffer = vec![0u16; length as usize];
        // SAFETY: source remains live, and buffer has length writable UTF-16 code units.
        let required =
            unsafe { ExpandEnvironmentStringsW(&wide, Some(PWSTR(buffer.as_mut_ptr())), length) };
        if required == 0 {
            let code = last_code();
            return Err(Error::ReadRegistry {
                step: "expand string",
                path: value.into(),
                code,
            });
        }
        if required <= length {
            return Ok(String::from_utf16_lossy(&buffer[..required as usize - 1]));
        }
        length = required; // Retry if the environment changed between the size/read calls.
    }
    Err(Error::ReadRegistry {
        step: "expand changing string",
        path: value.into(),
        code: ERROR_MORE_DATA as u32,
    })
}

/// Read the current user's mappings directly. Per the original, skip malformed subkeys
/// with a debug diagnostic; registry display names are not indirect-string resolved.
pub fn enumerate_from_registry() -> Result<Vec<AppContainer>, Error> {
    let root = open_key(HKEY_CURRENT_USER, MAPPINGS)?;
    let user = current_user()?;
    let mut records = Vec::new();
    for index in 0..u32::MAX {
        let mut buffer = [0u16; 256]; // Registry subkey names are limited to 255 characters.
        let mut length = buffer.len() as u32;
        // SAFETY: root is live, name buffer is writable and capacity is supplied.
        let code = unsafe {
            RegEnumKeyExW(
                root.0,
                index,
                Some(PWSTR(buffer.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if code == ERROR_NO_MORE_ITEMS {
            break;
        }
        if code != ERROR_SUCCESS {
            return Err(Error::ReadRegistry {
                step: "enumerate",
                path: MAPPINGS.into(),
                code: code as u32,
            });
        }
        let name = String::from_utf16_lossy(&buffer[..length as usize]);
        let read = || -> Result<AppContainer, Error> {
            let key = open_key(root.0, &name)?;
            Ok(AppContainer {
                sid: name.parse()?,
                users: vec![user.clone()],
                name: registry_string(&key, "Moniker")?,
                display_name: registry_string(&key, "DisplayName")?,
                description: Some("?".into()),
                package_full_name: Some("?".into()),
                binaries: Vec::new(),
                exempt_at_last_check: false,
            })
        };
        match read() {
            Ok(record) => records.push(record),
            Err(error) => crate::debug(&error.to_string()),
        }
    }
    if records.is_empty() {
        return Err(Error::ReadRegistry {
            step: "find mappings",
            path: MAPPINGS.into(),
            code: ERROR_NO_MORE_ITEMS as u32,
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_real_netiso_smoke() {
        let apps = enumerate(false).expect("unelevated NetIso enumeration");
        let exemptions = exempted_sids().expect("unelevated NetIso configuration read");
        for sid in apps.iter().map(|app| &app.sid).chain(&exemptions) {
            assert_eq!(&sid.to_string().parse::<Sid>().unwrap(), sid);
        }
        let unique: std::collections::BTreeSet<_> = apps.iter().map(|app| &app.sid).collect();
        assert_eq!(unique.len(), apps.len());
        for app in apps {
            assert!(
                app.users
                    .windows(2)
                    .all(|pair| pair[0].encode_utf16().cmp(pair[1].encode_utf16()).is_le())
            );
        }
    }

    #[test]
    fn current_account_and_registry_expansion_are_readonly() {
        let user = current_user().unwrap();
        assert!(user.contains('\\'));
        assert!(!user.ends_with('\0'));
        assert_eq!(expand_registry_string("").unwrap(), "");
        assert_eq!(expand_registry_string("literal").unwrap(), "literal");
        assert_eq!(
            expand_registry_string("%SystemRoot%").unwrap(),
            std::env::var("SystemRoot").unwrap()
        );
    }

    #[test]
    fn sid_conversion_and_optional_text() {
        let sid: Sid = "S-1-15-2-1".parse().unwrap();
        assert_eq!(sid.to_string(), "S-1-15-2-1");
        assert!("not a SID".parse::<Sid>().is_err());
        assert!("S-1-1-0\0suffix".parse::<Sid>().is_err());
        // SAFETY: null is accepted, and empty is a live terminated UTF-16 buffer.
        unsafe {
            assert!(Sid::from_psid(ptr::null_mut()).is_err());
            assert_eq!(optional_string(PWSTR::null()), None);
            let mut empty = [0u16];
            assert_eq!(
                optional_string(PWSTR(empty.as_mut_ptr())),
                Some(String::new())
            );
        }
        for text in [
            None,
            Some("".into()),
            Some("plain".into()),
            Some("@missing-resource.dll,-1".into()),
        ] {
            assert_eq!(resolve_indirect(text.clone()), text);
        }
    }
}
