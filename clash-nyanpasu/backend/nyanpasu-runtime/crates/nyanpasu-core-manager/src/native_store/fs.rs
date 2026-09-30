use super::{NativeStore, StoreError, StoreLease};
use crate::CoreKind;
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, Write},
    sync::Arc,
};

#[derive(Debug, Clone, Copy)]
pub enum StoreOwner {
    #[cfg(unix)]
    Unix {
        uid: u32,
        gid: u32,
    },
    #[cfg(windows)]
    Inherited,
    Missing,
}

impl StoreOwner {
    pub fn current() -> Self {
        #[cfg(unix)]
        {
            Self::Unix {
                uid: unsafe { libc::geteuid() },
                gid: unsafe { libc::getegid() },
            }
        }
        #[cfg(windows)]
        {
            Self::Inherited
        }
    }

    fn authorize(
        &self,
        _file: &File,
        _path: &Utf8Path,
        _directory: bool,
    ) -> Result<(), StoreError> {
        match self {
            Self::Missing => Err(StoreError::OwnerMissing),
            #[cfg(unix)]
            Self::Unix { uid, gid } => {
                use std::os::{
                    fd::AsRawFd,
                    unix::fs::{MetadataExt, PermissionsExt},
                };
                let meta = _file.metadata()?;
                if meta.uid() != *uid && meta.uid() != 0 {
                    return Err(StoreError::Unsafe(_path.into()));
                }
                if meta.uid() != *uid || meta.gid() != *gid {
                    // Ownership is applied to the opened inode, never a followed path.
                    if unsafe { libc::fchown(_file.as_raw_fd(), *uid, *gid) } != 0 {
                        return Err(std::io::Error::last_os_error().into());
                    }
                }
                let permissions =
                    std::fs::Permissions::from_mode(if _directory { 0o700 } else { 0o600 });
                if let Err(error) = _file.set_permissions(permissions.clone()) {
                    #[cfg(target_os = "linux")]
                    if error.raw_os_error() == Some(libc::EBADF) {
                        // O_PATH pins even a mode-000 inode. This is the kernel's
                        // descriptor link, not an attacker-controlled pathname.
                        std::fs::set_permissions(
                            format!("/proc/self/fd/{}", _file.as_raw_fd()),
                            permissions,
                        )?;
                        return Ok(());
                    }
                    return Err(error.into());
                }
                Ok(())
            }
            #[cfg(windows)]
            Self::Inherited => Ok(()),
        }
    }
}

/// The host provides the data root and the PRE-migration core identity. Native
/// bbolt headers cannot distinguish Mihomo and Premium by themselves.
pub struct FsNativeStore {
    data: Utf8PathBuf,
    owner: StoreOwner,
    legacy_kind: Option<CoreKind>,
}

impl FsNativeStore {
    pub fn new(data: Utf8PathBuf, owner: StoreOwner, legacy_kind: Option<CoreKind>) -> Self {
        Self {
            data,
            owner,
            legacy_kind,
        }
    }

    fn directories(&self, kind: CoreKind) -> Result<(Directory, Directory, Directory), StoreError> {
        if matches!(self.owner, StoreOwner::Missing) {
            return Err(StoreError::OwnerMissing);
        }
        let data = Directory::open(&self.data)?;
        let root = data.child("native-store", &self.owner)?;
        let version = root.child("v1", &self.owner)?;
        let home = version.child(kind.as_ref(), &self.owner)?;
        Ok((data, root, home))
    }
}

impl NativeStore for FsNativeStore {
    fn data_dir(&self) -> &Utf8Path {
        &self.data
    }

    fn acquire(&self, kind: CoreKind) -> Result<Arc<dyn StoreLease>, StoreError> {
        let (data, root, home) = self.directories(kind)?;
        // One application core can own native data at a time, across both hosts.
        let lock = root.file("lease.lock", &self.owner, true)?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => StoreError::Busy(root.path.clone()),
            std::fs::TryLockError::Error(error) => StoreError::Access(error),
        })?;
        if root.exists("active")? {
            let mut raw = String::new();
            root.read("active")?.take(16384).read_to_string(&mut raw)?;
            match nyanpasu_utils::process::recorded_process_is_alive(&raw) {
                Ok(false) => root.remove("active")?,
                Ok(true) => return Err(StoreError::Busy(root.path.join("active"))),
                Err(_) => return Err(StoreError::Recovery(root.path.join("active"))),
            }
        }
        home.repair_tree(&self.owner)?;
        if kind == CoreKind::Meow {
            self.migrate_meow(&data, &root, &home)?;
        } else {
            self.migrate(&data, &root, &home)?;
        }
        seed_resources(&data, &home, &self.owner)?;
        let lease = FsLease {
            root,
            home,
            owner: self.owner,
            kind,
            _lock: lock,
        };
        lease.check_cache()?;
        Ok(Arc::new(lease))
    }

    fn check_home(&self) -> Result<tempfile::TempDir, StoreError> {
        // Checks do not open/repair the real database, acquire its lease, or
        // create a native-store directory. Fake-IP parsing may itself open a DB.
        let temporary = tempfile::Builder::new()
            .prefix("nyanpasu-core-check-")
            .tempdir()?;
        let home = Directory::open(
            Utf8Path::from_path(temporary.path())
                .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?,
        )?;
        seed_resources(&Directory::open(&self.data)?, &home, &StoreOwner::current())?;
        Ok(temporary)
    }
}

impl FsNativeStore {
    fn migrate_meow(
        &self,
        data: &Directory,
        root: &Directory,
        home: &Directory,
    ) -> Result<(), StoreError> {
        let name = "selector-cache.json";
        if home.exists(name)? || root.exists("meow-migrated.json")? || !data.exists(name)? {
            return Ok(());
        }
        let mut source = data.read(name)?;
        validate_selector_json(&mut source, &data.path.join(name))?;
        let backup = root.child("migration", &self.owner)?;
        copy_once(
            &mut source,
            &backup,
            "selector-cache.json.backup",
            &self.owner,
        )?;
        copy_once(&mut source, home, name, &self.owner)?;
        let mut marker = root.file("meow-migrated.new", &self.owner, true)?;
        marker.set_len(0)?;
        marker.write_all(b"{\"version\":1}")?;
        marker.sync_all()?;
        root.rename("meow-migrated.new", "meow-migrated.json")?;
        root.sync()?;
        Ok(())
    }

    fn migrate(
        &self,
        data: &Directory,
        root: &Directory,
        home: &Directory,
    ) -> Result<(), StoreError> {
        if home.exists("cache.db")? || root.exists("migration.json")? {
            return Ok(());
        }
        if !data.exists("cache.db")? {
            return Ok(());
        }
        let mut old = data.read("cache.db")?;
        // bbolt uses the same advisory lock. Never snapshot its live writer.
        old.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => StoreError::Busy(data.path.join("cache.db")),
            std::fs::TryLockError::Error(error) => StoreError::Access(error),
        })?;
        let format = cache_format(&mut old)?;
        let source_kind = match format {
            CacheFormat::Yaml => CoreKind::ClashRust,
            CacheFormat::Bolt => self
                .legacy_kind
                .filter(|kind| matches!(kind, CoreKind::Mihomo | CoreKind::ClashPremium))
                .ok_or_else(|| StoreError::Format(data.path.join("cache.db")))?,
            CacheFormat::Empty => return Ok(()),
            CacheFormat::Unknown => return Err(StoreError::Format(data.path.join("cache.db"))),
        };
        let migration = root.child("migration", &self.owner)?;
        copy_once(&mut old, &migration, "cache.db.backup", &self.owner)?;
        // Import into its own namespace even when the requested core is different.
        let target = root
            .child("v1", &self.owner)?
            .child(source_kind.as_ref(), &self.owner)?;
        if !target.exists("cache.db")? {
            copy_once(&mut old, &target, "cache.db", &self.owner)?;
        }
        let mut marker = root.file("migration.new", &self.owner, true)?;
        marker.set_len(0)?;
        marker.write_all(
            format!("{{\"version\":1,\"kind\":\"{}\"}}", source_kind.as_ref()).as_bytes(),
        )?;
        marker.sync_all()?;
        root.rename("migration.new", "migration.json")?;
        root.sync()?;
        Ok(())
    }
}

struct FsLease {
    root: Directory,
    home: Directory,
    owner: StoreOwner,
    kind: CoreKind,
    _lock: File,
}

impl Drop for FsLease {
    fn drop(&mut self) {
        // A concurrent fork may briefly retain this open file description.
        // Release the lock explicitly instead of waiting for every copy to close.
        let _ = self._lock.unlock();
    }
}

impl FsLease {
    fn check_cache(&self) -> Result<(), StoreError> {
        if self.kind == CoreKind::Meow {
            let mut file = self.home.file("selector-cache.json", &self.owner, true)?;
            if file.metadata()?.len() == 0 {
                file.write_all(b"{}")?;
                file.sync_all()?;
            }
            return validate_selector_json(&mut file, &self.home.path.join("selector-cache.json"));
        }
        let mut cache = self.home.file("cache.db", &self.owner, true)?;
        let format = cache_format(&mut cache)?;
        match (self.kind, format) {
            (_, CacheFormat::Empty) => {
                // An empty file is a valid new bbolt database; clash-rs expects
                // a YAML mapping and otherwise emits a spurious read warning.
                if self.kind == CoreKind::ClashRust {
                    cache.write_all(b"selected: {}\nip_to_host: {}\nhost_to_ip: {}\n")?;
                    cache.sync_all()?;
                }
                Ok(())
            }
            (CoreKind::ClashRust, CacheFormat::Yaml) => Ok(()),
            (CoreKind::Mihomo | CoreKind::ClashPremium, CacheFormat::Bolt) => Ok(()),
            _ => Err(StoreError::Format(self.home.path.join("cache.db"))),
        }
    }
}

impl StoreLease for FsLease {
    fn prepare_spawn(&self) -> Result<(), StoreError> {
        self.home.repair_tree(&self.owner)?;
        self.check_cache()?;
        let mut active = self.root.file("active", &self.owner, true)?;
        active.set_len(0)?;
        active.write_all(self.kind.as_ref().as_bytes())?;
        active.sync_all()?;
        self.root.sync()?;
        Ok(())
    }
    fn record_started(&self, pid_file: &Utf8Path) -> Result<(), StoreError> {
        let parent = Directory::open(
            pid_file
                .parent()
                .ok_or_else(|| StoreError::Unsafe(pid_file.into()))?,
        )?;
        let mut record = parent.read(pid_file.file_name().unwrap())?;
        let mut raw = String::new();
        record.read_to_string(&mut raw)?;
        nyanpasu_utils::process::recorded_process_is_alive(&raw)?;
        let mut file = self.root.file("active.new", &self.owner, true)?;
        file.set_len(0)?;
        file.write_all(raw.as_bytes())?;
        file.sync_all()?;
        self.root.rename("active.new", "active")?;
        self.root.sync()?;
        Ok(())
    }
    fn confirm_stopped(&self) -> Result<(), StoreError> {
        // Native downloads and atomic replacements may have created new inodes
        // as root. Hand those back before a desktop host takes the lease.
        self.home.repair_tree(&self.owner)?;
        self.root.remove("active")?;
        self.root.sync()?;
        Ok(())
    }
}

fn validate_selector_json(file: &mut File, path: &Utf8Path) -> Result<(), StoreError> {
    file.rewind()?;
    if file.metadata()?.len() > 64 * 1024 * 1024 {
        return Err(StoreError::Format(path.into()));
    }
    serde_json::from_reader::<_, std::collections::HashMap<String, String>>(file)
        .map_err(|_| StoreError::Format(path.into()))?;
    Ok(())
}

#[derive(Debug, PartialEq)]
enum CacheFormat {
    Empty,
    Yaml,
    Bolt,
    Unknown,
}
fn cache_format(file: &mut File) -> Result<CacheFormat, StoreError> {
    file.rewind()?;
    let size = file.metadata()?.len();
    if size == 0 {
        return Ok(CacheFormat::Empty);
    }
    let mut header = [0u8; 80];
    let n = file.read(&mut header)?;
    // bbolt meta page: page header (16), magic/version/page size. Check both
    // meta pages and their checksums before letting bbolt auto-delete a file.
    if n >= 80 && u32::from_le_bytes(header[16..20].try_into().unwrap()) == 0xed0cdaed {
        let page_size = u32::from_le_bytes(header[24..28].try_into().unwrap()) as u64;
        if !(1024..=65536).contains(&page_size)
            || !page_size.is_power_of_two()
            || size < page_size * 2
        {
            return Ok(CacheFormat::Unknown);
        }
        let first = valid_meta(&header);
        file.seek(std::io::SeekFrom::Start(page_size))?;
        let mut second = [0; 80];
        file.read_exact(&mut second)?;
        return Ok(if first || valid_meta(&second) {
            CacheFormat::Bolt
        } else {
            CacheFormat::Unknown
        });
    }
    file.rewind()?;
    // No unbounded YAML allocation from a malformed cache.
    if size > 64 * 1024 * 1024 {
        return Ok(CacheFormat::Unknown);
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let Ok(serde_yaml_ng::Value::Mapping(map)) = serde_yaml_ng::from_slice(&bytes) else {
        return Ok(CacheFormat::Unknown);
    };
    if !["selected", "ip_to_host", "host_to_ip"]
        .iter()
        .any(|k| map.contains_key(*k))
    {
        return Ok(CacheFormat::Unknown);
    }
    for key in ["selected", "ip_to_host", "host_to_ip"] {
        if let Some(value) = map.get(key) {
            let Some(entries) = value.as_mapping() else {
                return Ok(CacheFormat::Unknown);
            };
            if entries
                .iter()
                .any(|(k, v)| !k.is_string() || !v.is_string())
            {
                return Ok(CacheFormat::Unknown);
            }
        }
    }
    Ok(CacheFormat::Yaml)
}
fn valid_meta(bytes: &[u8; 80]) -> bool {
    if u32::from_le_bytes(bytes[16..20].try_into().unwrap()) != 0xed0cdaed
        || u32::from_le_bytes(bytes[20..24].try_into().unwrap()) != 2
    {
        return false;
    }
    let hash = bytes[16..72].iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    hash == u64::from_le_bytes(bytes[72..80].try_into().unwrap())
}

const RESOURCES: &[&str] = &[
    "Country.mmdb",
    "geoip.dat",
    "GeoIP.dat",
    "geosite.dat",
    "GeoSite.dat",
    "geoip.metadb",
    "geoip.db",
    "ASN.mmdb",
    "BundleMRS.7z",
    "wintun.dll",
];
fn seed_resources(
    source: &Directory,
    target: &Directory,
    owner: &StoreOwner,
) -> Result<(), StoreError> {
    for name in RESOURCES {
        if !source.exists(name)? {
            continue;
        }
        let mut input = source.read(name)?;
        let replace = if target.exists(name)? {
            input.metadata()?.modified()? > target.read(name)?.metadata()?.modified()?
        } else {
            true
        };
        if replace {
            copy_file(&mut input, target, name, owner)?;
        }
    }
    Ok(())
}
fn copy_once(
    source: &mut File,
    target: &Directory,
    name: &str,
    owner: &StoreOwner,
) -> Result<(), StoreError> {
    if target.exists(name)? {
        return Ok(());
    }
    copy_file(source, target, name, owner)
}
fn copy_file(
    source: &mut File,
    target: &Directory,
    name: &str,
    owner: &StoreOwner,
) -> Result<(), StoreError> {
    let temp = format!("{name}.import");
    let mut file = target.file(&temp, owner, true)?;
    file.set_len(0)?;
    source.rewind()?;
    std::io::copy(source, &mut file)?;
    file.sync_all()?;
    target.rename(&temp, name)?;
    target.sync()?;
    Ok(())
}

struct Directory {
    path: Utf8PathBuf,
    file: File,
}
impl Directory {
    fn repair_tree(&self, owner: &StoreOwner) -> Result<(), StoreError> {
        owner.authorize(&self.file, &self.path, true)?;
        // Names are enumerated by path, but all Unix opens and mutations use
        // the pinned parent handle. Replacements cannot redirect chown/chmod.
        for entry in std::fs::read_dir(&self.path)? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| StoreError::Unsafe(self.path.clone()))?;
            if entry.file_type()?.is_dir() {
                self.child(&name, owner)?.repair_tree(owner)?;
            } else {
                self.file(&name, owner, false)?;
            }
        }
        Ok(())
    }

    fn sync(&self) -> Result<(), StoreError> {
        #[cfg(unix)]
        self.file.sync_all()?;
        Ok(())
    }

    fn open(path: &Utf8Path) -> Result<Self, StoreError> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options
                .custom_flags(0x02000000 | 0x00200000)
                .share_mode(0x1 | 0x2);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_dir()
            || nyanpasu_utils::io::atomic_fs::is_reparse_point(&file.metadata()?)
        {
            return Err(StoreError::Unsafe(path.into()));
        }
        Ok(Self {
            path: path.into(),
            file,
        })
    }
    fn child(&self, name: &str, owner: &StoreOwner) -> Result<Self, StoreError> {
        #[cfg(unix)]
        {
            use std::os::fd::{AsRawFd, FromRawFd};
            let component = std::ffi::CString::new(name).unwrap();
            let fd = self.file.as_raw_fd();
            if unsafe { libc::mkdirat(fd, component.as_ptr(), 0o700) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(error.into());
                }
            }
            let child = unsafe {
                libc::openat(
                    fd,
                    component.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if child < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let child = Self {
                path: self.path.join(name),
                file: unsafe { File::from_raw_fd(child) },
            };
            owner.authorize(&child.file, &child.path, true)?;
            Ok(child)
        }
        #[cfg(windows)]
        {
            let path = self.path.join(name);
            match std::fs::create_dir(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.into()),
            }
            let child = Self::open(&path)?;
            owner.authorize(&child.file, &path, true)?;
            Ok(child)
        }
    }
    fn raw_file(&self, name: &str, write: bool, create: bool) -> Result<File, StoreError> {
        #[cfg(unix)]
        {
            use std::os::{
                fd::{AsRawFd, FromRawFd},
                unix::fs::MetadataExt,
            };
            let name_c = std::ffi::CString::new(name).unwrap();
            let flags = libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if write { libc::O_RDWR } else { libc::O_RDONLY }
                | if create { libc::O_CREAT } else { 0 };
            let fd = unsafe { libc::openat(self.file.as_raw_fd(), name_c.as_ptr(), flags, 0o600) };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let file = unsafe { File::from_raw_fd(fd) };
            let meta = file.metadata()?;
            if !meta.is_file() || meta.nlink() != 1 {
                return Err(StoreError::Unsafe(self.path.join(name)));
            }
            Ok(file)
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let file = OpenOptions::new()
                .read(true)
                .write(write)
                .create(create)
                .custom_flags(0x00200000)
                .open(self.path.join(name))?;
            let meta = file.metadata()?;
            if !meta.is_file()
                || windows_link_count(&file)? != 1
                || nyanpasu_utils::io::atomic_fs::is_reparse_point(&meta)
            {
                return Err(StoreError::Unsafe(self.path.join(name)));
            }
            Ok(file)
        }
    }
    #[cfg(target_os = "linux")]
    fn permission_handle(&self, name: &str) -> Result<File, StoreError> {
        use std::os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        };
        let component = std::ffi::CString::new(name).unwrap();
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                component.as_ptr(),
                libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.nlink() != 1 {
            return Err(StoreError::Unsafe(self.path.join(name)));
        }
        Ok(file)
    }

    fn file(&self, name: &str, owner: &StoreOwner, create: bool) -> Result<File, StoreError> {
        // Repair a readable but unwritable inode through its handle first.
        match self.read(name) {
            Ok(file) => owner.authorize(&file, &self.path.join(name), false)?,
            #[cfg(target_os = "linux")]
            Err(StoreError::Access(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                owner.authorize(&self.permission_handle(name)?, &self.path.join(name), false)?;
            }
            Err(StoreError::Access(e)) if e.kind() == std::io::ErrorKind::NotFound && create => {}
            Err(e) => return Err(e),
        }
        let file = self.raw_file(name, true, create)?;
        owner.authorize(&file, &self.path.join(name), false)?;
        Ok(file)
    }
    fn read(&self, name: &str) -> Result<File, StoreError> {
        self.raw_file(name, false, false)
    }
    fn exists(&self, name: &str) -> Result<bool, StoreError> {
        match self.read(name) {
            Ok(_) => Ok(true),
            Err(StoreError::Access(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }
    fn remove(&self, name: &str) -> Result<(), StoreError> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let name = std::ffi::CString::new(name).unwrap();
            if unsafe { libc::unlinkat(self.file.as_raw_fd(), name.as_ptr(), 0) } != 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e.into());
                }
            }
        }
        #[cfg(windows)]
        {
            match std::fs::remove_file(self.path.join(name)) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }
    fn rename(&self, old: &str, new: &str) -> Result<(), StoreError> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let old = std::ffi::CString::new(old).unwrap();
            let new = std::ffi::CString::new(new).unwrap();
            if unsafe {
                libc::renameat(
                    self.file.as_raw_fd(),
                    old.as_ptr(),
                    self.file.as_raw_fd(),
                    new.as_ptr(),
                )
            } != 0
            {
                return Err(std::io::Error::last_os_error().into());
            }
        }
        #[cfg(windows)]
        {
            std::fs::rename(self.path.join(old), self.path.join(new))?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn windows_link_count(file: &File) -> Result<u32, StoreError> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle},
    };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    Ok(info.nNumberOfLinks)
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &[u8] = b"selected:\n  Proxy: REJECT\nip_to_host: {}\nhost_to_ip: {}\n";

    fn store(root: &std::path::Path, kind: Option<CoreKind>) -> FsNativeStore {
        FsNativeStore::new(
            Utf8PathBuf::from_path_buf(root.into()).unwrap(),
            StoreOwner::current(),
            kind,
        )
    }

    fn bolt() -> Vec<u8> {
        let mut bytes = vec![0u8; 8192];
        for offset in [0, 4096] {
            let meta = &mut bytes[offset..offset + 80];
            meta[16..20].copy_from_slice(&0xed0cdaedu32.to_le_bytes());
            meta[20..24].copy_from_slice(&2u32.to_le_bytes());
            meta[24..28].copy_from_slice(&4096u32.to_le_bytes());
            let hash = meta[16..72].iter().fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            });
            meta[72..80].copy_from_slice(&hash.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn yaml_migrates_to_clash_rs_even_when_mihomo_starts_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cache.db"), YAML).unwrap();
        let store = store(dir.path(), Some(CoreKind::Mihomo));
        let lease = store.acquire(CoreKind::Mihomo).unwrap();
        assert!(
            std::fs::read(store.home(CoreKind::Mihomo).join("cache.db"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            std::fs::read(store.home(CoreKind::ClashRust).join("cache.db")).unwrap(),
            YAML
        );
        assert_eq!(
            std::fs::read(dir.path().join("native-store/migration/cache.db.backup")).unwrap(),
            YAML
        );
        assert_eq!(std::fs::read(dir.path().join("cache.db")).unwrap(), YAML);
        drop(lease);
        let newer = b"selected: {Proxy: DIRECT}\n";
        std::fs::write(store.home(CoreKind::ClashRust).join("cache.db"), newer).unwrap();
        drop(store.acquire(CoreKind::ClashRust).unwrap());
        assert_eq!(
            std::fs::read(store.home(CoreKind::ClashRust).join("cache.db")).unwrap(),
            newer
        );
    }

    #[test]
    fn bolt_requires_provenance_and_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = bolt();
        std::fs::write(dir.path().join("cache.db"), &bytes).unwrap();
        let unknown = store(dir.path(), None);
        assert!(matches!(
            unknown.acquire(CoreKind::Mihomo),
            Err(StoreError::Format(_))
        ));
        assert_eq!(std::fs::read(dir.path().join("cache.db")).unwrap(), bytes);
        let known = store(dir.path(), Some(CoreKind::Mihomo));
        drop(known.acquire(CoreKind::ClashRust).unwrap());
        assert_eq!(
            std::fs::read(known.home(CoreKind::Mihomo).join("cache.db")).unwrap(),
            bytes
        );
    }

    #[test]
    fn corrupt_or_wrong_format_is_rejected_without_recreation() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), None);
        drop(store.acquire(CoreKind::Mihomo).unwrap());
        let path = store.home(CoreKind::Mihomo).join("cache.db");
        let mut corrupt = bolt();
        corrupt[72] ^= 1;
        corrupt[4096 + 72] ^= 1;
        for bytes in [YAML.to_vec(), corrupt, b"broken: [".to_vec()] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(matches!(
                store.acquire(CoreKind::Mihomo),
                Err(StoreError::Format(_))
            ));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn leases_exclude_other_hosts_and_checks_never_touch_native_data() {
        let dir = tempfile::tempdir().unwrap();
        let first = store(dir.path(), None);
        let second = store(dir.path(), None);
        let check = first.check_home().unwrap();
        assert!(!dir.path().join("native-store").exists());
        std::fs::write(
            check.path().join("cache.db"),
            b"a check may initialize fake IP state",
        )
        .unwrap();
        let lease = first.acquire(CoreKind::Mihomo).unwrap();
        assert!(matches!(
            second.acquire(CoreKind::ClashRust),
            Err(StoreError::Busy(_))
        ));
        drop(lease);
        drop(second.acquire(CoreKind::ClashRust).unwrap());
    }

    #[test]
    fn dropping_a_prepared_lease_does_not_claim_process_death() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), None);
        let lease = store.acquire(CoreKind::ClashRust).unwrap();
        lease.prepare_spawn().unwrap();
        drop(lease);
        assert!(matches!(
            store.acquire(CoreKind::ClashRust),
            Err(StoreError::Recovery(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_unlocked_store_still_rejects_a_live_recorded_process() {
        use nyanpasu_utils::process::{Command, EpochPidFile, EpochPidFileSpec};
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), None);
        let config = dir.path().join("config-1.yaml");
        let pid = dir.path().join("core-1.pid");
        std::fs::write(&config, "{}").unwrap();
        let lease = store.acquire(CoreKind::ClashRust).unwrap();
        lease.prepare_spawn().unwrap();
        let (child, _events) = Command::new("sleep")
            .args(["30"])
            .epoch_pid_file(EpochPidFile::new(EpochPidFileSpec {
                pid_path: &pid,
                runtime_config: &config,
                epoch: 1,
            }))
            .spawn()
            .await
            .unwrap();
        lease
            .record_started(Utf8Path::from_path(&pid).unwrap())
            .unwrap();
        drop(lease);
        assert!(matches!(
            store.acquire(CoreKind::ClashRust),
            Err(StoreError::Busy(_))
        ));
        child.kill().await.unwrap();
        drop(store.acquire(CoreKind::ClashRust).unwrap());
        assert!(!dir.path().join("native-store/active").exists());
    }

    #[test]
    fn confirmed_stop_releases_the_dirty_marker() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), None);
        let lease = store.acquire(CoreKind::ClashRust).unwrap();
        lease.prepare_spawn().unwrap();
        lease.confirm_stopped().unwrap();
        drop(lease);
        drop(store.acquire(CoreKind::ClashRust).unwrap());
    }

    #[test]
    fn dropping_a_lease_unlocks_a_duplicated_descriptor() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), None);
        let (_, root, home) = store.directories(CoreKind::ClashRust).unwrap();
        let lock = root.file("lease.lock", &store.owner, true).unwrap();
        lock.try_lock().unwrap();
        let duplicate = lock.try_clone().unwrap();
        let lease = FsLease {
            root,
            home,
            owner: store.owner,
            kind: CoreKind::ClashRust,
            _lock: lock,
        };
        drop(lease);
        drop(store.acquire(CoreKind::ClashRust).unwrap());
        drop(duplicate);
    }

    #[test]
    fn meow_migrates_its_json_without_consuming_a_bolt_database() {
        let dir = tempfile::tempdir().unwrap();
        let selected = br#"{"Proxy":"REJECT"}"#;
        std::fs::write(dir.path().join("selector-cache.json"), selected).unwrap();
        std::fs::write(dir.path().join("cache.db"), bolt()).unwrap();
        let store = store(dir.path(), Some(CoreKind::Meow));
        drop(store.acquire(CoreKind::Meow).unwrap());
        let target = store.home(CoreKind::Meow).join("selector-cache.json");
        assert_eq!(std::fs::read(&target).unwrap(), selected);
        assert!(!store.home(CoreKind::Meow).join("cache.db").exists());
        assert_eq!(
            std::fs::read(
                dir.path()
                    .join("native-store/migration/selector-cache.json.backup")
            )
            .unwrap(),
            selected
        );
        std::fs::write(&target, b"broken").unwrap();
        assert!(matches!(
            store.acquire(CoreKind::Meow),
            Err(StoreError::Format(_))
        ));
        assert_eq!(std::fs::read(target).unwrap(), b"broken");
    }

    #[test]
    fn legacy_bolt_is_not_copied_while_its_writer_holds_the_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.db");
        std::fs::write(&path, bolt()).unwrap();
        let writer = File::open(path).unwrap();
        writer.lock().unwrap();
        let store = store(dir.path(), Some(CoreKind::Mihomo));
        assert!(matches!(
            store.acquire(CoreKind::Mihomo),
            Err(StoreError::Busy(_))
        ));
        assert!(!store.home(CoreKind::Mihomo).join("cache.db").exists());
        drop(writer);
        drop(store.acquire(CoreKind::Mihomo).unwrap());
    }

    #[test]
    fn absent_service_owner_fails_before_creating_native_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsNativeStore::new(
            Utf8PathBuf::from_path_buf(dir.path().into()).unwrap(),
            StoreOwner::Missing,
            None,
        );
        assert!(matches!(
            store.acquire(CoreKind::Mihomo),
            Err(StoreError::OwnerMissing)
        ));
        assert!(!dir.path().join("native-store").exists());
    }

    #[cfg(unix)]
    #[ignore = "requires root; changes ownership only inside a disposable temporary directory"]
    #[test]
    fn privileged_handoff_repairs_replaced_files_before_desktop_access() {
        use std::os::unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        };
        const CHILD: &str = "NYANPASU_STORE_HANDOFF_TEST_DIR";
        if let Some(path) = std::env::var_os(CHILD) {
            assert_eq!(unsafe { libc::geteuid() }, 65534);
            let store = store(std::path::Path::new(&path), None);
            let lease = store.acquire(CoreKind::ClashRust).unwrap();
            lease.prepare_spawn().unwrap();
            let cache = store.home(CoreKind::ClashRust).join("cache.db");
            assert_eq!(std::fs::read(&cache).unwrap(), YAML);
            std::fs::write(cache, b"selected: {Proxy: DIRECT}\n").unwrap();
            lease.confirm_stopped().unwrap();
            return;
        }
        assert_eq!(
            unsafe { libc::geteuid() },
            0,
            "run this test in a disposable privileged environment"
        );
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.path().join("cache.db"), YAML).unwrap();
        let store = FsNativeStore::new(
            Utf8PathBuf::from_path_buf(dir.path().into()).unwrap(),
            StoreOwner::Unix {
                uid: 65534,
                gid: 65534,
            },
            Some(CoreKind::ClashRust),
        );
        let lease = store.acquire(CoreKind::ClashRust).unwrap();
        lease.prepare_spawn().unwrap();
        let cache = store.home(CoreKind::ClashRust).join("cache.db");
        let replacement = cache.with_extension("replacement");
        std::fs::write(&replacement, YAML).unwrap();
        std::fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::rename(replacement, &cache).unwrap();
        assert_eq!(std::fs::metadata(&cache).unwrap().uid(), 0);
        lease.confirm_stopped().unwrap();
        drop(lease);
        assert_eq!(std::fs::metadata(&cache).unwrap().uid(), 65534);
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "native_store::fs::tests::privileged_handoff_repairs_replaced_files_before_desktop_access", "--ignored"])
            .env(CHILD, dir.path()).uid(65534).gid(65534).status().unwrap();
        assert!(status.success());
    }

    #[cfg(unix)]
    #[test]
    fn every_spawn_repairs_owned_permissions_without_truncation() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path(), None);
        let lease = store.acquire(CoreKind::ClashRust).unwrap();
        let cache = store.home(CoreKind::ClashRust).join("cache.db");
        std::fs::write(&cache, YAML).unwrap();
        #[cfg(target_os = "linux")]
        let modes = [0o444, 0o000];
        #[cfg(not(target_os = "linux"))]
        let modes = [0o444, 0o444];
        for mode in modes {
            std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(mode)).unwrap();
            lease.prepare_spawn().unwrap();
            assert_eq!(
                std::fs::metadata(&cache).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(std::fs::read(&cache).unwrap(), YAML);
        }
        lease.confirm_stopped().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_hardlinks_cannot_redirect_permission_repair() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), YAML).unwrap();
        std::fs::set_permissions(outside.path(), std::fs::Permissions::from_mode(0o444)).unwrap();
        let store = store(dir.path(), None);
        drop(store.acquire(CoreKind::ClashRust).unwrap());
        let cache = store.home(CoreKind::ClashRust).join("cache.db");
        std::fs::remove_file(&cache).unwrap();
        symlink(outside.path(), &cache).unwrap();
        assert!(store.acquire(CoreKind::ClashRust).is_err());
        std::fs::remove_file(&cache).unwrap();
        std::fs::hard_link(outside.path(), &cache).unwrap();
        assert!(matches!(
            store.acquire(CoreKind::ClashRust),
            Err(StoreError::Unsafe(_))
        ));
        assert_eq!(
            std::fs::metadata(outside.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
        assert_eq!(std::fs::read(outside.path()).unwrap(), YAML);
    }
}
