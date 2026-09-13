//! Permissions for an existing, core-owned Windows named pipe.
use anyhow::Context;
use std::{ffi::c_void, path::Path};

use windows::{
    Win32::{
        Foundation::{HANDLE, HLOCAL},
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::Pipes::GetNamedPipeServerProcessId,
    },
    core::{HSTRING, Owned, PWSTR},
};

/// An immutable policy; callers supply the authorized identities explicitly.
#[derive(Clone)]
pub struct PipeSecurity {
    sddl: String,
}

impl PipeSecurity {
    pub fn new(sids: &[&str]) -> anyhow::Result<Self> {
        // Existing HTTP transports open pipes with GENERIC_READ | GENERIC_WRITE.
        // This includes FILE_CREATE_PIPE_INSTANCE; removing it requires changing
        // the client open masks as well. Never grant WRITE_DAC to GUI clients.
        let entries: Vec<_> = sids
            .iter()
            .map(|sid| (*sid, FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0))
            .collect();
        let descriptor =
            crate::acl::generate_security_descriptor_with_permissions(&entries, None, None)?;
        let dacl = descriptor.split_once("D:").expect("generated DACL").1;
        Ok(Self {
            sddl: format!("D:P{}", dacl.replace(";;GA;", ";;FA;")),
        })
    }

    pub fn sddl(&self) -> &str {
        &self.sddl
    }

    /// Verify identity on the same handle used to inspect/change permissions.
    /// Opening is nonblocking: missing/busy pipes are retried by the caller's
    /// bounded readiness/liveness driver. A post-creation change cannot revoke
    /// handles already opened under the core's original ACL.
    pub fn authorize(
        &self,
        path: &Path,
        expected_pid: u32,
        set_permissions: bool,
    ) -> anyhow::Result<()> {
        let name = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("pipe path is not Unicode"))?;
        anyhow::ensure!(
            name.starts_with(r"\\.\pipe\"),
            "expected a local named pipe"
        );
        unsafe {
            let access = READ_CONTROL.0
                | FILE_READ_ATTRIBUTES.0
                | if set_permissions { WRITE_DAC.0 } else { 0 };
            let handle = Owned::new(
                CreateFileW(
                    &HSTRING::from(name),
                    access,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    None,
                )
                .context("open controller pipe for authorization")?,
            );
            let mut pid = 0;
            GetNamedPipeServerProcessId(*handle, &mut pid)
                .context("query controller pipe server PID")?;
            anyhow::ensure!(
                pid == expected_pid,
                "controller pipe belongs to PID {pid}, expected {expected_pid}"
            );
            let desired = Descriptor::parse(&self.sddl)?;
            let expected = entries(desired.dacl()?)?;
            let current = Descriptor::from_handle(*handle)?;
            if current.dacl().and_then(|acl| entries(acl)).ok().as_ref() != Some(&expected) {
                anyhow::ensure!(
                    set_permissions,
                    "core did not apply the requested pipe permissions"
                );
                SetSecurityInfo(
                    *handle,
                    SE_KERNEL_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    Some(desired.dacl()?),
                    None,
                )
                .ok()
                .context("set controller pipe DACL")?;
                let actual = Descriptor::from_handle(*handle)?;
                anyhow::ensure!(
                    entries(actual.dacl()?)? == expected,
                    "pipe DACL verification failed"
                );
            }
        }
        Ok(())
    }
}

struct Descriptor(Owned<HLOCAL>);
impl Descriptor {
    unsafe fn parse(sddl: &str) -> windows::core::Result<Self> {
        unsafe {
            let mut sd = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                &HSTRING::from(sddl),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )?;
            Ok(Self(Owned::new(HLOCAL(sd.0))))
        }
    }
    unsafe fn from_handle(handle: HANDLE) -> windows::core::Result<Self> {
        unsafe {
            let mut sd = PSECURITY_DESCRIPTOR::default();
            GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                Some(&mut sd),
            )
            .ok()?;
            Ok(Self(Owned::new(HLOCAL(sd.0))))
        }
    }
    unsafe fn dacl(&self) -> anyhow::Result<*mut ACL> {
        unsafe {
            let mut present = false.into();
            let mut defaulted = false.into();
            let mut acl = std::ptr::null_mut();
            GetSecurityDescriptorDacl(
                PSECURITY_DESCRIPTOR(self.0.0),
                &mut present,
                &mut acl,
                &mut defaulted,
            )?;
            anyhow::ensure!(
                present.as_bool() && !acl.is_null(),
                "pipe has an absent or NULL DACL"
            );
            Ok(acl)
        }
    }
}

/// Compare ACE semantics rather than SDDL aliases, ordering or generic masks.
unsafe fn entries(acl: *const ACL) -> anyhow::Result<Vec<(String, u32, u8)>> {
    unsafe {
        let mut result = Vec::new();
        for index in 0..(*acl).AceCount {
            let mut ptr: *mut c_void = std::ptr::null_mut();
            GetAce(acl, index.into(), &mut ptr)?;
            let ace = &*ptr.cast::<ACCESS_ALLOWED_ACE>();
            anyhow::ensure!(ace.Header.AceType == 0, "unexpected non-allow pipe ACE");
            let sid = PSID((&raw const ace.SidStart).cast_mut().cast());
            let mut string = PWSTR::null();
            ConvertSidToStringSidW(sid, &mut string)?;
            let _allocation = Owned::new(HLOCAL(string.0.cast()));
            let mut mask = ace.Mask;
            let mapping = GENERIC_MAPPING {
                GenericRead: FILE_GENERIC_READ.0,
                GenericWrite: FILE_GENERIC_WRITE.0,
                GenericExecute: FILE_GENERIC_EXECUTE.0,
                GenericAll: FILE_ALL_ACCESS.0,
            };
            MapGenericMask(&mut mask, &mapping);
            result.push((string.to_string()?, mask, ace.Header.AceFlags));
        }
        result.sort();
        result.dedup();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

    fn name(label: &str) -> String {
        format!(r"\\.\pipe\nyanpasu-security-{}-{label}", std::process::id())
    }

    async fn connection<T>(
        server: &tokio::net::windows::named_pipe::NamedPipeServer,
        action: impl FnOnce() -> T,
    ) -> T {
        let (connected, result) = tokio::join!(server.connect(), async { action() });
        connected.unwrap();
        server.disconnect().unwrap();
        result
    }

    #[tokio::test]
    async fn sets_permissions_and_rejects_wrong_pid() {
        let path = name("host");
        let user = crate::acl::get_current_user_sid_string().unwrap();
        let policy = PipeSecurity::new(&[&user]).unwrap();
        {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&path)
                .unwrap();
            assert!(
                connection(&server, || policy.authorize(
                    Path::new(&path),
                    std::process::id(),
                    false
                ))
                .await
                .is_err()
            );
            assert!(
                connection(&server, || policy.authorize(
                    Path::new(&path),
                    std::process::id() + 1,
                    true
                ))
                .await
                .is_err()
            );
            connection(&server, || {
                policy.authorize(Path::new(&path), std::process::id(), true)
            })
            .await
            .unwrap();
            connection(&server, || {
                policy.authorize(Path::new(&path), std::process::id(), false)
            })
            .await
            .unwrap();
            let client = connection(&server, || ClientOptions::new().open(&path))
                .await
                .unwrap();
            drop(client);
            drop(server);
        }
    }

    #[tokio::test]
    async fn verifies_native_creation_and_denies_unlisted_user() {
        let path = name("native");
        let user = crate::acl::get_current_user_sid_string().unwrap();
        let policy = PipeSecurity::new(&[&user]).unwrap();
        let descriptor = unsafe { Descriptor::parse(policy.sddl()).unwrap() };
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0.0,
            bInheritHandle: false.into(),
        };
        let server = unsafe {
            ServerOptions::new()
                .first_pipe_instance(true)
                .create_with_security_attributes_raw(
                    &path,
                    (&raw const attributes).cast_mut().cast(),
                )
                .unwrap()
        };
        connection(&server, || {
            policy.authorize(Path::new(&path), std::process::id(), false)
        })
        .await
        .unwrap();
        let client = connection(&server, || ClientOptions::new().open(&path))
            .await
            .unwrap();
        drop(client);
        let _available = ServerOptions::new().create(&path).unwrap();
        // A restricted token requires an additional allowed SID; the pipe does
        // not grant it. This exercises Windows' access check without elevation.
        unsafe {
            use windows::Win32::System::Threading::*;
            let mut token = HANDLE::default();
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_DUPLICATE | TOKEN_QUERY,
                &mut token,
            )
            .unwrap();
            let token = Owned::new(token);
            let mut sid = PSID::default();
            ConvertStringSidToSidW(&HSTRING::from("S-1-5-21-1-2-3-9999"), &mut sid).unwrap();
            let _sid = Owned::new(HLOCAL(sid.0));
            let restricted_sid = SID_AND_ATTRIBUTES {
                Sid: sid,
                Attributes: 0,
            };
            let mut restricted = HANDLE::default();
            CreateRestrictedToken(
                *token,
                DISABLE_MAX_PRIVILEGE,
                None,
                None,
                Some(&[restricted_sid]),
                &mut restricted,
            )
            .unwrap();
            let restricted = Owned::new(restricted);
            ImpersonateLoggedOnUser(*restricted).unwrap();
            let opened = ClientOptions::new().open(&path);
            RevertToSelf().unwrap();
            assert_eq!(opened.unwrap_err().raw_os_error(), Some(5));
        }
    }
}
