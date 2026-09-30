//! The desktop identity is supplied during privileged installation or update,
//! never inferred from the daemon's UID or environment.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::PathBuf,
};

use anyhow::ensure;
use serde::{Deserialize, Serialize};

const FILE: &str = "native-store-owner.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnixOwner {
    pub uid: u32,
    pub gid: u32,
}

#[derive(Serialize, Deserialize)]
struct Binding {
    data_dir: PathBuf,
    owner: UnixOwner,
}

pub struct FsOwnerBindingStore {
    config_dir: PathBuf,
    data_dir: PathBuf,
}

impl FsOwnerBindingStore {
    pub fn new(config_dir: impl Into<PathBuf>, data_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
            data_dir: data_dir.into(),
        }
    }

    pub fn save(&self, user: &str) -> anyhow::Result<()> {
        let owner = Self::resolve_owner(user)?;
        let data_dir = dunce::canonicalize(&self.data_dir)?;
        std::fs::create_dir_all(&self.config_dir)?;
        self.verify_directory()?;
        let mut file = tempfile::NamedTempFile::new_in(&self.config_dir)?;
        file.write_all(&serde_json::to_vec(&Binding { data_dir, owner })?)?;
        file.as_file().sync_all()?;
        file.persist(self.config_dir.join(FILE))
            .map_err(|error| error.error)?;
        File::open(&self.config_dir)?.sync_all()?;
        Ok(())
    }

    pub fn load(&self) -> anyhow::Result<Option<UnixOwner>> {
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(self.config_dir.join(FILE))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        self.verify_directory()?;
        let meta = file.metadata()?;
        ensure!(
            meta.is_file() && meta.uid() == 0 && meta.mode() & 0o022 == 0 && meta.nlink() == 1,
            "native store owner binding must be a root-owned regular file, not writable by other users"
        );
        ensure!(
            meta.len() <= 16384,
            "native store owner binding is too large"
        );
        let binding: Binding = serde_json::from_reader(file.take(16384))?;
        ensure!(
            binding.data_dir == dunce::canonicalize(&self.data_dir)?,
            "native store owner binding belongs to another data directory; reinstall the service"
        );
        Ok(Some(binding.owner))
    }

    fn verify_directory(&self) -> anyhow::Result<()> {
        let meta = std::fs::symlink_metadata(&self.config_dir)?;
        ensure!(
            meta.is_dir() && meta.uid() == 0 && meta.mode() & 0o022 == 0,
            "service configuration directory must be root-owned and not writable by other users"
        );
        Ok(())
    }

    fn resolve_owner(user: &str) -> anyhow::Result<UnixOwner> {
        let name = std::ffi::CString::new(user)?;
        let mut buffer = vec![0u8; 65536];
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        let code = unsafe {
            libc::getpwnam_r(
                name.as_ptr(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if code != 0 {
            return Err(std::io::Error::from_raw_os_error(code).into());
        }
        ensure!(
            !result.is_null(),
            "native store owner does not exist: {user}"
        );
        let entry = unsafe { entry.assume_init() };
        Ok(UnixOwner {
            uid: entry.pw_uid,
            gid: entry.pw_gid,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn missing_binding_never_uses_the_daemon_identity() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsOwnerBindingStore::new(dir.path(), dir.path());
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn resolves_the_explicit_user() {
        assert_eq!(
            FsOwnerBindingStore::resolve_owner("root").unwrap(),
            UnixOwner { uid: 0, gid: 0 }
        );
        assert!(FsOwnerBindingStore::resolve_owner("root\0other").is_err());
    }

    #[test]
    fn an_unknown_user_does_not_create_a_binding() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let store = FsOwnerBindingStore::new(&config, dir.path());
        assert!(store.save("nyanpasu-nonexistent-5326").is_err());
        assert!(!config.exists());
    }

    #[test]
    fn a_dangling_symlink_is_not_a_missing_binding() {
        let dir = tempfile::tempdir().unwrap();
        symlink(dir.path().join("missing"), dir.path().join(FILE)).unwrap();
        let store = FsOwnerBindingStore::new(dir.path(), dir.path());
        assert!(store.load().is_err());
    }

    #[test]
    fn rejects_a_writable_configuration_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), b"{}").unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
        let store = FsOwnerBindingStore::new(dir.path(), dir.path());
        let error = store.load().unwrap_err().to_string();
        assert!(error.contains("service configuration directory"), "{error}");
    }

    #[test]
    #[ignore = "requires UID 0; can run inside a private user namespace"]
    fn privileged_binding_round_trip_and_path_validation() {
        assert_eq!(unsafe { libc::geteuid() }, 0);
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let data = dir.path().join("data");
        std::fs::create_dir(&data).unwrap();
        let store = FsOwnerBindingStore::new(&config, &data);
        let owner = FsOwnerBindingStore::resolve_owner("nobody").unwrap();
        assert_ne!(owner.uid, 0);
        store.save("nobody").unwrap();
        assert_eq!(store.load().unwrap(), Some(owner));
        let meta = std::fs::metadata(config.join(FILE)).unwrap();
        assert_eq!(meta.uid(), 0);
        assert_eq!(meta.mode() & 0o777, 0o600);

        let alias = dir.path().join("alias");
        symlink(&data, &alias).unwrap();
        assert_eq!(
            FsOwnerBindingStore::new(&config, alias).load().unwrap(),
            Some(owner)
        );
        assert!(
            FsOwnerBindingStore::new(&config, dir.path())
                .load()
                .is_err()
        );

        store.save("root").unwrap();
        assert_eq!(store.load().unwrap(), Some(UnixOwner { uid: 0, gid: 0 }));
    }

    #[test]
    #[ignore = "requires UID 0; can run inside a private user namespace"]
    fn privileged_binding_rejects_unsafe_files_and_directories() {
        assert_eq!(unsafe { libc::geteuid() }, 0);
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let store = FsOwnerBindingStore::new(&config, dir.path());
        store.save("root").unwrap();
        let binding = config.join(FILE);
        std::fs::set_permissions(&binding, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(store.load().is_err());
        store.save("root").unwrap();
        let link = dir.path().join("link");
        std::fs::hard_link(&binding, &link).unwrap();
        assert!(store.load().is_err());
        std::fs::remove_file(&binding).unwrap();
        symlink(&link, &binding).unwrap();
        assert!(store.load().is_err());
        let config_alias = dir.path().join("config-alias");
        symlink(&config, &config_alias).unwrap();
        assert!(
            FsOwnerBindingStore::new(config_alias, dir.path())
                .save("root")
                .is_err()
        );
    }
}
