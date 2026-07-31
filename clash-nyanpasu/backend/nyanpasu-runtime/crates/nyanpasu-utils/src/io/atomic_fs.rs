use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum AtomicFsError {
    #[error("unsafe filesystem artifact: {0}")]
    UnsafePath(PathBuf),
    #[error("directory lock is held by another process: {0}")]
    Contended(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct DirLock {
    #[cfg(unix)]
    _file: nix::fcntl::Flock<std::fs::File>,
    #[cfg(not(unix))]
    _file: std::fs::File,
}

pub fn acquire_dir_lock(path: impl AsRef<Path>) -> Result<DirLock, AtomicFsError> {
    let path = path.as_ref();
    match std::fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || is_reparse_point(&metadata) =>
        {
            return Err(AtomicFsError::UnsafePath(path.to_owned()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let mut options = std::fs::OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    let file = options.open(path).map_err(|error| {
        if dir_lock_is_contended(&error) {
            AtomicFsError::Contended(path.to_owned())
        } else {
            error.into()
        }
    })?;

    #[cfg(unix)]
    {
        let file = nix::fcntl::Flock::lock(
            file,
            nix::fcntl::FlockArg::LockExclusiveNonblock,
        )
        .map_err(|(_, errno)| {
            let error: std::io::Error = errno.into();
            if dir_lock_is_contended(&error) {
                AtomicFsError::Contended(path.to_owned())
            } else {
                error.into()
            }
        })?;
        Ok(DirLock { _file: file })
    }

    #[cfg(not(unix))]
    Ok(DirLock { _file: file })
}

#[cfg(unix)]
fn dir_lock_is_contended(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::PermissionDenied
    )
}

#[cfg(windows)]
fn dir_lock_is_contended(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(5 | 32 | 33))
}

#[cfg(not(any(unix, windows)))]
fn dir_lock_is_contended(_error: &std::io::Error) -> bool {
    false
}

pub async fn validate_absent_regular_target(
    path: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    let path = path.as_ref();
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || is_reparse_point(&metadata) =>
        {
            Err(AtomicFsError::UnsafePath(path.to_owned()))
        }
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("runtime artifact already exists: {}", path.display()),
        )
        .into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub async fn validate_existing_regular_target(
    path: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    let path = path.as_ref();
    let metadata = tokio::fs::symlink_metadata(path).await?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || is_reparse_point(&metadata) {
        return Err(AtomicFsError::UnsafePath(path.to_owned()));
    }
    Ok(())
}

pub async fn remove_regular_file(path: impl AsRef<Path>) -> Result<(), AtomicFsError> {
    let path = path.as_ref();
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || is_reparse_point(&metadata) =>
        {
            Err(AtomicFsError::UnsafePath(path.to_owned()))
        }
        Ok(_) => {
            tokio::fs::remove_file(path).await?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(windows)]
pub fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
pub fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
pub fn harden_windows_directory_acl(path: impl AsRef<Path>) -> Result<(), AtomicFsError> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::{
                Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
                DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SetFileSecurityW,
            },
        },
        core::PCWSTR,
    };

    const SDDL_REVISION_1: u32 = 1;

    let path = path.as_ref();
    let sddl: Vec<u16> = "D:P(A;OICI;FA;;;OW)(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"
        .encode_utf16()
        .chain(iter::once(0))
        .collect();
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the SDDL string is valid, NUL-terminated UTF-16 and the output
    // pointer remains owned by LocalAlloc until LocalFree below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(windows_io_error)?;
    // SAFETY: both path and descriptor are valid for the duration of the call.
    let result = unsafe {
        SetFileSecurityW(
            PCWSTR(path_wide.as_ptr()),
            DACL_SECURITY_INFORMATION,
            descriptor,
        )
    };
    // SAFETY: descriptor was allocated by the conversion API above.
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    if result.0 == 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn verify_windows_directory_acl(path: impl AsRef<Path>) -> Result<(), AtomicFsError> {
    use std::{iter, os::windows::ffi::OsStrExt};
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::{
                Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW,
                DACL_SECURITY_INFORMATION, GetFileSecurityW, GetSecurityDescriptorControl,
                PSECURITY_DESCRIPTOR, SE_DACL_PROTECTED,
            },
        },
        core::{PCWSTR, PWSTR},
    };

    const SDDL_REVISION_1: u32 = 1;

    let path = path.as_ref();
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let mut needed = 0_u32;
    // SAFETY: null output with length zero is the documented size query.
    // The BOOL is intentionally ignored because only the required size is used.
    #[allow(unused_must_use)]
    unsafe {
        let _ = GetFileSecurityW(
            PCWSTR(path_wide.as_ptr()),
            DACL_SECURITY_INFORMATION.0,
            None,
            0,
            &mut needed,
        );
    }
    if needed == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let words = (needed as usize).div_ceil(std::mem::size_of::<usize>());
    let mut descriptor = vec![0_usize; words];
    let descriptor_ptr = PSECURITY_DESCRIPTOR(descriptor.as_mut_ptr().cast());
    // SAFETY: descriptor has at least `needed` writable bytes and path is NUL-terminated.
    if unsafe {
        GetFileSecurityW(
            PCWSTR(path_wide.as_ptr()),
            DACL_SECURITY_INFORMATION.0,
            Some(descriptor_ptr),
            needed,
            &mut needed,
        )
    }
    .0 == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }

    let mut control = 0_u16;
    let mut revision = 0_u32;
    // SAFETY: descriptor contains a security descriptor returned by GetFileSecurityW.
    unsafe { GetSecurityDescriptorControl(descriptor_ptr, &mut control, &mut revision) }
        .map_err(windows_io_error)?;
    if control & SE_DACL_PROTECTED.0 == 0 {
        return Err(AtomicFsError::UnsafePath(path.to_owned()));
    }

    let mut sddl = PWSTR(std::ptr::null_mut());
    let mut sddl_len = 0_u32;
    // SAFETY: descriptor is valid; the output is LocalAlloc-owned on success.
    unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor_ptr,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut sddl,
            Some(&mut sddl_len),
        )
    }
    .map_err(windows_io_error)?;
    // SAFETY: conversion returned `sddl_len` initialized UTF-16 code units.
    let text =
        String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sddl.0, sddl_len as usize) });
    // SAFETY: sddl was allocated by the conversion API above.
    unsafe { LocalFree(Some(HLOCAL(sddl.0.cast()))) };
    let broad_principals = [";;;WD)", ";;;AU)", ";;;BU)", ";;;IU)", ";;;AN)", ";;;NU)"];
    if broad_principals
        .iter()
        .any(|principal| text.contains(principal))
    {
        return Err(AtomicFsError::UnsafePath(path.to_owned()));
    }
    Ok(())
}

#[cfg(windows)]
fn windows_io_error(error: windows::core::Error) -> std::io::Error {
    let code = error.code().0 as u32;
    if code & 0xffff_0000 == 0x8007_0000 {
        std::io::Error::from_raw_os_error((code & 0xffff) as i32)
    } else {
        std::io::Error::other(error)
    }
}

#[cfg(unix)]
pub async fn atomic_move_new(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    // Linking publishes the already-fsynced inode atomically and fails if the
    // destination appeared after validation; removing the staging name does
    // not affect readers of the committed target.
    tokio::fs::hard_link(source.as_ref(), target.as_ref()).await?;
    tokio::fs::remove_file(source.as_ref()).await?;
    Ok(())
}

#[cfg(unix)]
pub async fn atomic_replace(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    tokio::fs::rename(source.as_ref(), target.as_ref()).await?;
    Ok(())
}

#[cfg(windows)]
pub async fn atomic_move_new(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    windows_move_file(source, target, false)
}

#[cfg(windows)]
pub async fn atomic_replace(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    const RETRIES: usize = 20;
    for attempt in 0..RETRIES {
        match windows_replace_file(source.as_ref(), target.as_ref()) {
            Ok(()) => return Ok(()),
            Err(AtomicFsError::Io(error))
                if matches!(error.raw_os_error(), Some(5 | 32 | 33))
                    && attempt + 1 < RETRIES =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("bounded retry loop returns on its final attempt")
}

#[cfg(windows)]
fn windows_move_file(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
    replace: bool,
) -> Result<(), AtomicFsError> {
    use std::{iter, os::windows::ffi::OsStrExt};

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let source: Vec<u16> = source
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let target: Vec<u16> = target
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    // SAFETY: both arguments are valid, NUL-terminated UTF-16 strings for the duration of the call.
    if unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), flags) } == 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn windows_replace_file(
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> Result<(), AtomicFsError> {
    use std::{iter, os::windows::ffi::OsStrExt};

    const REPLACEFILE_WRITE_THROUGH: u32 = 0x1;
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced: *const u16,
            replacement: *const u16,
            backup: *const u16,
            flags: u32,
            exclude: *mut core::ffi::c_void,
            reserved: *mut core::ffi::c_void,
        ) -> i32;
    }
    let source: Vec<u16> = source
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    let target: Vec<u16> = target
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(iter::once(0))
        .collect();
    // SAFETY: path arguments are valid NUL-terminated UTF-16 strings; optional pointers are null.
    if unsafe {
        ReplaceFileW(
            target.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            REPLACEFILE_WRITE_THROUGH,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
pub async fn sync_dir(dir: impl AsRef<Path>) -> std::io::Result<()> {
    let dir = dir.as_ref().to_owned();
    tokio::task::spawn_blocking(move || std::fs::File::open(dir)?.sync_all())
        .await
        .map_err(std::io::Error::other)?
}

#[cfg(windows)]
pub async fn sync_dir(_dir: impl AsRef<Path>) -> std::io::Result<()> {
    // MoveFileExW requests write-through for first publication. Microsoft
    // documents ReplaceFileW's WRITE_THROUGH flag as unsupported, and std
    // cannot fsync a Windows directory, so replacement power-loss durability
    // is not asserted here.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn atomic_move_new_never_clobbers_and_publishes_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        tokio::fs::write(&source, b"new").await.unwrap();
        tokio::fs::write(&target, b"old").await.unwrap();

        let error = atomic_move_new(&source, &target).await.unwrap_err();
        assert!(matches!(
            error,
            AtomicFsError::Io(error) if error.kind() == std::io::ErrorKind::AlreadyExists
        ));
        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"old");
        assert!(source.exists());

        let target = dir.path().join("absent-target");
        atomic_move_new(&source, &target).await.unwrap();
        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"new");
        assert!(!source.exists());
    }

    #[tokio::test]
    async fn atomic_replace_overwrites_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = dir.path().join("target");
        tokio::fs::write(&source, b"new").await.unwrap();
        tokio::fs::write(&target, b"old").await.unwrap();

        atomic_replace(&source, &target).await.unwrap();

        assert_eq!(tokio::fs::read(&target).await.unwrap(), b"new");
        assert!(!source.exists());
    }

    #[tokio::test]
    async fn sync_dir_succeeds_for_real_directory() {
        let dir = tempfile::tempdir().unwrap();
        sync_dir(dir.path()).await.unwrap();
    }

    #[test]
    fn ordinary_artifacts_are_not_reparse_points() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("file");
        std::fs::write(&file, b"contents").unwrap();

        assert!(!is_reparse_point(&std::fs::metadata(dir.path()).unwrap()));
        assert!(!is_reparse_point(&std::fs::metadata(file).unwrap()));
    }

    #[test]
    fn dir_lock_reports_contention_and_releases_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let first = acquire_dir_lock(&path).unwrap();

        assert!(matches!(
            acquire_dir_lock(&path).unwrap_err(),
            AtomicFsError::Contended(contended) if contended == path
        ));

        drop(first);
        acquire_dir_lock(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_acl_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        harden_windows_directory_acl(dir.path()).unwrap();
        verify_windows_directory_acl(dir.path()).unwrap();
    }
}
