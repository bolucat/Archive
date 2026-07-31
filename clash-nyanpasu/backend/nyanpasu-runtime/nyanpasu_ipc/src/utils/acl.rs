#![allow(unused_variables)]
use std::{
    ffi::c_void,
    ops::{Deref, DerefMut},
};

use anyhow::{Context, Result};
use windows::{
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        System::{SystemServices::*, Threading::*},
    },
    core::*,
};

/// Administrators
pub const ADMINISTRATORS_GROUP_SID: &str = "S-1-5-32-544";
/// SYSTEM
pub const SYSTEM_SID: &str = "S-1-5-18";
/// AUTHENTICATED_USER
pub const AUTHENTICATED_USER_SID: &str = "S-1-5-11";
/// EVERYONE, aka World
pub const EVERYONE_SID: &str = "S-1-1-0";

struct OwnedPSID(PSID);

impl AsRef<PSID> for OwnedPSID {
    fn as_ref(&self) -> &PSID {
        &self.0
    }
}

impl Deref for OwnedPSID {
    type Target = PSID;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OwnedPSID {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for OwnedPSID {
    fn drop(&mut self) {
        let _ = unsafe { Owned::new(HLOCAL(self.0.0)) };
    }
}

impl OwnedPSID {
    /// Create OwnedPSID from PSID
    unsafe fn from_psid(psid: PSID) -> Self {
        Self(psid)
    }

    /// Create OwnedPSID from SID string
    unsafe fn try_from_sid_str(sid_str: &str) -> windows::core::Result<Self> {
        let sid_hstring = HSTRING::from(sid_str);
        let mut psid = PSID::default();
        unsafe {
            ConvertStringSidToSidW(&sid_hstring, &mut psid)?;
            Ok(Self::from_psid(psid))
        }
    }
}

/// Internal common function: generate security descriptor with permissions
///
/// # Parameters
/// * `sids_and_permissions` - Array of SID strings and permission pairs
/// * `owner` - Optional owner SID
/// * `group` - Optional group SID
///
/// # Returns
/// Returns SDDL string
#[cfg(windows)]
fn generate_security_descriptor_internal<T: AsRef<str>>(
    sids_and_permissions: &[(&T, Option<u32>)],
    owner: Option<&str>,
    group: Option<&str>,
) -> Result<String> {
    unsafe {
        let mut security_descriptor = SECURITY_DESCRIPTOR::default();
        let security_descriptor_ptr =
            PSECURITY_DESCRIPTOR(&mut security_descriptor as *mut _ as *mut c_void);
        InitializeSecurityDescriptor(security_descriptor_ptr, SECURITY_DESCRIPTOR_REVISION)
            .context("failed to initialize security descriptor")?;

        let system_sid = OwnedPSID::try_from_sid_str(SYSTEM_SID)
            .context("failed to convert system sid string to psid")?;
        let admins_sid = OwnedPSID::try_from_sid_str(ADMINISTRATORS_GROUP_SID)
            .context("failed to convert admins sid string to psid")?;

        let owner_sid = match owner {
            Some(s) => OwnedPSID::try_from_sid_str(s)
                .context("failed to convert owner sid string to psid")?,
            None => admins_sid,
        };
        let group_sid = match group {
            Some(s) => OwnedPSID::try_from_sid_str(s)
                .context("failed to convert group sid string to psid")?,
            None => system_sid,
        };

        let sids = sids_and_permissions
            .iter()
            .map(|(sid, permissions)| {
                let owned_sid = OwnedPSID::try_from_sid_str(sid.as_ref()).context(format!(
                    "failed to convert sid string to psid: {}",
                    sid.as_ref()
                ))?;
                Ok((owned_sid, *permissions))
            })
            .collect::<Result<Vec<_>>>()?;

        let mut psids = Vec::with_capacity(sids.len() + 2);
        psids.push((*owner_sid.as_ref(), None));
        psids.push((*group_sid.as_ref(), None));
        psids.extend(
            sids.iter()
                .map(|(sid, permissions)| (*sid.as_ref(), *permissions)),
        );

        // Calculate ACL size
        let acl_size = calculate_acl_size(psids.iter().map(|(sid, _)| sid).collect::<Vec<_>>())
            .context("failed to calculate acl size")?;

        // Create ACL
        let mut acl_buffer = vec![0u8; acl_size + 256];
        let acl = acl_buffer.as_mut_ptr() as *mut ACL;
        InitializeAcl(acl, (acl_size + 256) as u32, ACL_REVISION)
            .context("failed to initialize acl")?;

        // Add SIDs to ACL
        add_sids_to_acl(&mut *acl, psids).context("failed to add sids to acl")?;

        // Set DACL
        SetSecurityDescriptorDacl(security_descriptor_ptr, true, Some(acl), false)
            .context("failed to set dacl to security descriptor")?;

        // Set owner and group
        set_owner_and_group(
            &mut security_descriptor,
            *owner_sid.as_ref(),
            *group_sid.as_ref(),
        )
        .context("failed to set owner and group")?;

        // Convert to SDDL string
        let mut sddl_string = PWSTR::default();
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            security_descriptor_ptr,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION,
            &mut sddl_string,
            None,
        )
        .context("failed to convert security descriptor to sddl string")?;

        sddl_string
            .to_string()
            .context("failed to convert sddl string to string")
    }
}

/// Generate security descriptor using Windows native API
///
/// # Parameters
/// * `sids` - Array of SID strings
/// * `owner` - Optional owner SID
/// * `group` - Optional group SID
///
/// # Returns
/// Returns SDDL string
#[cfg(windows)]
pub fn generate_windows_security_descriptor<T: AsRef<str> + core::fmt::Debug>(
    sids: &[T],
    owner: Option<&str>,
    group: Option<&str>,
) -> Result<String> {
    let sids_and_permissions: Vec<(&T, Option<u32>)> = sids.iter().map(|sid| (sid, None)).collect();

    generate_security_descriptor_internal(&sids_and_permissions, owner, group)
}

/// Calculate the buffer size required for the ACL
fn calculate_acl_size<'a, I>(sids: I) -> Result<usize>
where
    I: IntoIterator<Item = &'a PSID>,
{
    let mut total_size = std::mem::size_of::<ACL>();

    // Reserve space for user SIDs
    for sid in sids {
        total_size += calculate_ace_size(*sid).context("failed to calculate ace size")?;
    }

    // Add some extra buffer just in case
    Ok(total_size + 256)
}

/// Calculate the size of a single ACE
fn calculate_ace_size(sid: PSID) -> Result<usize> {
    unsafe {
        let sid_length = GetLengthSid(sid) as usize;
        // Size of ACCESS_ALLOWED_ACE structure + size of SID - 4 (because the structure already contains 4 bytes for SidStart)
        Ok(std::mem::size_of::<ACCESS_ALLOWED_ACE>() + sid_length - 4)
    }
}

struct AddSidToAcl {
    sid: PSID,
    permissions: Option<u32>,
}

impl From<(PSID, Option<u32>)> for AddSidToAcl {
    fn from((sid, permissions): (PSID, Option<u32>)) -> Self {
        Self { sid, permissions }
    }
}

impl From<&(PSID, Option<u32>)> for AddSidToAcl {
    fn from((sid, permissions): &(PSID, Option<u32>)) -> Self {
        Self {
            sid: *sid,
            permissions: *permissions,
        }
    }
}

impl From<PSID> for AddSidToAcl {
    fn from(sid: PSID) -> Self {
        Self {
            sid,
            permissions: None,
        }
    }
}

impl From<&PSID> for AddSidToAcl {
    fn from(sid: &PSID) -> Self {
        Self {
            sid: *sid,
            permissions: None,
        }
    }
}

/// Add well-known SIDs to the ACL
fn add_sids_to_acl<I, T>(acl: &mut ACL, sids: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<AddSidToAcl>,
{
    unsafe {
        for sid in sids {
            let AddSidToAcl { sid, permissions } = sid.into();
            AddAccessAllowedAce(acl, ACL_REVISION, permissions.unwrap_or(GENERIC_ALL.0), sid)
                .context("failed to add access allowed ace")?;
        }

        Ok(())
    }
}

/// Set the owner and group of the security descriptor
unsafe fn set_owner_and_group(
    security_descriptor: &mut SECURITY_DESCRIPTOR,
    group: PSID,
    owner: PSID,
) -> Result<()> {
    unsafe {
        let security_descriptor_ptr =
            PSECURITY_DESCRIPTOR(security_descriptor as *mut _ as *mut c_void);

        SetSecurityDescriptorOwner(security_descriptor_ptr, Some(owner), false)
            .context("failed to set owner")?;

        SetSecurityDescriptorGroup(security_descriptor_ptr, Some(group), false)
            .context("failed to set group")?;

        // Note: Do not free the SIDs here, as the security descriptor is still using them.
        // They will be freed automatically when the security descriptor is destroyed.

        Ok(())
    }
}

/// Generate a security descriptor with specific permissions using the Windows API
pub fn generate_security_descriptor_with_permissions<T: AsRef<str>>(
    sids_and_permissions: &[(T, u32)],
    owner: Option<&str>,
    group: Option<&str>,
) -> Result<String> {
    let sids_and_permissions_with_perms: Vec<(&T, Option<u32>)> = sids_and_permissions
        .iter()
        .map(|(sid, permissions)| (sid, Some(*permissions)))
        .collect();

    generate_security_descriptor_internal(&sids_and_permissions_with_perms, owner, group)
}

/// Get the SID string for the current user
pub fn get_current_user_sid_string() -> Result<String> {
    unsafe {
        let mut token_handle = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token_handle)
            .context("failed to open process token")?;

        let mut token_info_length = 0u32;
        let _ = GetTokenInformation(token_handle, TokenUser, None, 0, &mut token_info_length);

        let mut token_user_buffer = vec![0u8; token_info_length as usize];
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(token_user_buffer.as_mut_ptr() as *mut _),
            token_info_length,
            &mut token_info_length,
        )
        .context("failed to get token information")?;

        let token_user = &*(token_user_buffer.as_ptr() as *const TOKEN_USER);
        let user_sid = token_user.User.Sid;

        let mut sid_string = PWSTR::default();
        ConvertSidToStringSidW(user_sid, &mut sid_string)
            .context("failed to convert sid to string")?;
        sid_string
            .to_string()
            .context("failed to convert sid string to string")
    }
}

/// Create a default security descriptor for a named pipe
pub fn create_default_pipe_security_descriptor() -> Result<String> {
    let current_user_sid =
        get_current_user_sid_string().context("failed to get current user sid")?;
    let sids = vec![current_user_sid];
    generate_windows_security_descriptor(&sids, None, None)
}

#[cfg(test)]
mod tests {
    use windows::Win32::Storage::FileSystem::*;

    use super::*;

    #[test]
    fn test_generate_with_api() {
        let sids = vec!["S-1-1-0".to_string(), "S-1-5-11".to_string()];
        let result = generate_windows_security_descriptor(&sids, None, None).unwrap();

        assert!(result.starts_with("O:") || result.starts_with("D:"));
        assert_eq!(
            result,
            "O:SYG:BAD:(A;;GA;;;BA)(A;;GA;;;SY)(A;;GA;;;WD)(A;;GA;;;AU)"
        );
        println!("Generated SDDL: {result}");
    }

    #[test]
    fn test_permissions_api() {
        let sids_and_perms = vec![
            ("S-1-1-0".to_string(), FILE_GENERIC_READ.0),
            (
                "S-1-5-11".to_string(),
                FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            ),
        ];

        let result =
            generate_security_descriptor_with_permissions(&sids_and_perms, None, None).unwrap();
        assert_eq!(
            result,
            "O:SYG:BAD:(A;;GA;;;BA)(A;;GA;;;SY)(A;;FR;;;WD)(A;;0x12019f;;;AU)"
        );
        println!("SDDL with custom permissions: {result}");
    }

    #[test]
    fn test_calculate_acl_size() {
        let sids = unsafe {
            vec![
                OwnedPSID::try_from_sid_str("S-1-1-0").unwrap(),
                OwnedPSID::try_from_sid_str("S-1-5-11").unwrap(),
            ]
        };
        let size = calculate_acl_size(sids.iter().map(|s| s.as_ref())).unwrap();
        assert!(size > 0);
        println!("Calculated ACL size: {size} bytes");
    }

    #[test]
    fn test_default_pipe_security() {
        let current_user_sid = get_current_user_sid_string().unwrap();
        assert!(!current_user_sid.is_empty() && current_user_sid.starts_with("S-1-5-"));
        let result = create_default_pipe_security_descriptor().unwrap();
        assert!(!result.is_empty());
        // Well-known SIDs are rendered as aliases in the SDDL string (e.g.
        // SYSTEM, which GitHub Actions runners execute as, becomes "SY"), so
        // assert against the parsed security descriptor instead of the string.
        unsafe {
            let user_sid = OwnedPSID::try_from_sid_str(&current_user_sid).unwrap();
            let sddl = HSTRING::from(result.as_str());
            let mut sd = PSECURITY_DESCRIPTOR::default();
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut sd,
                None,
            )
            .expect("generated SDDL should parse back");
            let mut has_dacl = BOOL::default();
            let mut dacl: *mut ACL = std::ptr::null_mut();
            let mut dacl_defaulted = BOOL::default();
            GetSecurityDescriptorDacl(sd, &mut has_dacl, &mut dacl, &mut dacl_defaulted)
                .expect("failed to query the DACL");
            assert!(has_dacl.as_bool() && !dacl.is_null());
            let user_in_dacl = (0..(*dacl).AceCount).any(|index| {
                let mut ace: *mut c_void = std::ptr::null_mut();
                GetAce(dacl, u32::from(index), &mut ace).expect("failed to get ACE");
                let ace = &*(ace as *const ACCESS_ALLOWED_ACE);
                let ace_sid = PSID(&raw const ace.SidStart as *mut c_void);
                EqualSid(ace_sid, user_sid.0).is_ok()
            });
            let _ = Owned::new(HLOCAL(sd.0));
            assert!(
                user_in_dacl,
                "the DACL should grant access to the current user"
            );
        }
        println!("Default pipe SDDL: {result}");
    }
}
