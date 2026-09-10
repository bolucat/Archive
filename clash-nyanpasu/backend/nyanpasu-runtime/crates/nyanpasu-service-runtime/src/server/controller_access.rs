use nyanpasu_core_manager::{ControllerAccess, Host};

pub struct UnavailableControllerAccess;
impl ControllerAccess for UnavailableControllerAccess {
    fn supports_local_ipc(&self) -> bool {
        false
    }
    fn authorize(&self, host: &Host) -> std::io::Result<()> {
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
}

#[cfg(unix)]
impl UnixControllerAccess {
    pub fn prepare(root: &std::path::Path, gid: u32) -> std::io::Result<Self> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
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
        std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o750))?;
        Ok(Self {
            root: root.canonicalize()?,
            gid,
            uid,
        })
    }
}

#[cfg(unix)]
impl ControllerAccess for UnixControllerAccess {
    fn supports_local_ipc(&self) -> bool {
        true
    }
    fn authorize(&self, host: &Host) -> std::io::Result<()> {
        use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
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
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o660))
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
        let access = UnixControllerAccess::prepare(&dir, gid).unwrap();
        let dir = dir.canonicalize().unwrap();
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o750
        );
        let socket = dir.join("core-1.sock");
        for _ in 0..2 {
            let listener = UnixListener::bind(&socket).unwrap();
            access.authorize(&Host::unix_socket(&socket)).unwrap();
            let metadata = std::fs::metadata(&socket).unwrap();
            assert_eq!(metadata.mode() & 0o777, 0o660);
            assert_eq!(metadata.gid(), gid);
            drop(listener);
            std::fs::remove_file(&socket).unwrap();
        }
        let file = dir.join("config.yaml");
        std::fs::write(&file, "secret: private").unwrap();
        assert!(access.authorize(&Host::unix_socket(&file)).is_err());
        let link = dir.join("link.sock");
        symlink(&file, &link).unwrap();
        assert!(access.authorize(&Host::unix_socket(&link)).is_err());
        assert!(
            access
                .authorize(&Host::unix_socket(temp.path().join("outside.sock")))
                .is_err()
        );
    }

    #[test]
    fn refuses_a_writable_or_symlinked_controller_directory() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("controllers");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(UnixControllerAccess::prepare(&dir, unsafe { libc::getegid() }).is_err());
        let link = temp.path().join("link");
        symlink(&dir, &link).unwrap();
        assert!(UnixControllerAccess::prepare(&link, unsafe { libc::getegid() }).is_err());
    }
}
