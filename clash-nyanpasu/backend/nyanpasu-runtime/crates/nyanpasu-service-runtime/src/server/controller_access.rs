use nyanpasu_core_manager::{ControllerAccess, ControllerAuthorization, Host};

#[cfg(any(test, not(windows)))]
pub struct UnavailableControllerAccess;
#[cfg(any(test, not(windows)))]
impl ControllerAccess for UnavailableControllerAccess {
    fn supports_local_ipc(&self) -> bool {
        false
    }
    fn authorize(
        &self,
        host: &Host,
        _pid: u32,
        _mode: ControllerAuthorization,
    ) -> std::io::Result<()> {
        if matches!(host, Host::Http(_)) {
            Ok(())
        } else {
            Err(std::io::Error::other(
                "core IPC authorization is unavailable on this host",
            ))
        }
    }
}

#[cfg(unix)]
pub struct UnixControllerAccess {
    root: std::path::PathBuf,
    gid: u32,
    uid: u32,
    #[cfg(target_os = "linux")]
    desktop_uid: Option<u32>,
}

#[cfg(unix)]
impl UnixControllerAccess {
    pub fn prepare(
        root: &std::path::Path,
        gid: u32,
        #[cfg(target_os = "linux")] desktop_uid: Option<u32>,
    ) -> std::io::Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        let uid = unsafe { libc::geteuid() };
        match std::fs::DirBuilder::new().mode(0o700).create(root) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = std::fs::symlink_metadata(root)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || metadata.uid() != uid
            || metadata.mode() & 0o022 != 0
        {
            return Err(std::io::Error::other("unsafe controller directory"));
        }
        std::os::unix::fs::chown(root, Some(uid), Some(gid))?;
        let access = Self {
            root: root.canonicalize()?,
            gid,
            uid,
            #[cfg(target_os = "linux")]
            desktop_uid: desktop_uid.filter(|&desktop_uid| desktop_uid != uid),
        };
        access.set_access(&access.root, 0o750, 0o1)?;
        Ok(access)
    }

    fn set_access(
        &self,
        path: &std::path::Path,
        mode: u32,
        desktop_permissions: u16,
    ) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        #[cfg(target_os = "linux")]
        match set_linux_access_acl(path, mode, self.desktop_uid, desktop_permissions) {
            // An unbound installation only needs the original mode bits.
            Err(error)
                if self.desktop_uid.is_none() && error.raw_os_error() == Some(libc::EOPNOTSUPP) => {
            }
            result => return result,
        }
        #[cfg(not(target_os = "linux"))]
        let _ = desktop_permissions;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    }
}

#[cfg(target_os = "linux")]
fn set_linux_access_acl(
    path: &std::path::Path,
    mode: u32,
    desktop_uid: Option<u32>,
    desktop_permissions: u16,
) -> std::io::Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    // Linux POSIX ACL xattr v2: little-endian header and ordered entries.
    const USER_OBJ: u16 = 0x01;
    const USER: u16 = 0x02;
    const GROUP_OBJ: u16 = 0x04;
    const MASK: u16 = 0x10;
    const OTHER: u16 = 0x20;
    const UNDEFINED_ID: u32 = u32::MAX;
    let owner_permissions = ((mode >> 6) & 7) as u16;
    let group_permissions = ((mode >> 3) & 7) as u16;
    let mut acl = Vec::with_capacity(44);
    acl.extend_from_slice(&2u32.to_le_bytes());
    let mut entry = |tag: u16, permissions: u16, id: u32| {
        acl.extend_from_slice(&tag.to_le_bytes());
        acl.extend_from_slice(&permissions.to_le_bytes());
        acl.extend_from_slice(&id.to_le_bytes());
    };
    entry(USER_OBJ, owner_permissions, UNDEFINED_ID);
    if let Some(uid) = desktop_uid {
        entry(USER, desktop_permissions, uid);
    }
    entry(GROUP_OBJ, group_permissions, UNDEFINED_ID);
    if desktop_uid.is_some() {
        entry(MASK, group_permissions | desktop_permissions, UNDEFINED_ID);
    }
    entry(OTHER, (mode & 7) as u16, UNDEFINED_ID);

    let path = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: the path and attribute name are NUL-terminated; the value
    // points to `acl.len()` initialized bytes. Symlinks are not followed.
    let result = unsafe {
        libc::lsetxattr(
            path.as_ptr(),
            c"system.posix_acl_access".as_ptr(),
            acl.as_ptr().cast(),
            acl.len(),
            0,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
impl ControllerAccess for UnixControllerAccess {
    fn supports_local_ipc(&self) -> bool {
        true
    }
    fn authorize(
        &self,
        host: &Host,
        _pid: u32,
        _mode: ControllerAuthorization,
    ) -> std::io::Result<()> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let Host::UnixSocket(path) = host else {
            return Ok(());
        };
        if path.parent() != Some(self.root.as_path()) {
            return Err(std::io::Error::other("controller escaped its directory"));
        }
        let metadata = std::fs::symlink_metadata(path)?;
        if !metadata.file_type().is_socket() || metadata.uid() != self.uid {
            return Err(std::io::Error::other(
                "unexpected controller owner or file type",
            ));
        }
        std::os::unix::fs::chown(path, Some(self.uid), Some(self.gid))?;
        self.set_access(path, 0o660, 0o6)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::{
        fs::{MetadataExt, PermissionsExt, symlink},
        net::UnixListener,
    };

    #[test]
    fn authorizes_each_recreated_socket_without_exposing_runtime_files() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("controllers");
        let gid = unsafe { libc::getegid() };
        let access = UnixControllerAccess::prepare(
            &dir,
            gid,
            #[cfg(target_os = "linux")]
            None,
        )
        .unwrap();
        let dir = dir.canonicalize().unwrap();
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o750
        );
        let socket = dir.join("core-1.sock");
        for _ in 0..2 {
            let listener = UnixListener::bind(&socket).unwrap();
            access
                .authorize(
                    &Host::unix_socket(&socket),
                    0,
                    ControllerAuthorization::Host,
                )
                .unwrap();
            let metadata = std::fs::metadata(&socket).unwrap();
            assert_eq!(metadata.mode() & 0o777, 0o660);
            assert_eq!(metadata.gid(), gid);
            drop(listener);
            std::fs::remove_file(&socket).unwrap();
        }
        let file = dir.join("config.yaml");
        std::fs::write(&file, "secret: private").unwrap();
        assert!(
            access
                .authorize(&Host::unix_socket(&file), 0, ControllerAuthorization::Host)
                .is_err()
        );
        let link = dir.join("link.sock");
        symlink(&file, &link).unwrap();
        assert!(
            access
                .authorize(&Host::unix_socket(&link), 0, ControllerAuthorization::Host)
                .is_err()
        );
        assert!(
            access
                .authorize(
                    &Host::unix_socket(temp.path().join("outside.sock")),
                    0,
                    ControllerAuthorization::Host
                )
                .is_err()
        );
    }

    #[test]
    fn refuses_a_writable_or_symlinked_controller_directory() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("controllers");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        let prepare = |path: &std::path::Path| {
            UnixControllerAccess::prepare(
                path,
                unsafe { libc::getegid() },
                #[cfg(target_os = "linux")]
                None,
            )
        };
        assert!(prepare(&dir).is_err());
        let link = temp.path().join("link");
        symlink(&dir, &link).unwrap();
        assert!(prepare(&link).is_err());
    }
}

#[cfg(windows)]
pub struct WindowsControllerAccess {
    security: nyanpasu_windows_security::pipe::PipeSecurity,
}

#[cfg(windows)]
impl WindowsControllerAccess {
    pub fn new(sids: &[&str]) -> anyhow::Result<Self> {
        Ok(Self {
            security: nyanpasu_windows_security::pipe::PipeSecurity::new(sids)?,
        })
    }
}

#[cfg(windows)]
impl ControllerAccess for WindowsControllerAccess {
    fn supports_local_ipc(&self) -> bool {
        true
    }
    fn pipe_security_descriptor(&self) -> Option<&str> {
        Some(self.security.sddl())
    }
    fn authorize(
        &self,
        host: &Host,
        pid: u32,
        mode: ControllerAuthorization,
    ) -> std::io::Result<()> {
        match host {
            Host::NamedPipe(path) => self
                .security
                .authorize(path, pid, mode == ControllerAuthorization::Host)
                .map_err(std::io::Error::other),
            Host::Http(_) => Ok(()),
            _ => Err(std::io::Error::other("unsupported controller transport")),
        }
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn host_wiring_uses_the_installation_sid_policy() {
        let user = nyanpasu_windows_security::acl::get_current_user_sid_string().unwrap();
        let (directory, access) = super::super::controller_access_for_host(&[&user]).unwrap();
        assert!(directory.is_none());
        assert!(access.supports_local_ipc());
        let expected = nyanpasu_windows_security::pipe::PipeSecurity::new(&[&user]).unwrap();
        assert_eq!(access.pipe_security_descriptor(), Some(expected.sddl()));
        assert!(super::super::controller_access_for_host(&["not-a-sid"]).is_err());
        access
            .authorize(
                &Host::http("http://127.0.0.1:9090").unwrap(),
                0,
                ControllerAuthorization::Host,
            )
            .unwrap();
    }
}
