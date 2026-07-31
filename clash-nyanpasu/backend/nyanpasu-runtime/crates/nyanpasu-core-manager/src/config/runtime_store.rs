//! Durable, manager-owned runtime configuration artifacts.

use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "test-hooks")]
use std::sync::{Arc, atomic::AtomicUsize};

use camino::{Utf8Path, Utf8PathBuf};
use nyanpasu_utils::io::atomic_fs;
use tokio::io::AsyncWriteExt;

use crate::Error;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct RuntimeConfigStore {
    dir: Utf8PathBuf,
    #[cfg(feature = "test-hooks")]
    replace_parent_sync_failures: Arc<AtomicUsize>,
}

#[derive(Debug)]
pub(crate) struct RuntimeDirectoryLock {
    _lock: atomic_fs::DirLock,
}

#[derive(Debug)]
pub struct StagedRuntimeConfig {
    path: Utf8PathBuf,
    consumed: bool,
}

impl StagedRuntimeConfig {
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }
}

impl Drop for StagedRuntimeConfig {
    fn drop(&mut self) {
        if !self.consumed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeConfigBackup {
    path: Utf8PathBuf,
    epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCommitDurability {
    Durable,
    Uncertain(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeConfigCommit {
    path: Utf8PathBuf,
    durability: RuntimeCommitDurability,
}

impl RuntimeConfigCommit {
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    pub fn durability(&self) -> &RuntimeCommitDurability {
        &self.durability
    }

    pub fn durability_warning(&self) -> Option<&str> {
        match &self.durability {
            RuntimeCommitDurability::Durable => None,
            RuntimeCommitDurability::Uncertain(warning) => Some(warning),
        }
    }
}

impl RuntimeConfigBackup {
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }
}

impl RuntimeConfigStore {
    pub async fn new(dir: Utf8PathBuf) -> Result<Self, Error> {
        match tokio::fs::symlink_metadata(&dir).await {
            Ok(metadata) => validate_directory_metadata(&dir, &metadata)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir_all(&dir).await?;
            }
            Err(error) => return Err(error.into()),
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).await?;
        }
        #[cfg(windows)]
        {
            atomic_fs::harden_windows_directory_acl(&dir)?;
            atomic_fs::verify_windows_directory_acl(&dir)?;
        }

        let canonical = tokio::fs::canonicalize(&dir).await?;
        let dir = Utf8PathBuf::from_path_buf(canonical)
            .map_err(|_| Error::InvalidManagerOptions("runtime directory is not UTF-8".into()))?;
        let metadata = tokio::fs::symlink_metadata(&dir).await?;
        validate_directory_metadata(&dir, &metadata)?;

        Ok(Self {
            dir,
            #[cfg(feature = "test-hooks")]
            replace_parent_sync_failures: Arc::new(AtomicUsize::new(0)),
        })
    }

    pub fn dir(&self) -> &Utf8Path {
        &self.dir
    }

    pub fn runtime_path(&self, epoch: u64) -> Utf8PathBuf {
        self.dir.join(format!("config-{epoch}.yaml"))
    }

    pub fn pid_path(&self, epoch: u64) -> Utf8PathBuf {
        self.dir.join(format!("core-{epoch}.pid"))
    }

    pub fn socket_path(&self, epoch: u64) -> Utf8PathBuf {
        self.dir.join(format!("core-{epoch}.sock"))
    }

    #[cfg(feature = "test-hooks")]
    pub(crate) fn inject_replace_parent_sync_failure_once(&self) {
        self.replace_parent_sync_failures
            .fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) async fn acquire_ownership(&self) -> Result<RuntimeDirectoryLock, Error> {
        let path = self.dir.join(".manager.lock");
        tokio::task::spawn_blocking(move || acquire_runtime_directory_lock(&path))
            .await
            .map_err(std::io::Error::other)?
    }

    pub async fn stage(&self, epoch: u64, contents: &[u8]) -> Result<StagedRuntimeConfig, Error> {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = self.dir.join(format!(
            ".config-{epoch}.yaml.tmp-{}-{counter}",
            std::process::id()
        ));
        atomic_fs::validate_absent_regular_target(&path).await?;

        let mut options = tokio::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))
                .await?;
        }

        if let Err(error) = async {
            file.write_all(contents).await?;
            file.flush().await?;
            file.sync_all().await
        }
        .await
        {
            drop(file);
            let _ = tokio::fs::remove_file(&path).await;
            return Err(error.into());
        }
        drop(file);
        Ok(StagedRuntimeConfig {
            path,
            consumed: false,
        })
    }

    pub async fn commit_new(
        &self,
        mut staged: StagedRuntimeConfig,
        epoch: u64,
    ) -> Result<Utf8PathBuf, Error> {
        self.validate_staged(&staged, epoch).await?;
        let target = self.runtime_path(epoch);
        atomic_fs::validate_absent_regular_target(&target).await?;
        atomic_fs::atomic_move_new(&staged.path, &target).await?;
        staged.consumed = true;
        atomic_fs::sync_dir(&self.dir).await?;
        Ok(target)
    }

    pub async fn replace(&self, epoch: u64, contents: &[u8]) -> Result<RuntimeConfigCommit, Error> {
        let staged = self.stage(epoch, contents).await?;
        self.commit_replace(staged, epoch).await
    }

    /// Replaces the stable epoch file with bytes that were already staged and
    /// validated. The staged file remains in the runtime directory, so the
    /// atomicity guarantees are identical to [`Self::replace`].
    pub async fn commit_replace(
        &self,
        mut staged: StagedRuntimeConfig,
        epoch: u64,
    ) -> Result<RuntimeConfigCommit, Error> {
        self.validate_staged(&staged, epoch).await?;
        let target = self.runtime_path(epoch);
        atomic_fs::validate_existing_regular_target(&target).await?;
        atomic_fs::atomic_replace(&staged.path, &target).await?;
        staged.consumed = true;
        #[cfg(feature = "test-hooks")]
        let injected_failure = self
            .replace_parent_sync_failures
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |remaining| {
                remaining.checked_sub(1)
            })
            .is_ok();
        #[cfg(feature = "test-hooks")]
        let parent_sync = if injected_failure {
            Err(std::io::Error::other(
                "injected parent-directory synchronization failure",
            ))
        } else {
            atomic_fs::sync_dir(&self.dir).await
        };
        #[cfg(not(feature = "test-hooks"))]
        let parent_sync = atomic_fs::sync_dir(&self.dir).await;
        Ok(installed_commit(target, parent_sync))
    }

    pub async fn backup(&self, epoch: u64, generation: u64) -> Result<RuntimeConfigBackup, Error> {
        let target = self.runtime_path(epoch);
        atomic_fs::validate_existing_regular_target(&target).await?;
        let contents = tokio::fs::read(&target).await?;
        let mut staged = self.stage(epoch, &contents).await?;
        let backup_path = self
            .dir
            .join(format!("config-{epoch}.yaml.backup-{generation}"));
        atomic_fs::validate_absent_regular_target(&backup_path).await?;
        atomic_fs::atomic_move_new(&staged.path, &backup_path).await?;
        staged.consumed = true;
        atomic_fs::sync_dir(&self.dir).await?;
        Ok(RuntimeConfigBackup {
            path: backup_path,
            epoch,
        })
    }

    pub async fn restore(
        &self,
        backup: &RuntimeConfigBackup,
    ) -> Result<RuntimeConfigCommit, Error> {
        atomic_fs::validate_existing_regular_target(&backup.path).await?;
        let contents = tokio::fs::read(&backup.path).await?;
        self.replace(backup.epoch, &contents).await
    }

    pub async fn remove_backup(&self, backup: RuntimeConfigBackup) -> Result<(), Error> {
        atomic_fs::remove_regular_file(&backup.path)
            .await
            .map_err(Error::from)
    }

    pub async fn cleanup_epoch(&self, epoch: u64) -> Result<(), Error> {
        for path in [self.runtime_path(epoch), self.pid_path(epoch)] {
            atomic_fs::remove_regular_file(&path).await?;
        }
        remove_socket_artifact(&self.socket_path(epoch)).await?;

        let prefix = format!("config-{epoch}.yaml.backup-");
        let temp_prefix = format!(".config-{epoch}.yaml.tmp-");
        let pid_temp_prefix = format!("core-{epoch}.pid.tmp-");
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with(&prefix)
                || name.starts_with(&temp_prefix)
                || name.starts_with(&pid_temp_prefix)
            {
                let path = Utf8PathBuf::from_path_buf(entry.path())
                    .map_err(|_| Error::UnsafeRuntimeArtifact(self.dir.clone()))?;
                atomic_fs::remove_regular_file(&path).await?;
            }
        }
        atomic_fs::sync_dir(&self.dir).await?;
        Ok(())
    }

    pub async fn artifact_epochs(&self) -> Result<Vec<u64>, Error> {
        let mut epochs = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Some(epoch) = artifact_epoch(&name) {
                epochs.push(epoch);
            }
        }
        epochs.sort_unstable();
        epochs.dedup();
        Ok(epochs)
    }

    async fn validate_staged(&self, staged: &StagedRuntimeConfig, epoch: u64) -> Result<(), Error> {
        if staged.path.parent() != Some(self.dir.as_path())
            || !staged
                .path
                .file_name()
                .is_some_and(|name| name.starts_with(&format!(".config-{epoch}.yaml.tmp-")))
        {
            return Err(Error::UnsafeRuntimeArtifact(staged.path.clone()));
        }
        atomic_fs::validate_existing_regular_target(&staged.path)
            .await
            .map_err(Error::from)
    }
}

fn acquire_runtime_directory_lock(path: &Utf8Path) -> Result<RuntimeDirectoryLock, Error> {
    atomic_fs::acquire_dir_lock(path)
        .map(|lock| RuntimeDirectoryLock { _lock: lock })
        .map_err(Error::from)
}

fn artifact_epoch(name: &str) -> Option<u64> {
    let rest = name
        .strip_prefix("core-")
        .and_then(|value| value.strip_suffix(".pid"))
        .or_else(|| {
            name.strip_prefix("core-")
                .and_then(|value| value.strip_suffix(".sock"))
        })
        .or_else(|| {
            name.strip_prefix("config-").and_then(|value| {
                value
                    .strip_suffix(".yaml")
                    .or_else(|| value.split_once(".yaml.backup-").map(|(epoch, _)| epoch))
            })
        })
        .or_else(|| {
            name.strip_prefix(".config-")
                .and_then(|value| value.split_once(".yaml.tmp-").map(|(epoch, _)| epoch))
        })
        .or_else(|| {
            name.strip_prefix("core-")
                .and_then(|value| value.split_once(".pid.tmp-").map(|(epoch, _)| epoch))
        })?;
    rest.parse().ok()
}

fn installed_commit(path: Utf8PathBuf, parent_sync: std::io::Result<()>) -> RuntimeConfigCommit {
    let durability = match parent_sync {
        Ok(()) => RuntimeCommitDurability::Durable,
        Err(error) => RuntimeCommitDurability::Uncertain(format!(
            "runtime config was atomically installed, but parent-directory synchronization failed: {error}"
        )),
    };
    RuntimeConfigCommit { path, durability }
}

fn validate_directory_metadata(path: &Utf8Path, metadata: &std::fs::Metadata) -> Result<(), Error> {
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || atomic_fs::is_reparse_point(metadata)
    {
        return Err(Error::UnsafeRuntimeArtifact(path.to_owned()));
    }
    Ok(())
}

async fn remove_socket_artifact(path: &Utf8Path) -> Result<(), Error> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata)
            if metadata.file_type().is_symlink() || atomic_fs::is_reparse_point(&metadata) =>
        {
            Err(Error::UnsafeRuntimeArtifact(path.to_owned()))
        }
        #[cfg(unix)]
        Ok(metadata) if !std::os::unix::fs::FileTypeExt::is_socket(&metadata.file_type()) => {
            Err(Error::UnsafeRuntimeArtifact(path.to_owned()))
        }
        #[cfg(windows)]
        Ok(metadata) if !metadata.is_file() => Err(Error::UnsafeRuntimeArtifact(path.to_owned())),
        Ok(_) => {
            tokio::fs::remove_file(path).await?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store_dir() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.path().join("runtime")).unwrap();
        (dir, path)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn atomic_replace_readers_only_observe_complete_documents() {
        let (_guard, dir) = temp_store_dir();
        let store = RuntimeConfigStore::new(dir).await.unwrap();
        let old = b"marker: old\npayload: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
        let new = b"marker: new\npayload: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n";
        let staged = store.stage(7, old).await.unwrap();
        let path = store.commit_new(staged, 7).await.unwrap();

        let mut readers = Vec::new();
        for _ in 0..8 {
            let path = path.clone();
            let old = old.to_vec();
            let new = new.to_vec();
            readers.push(tokio::spawn(async move {
                for _ in 0..500 {
                    #[cfg(windows)]
                    let observed = loop {
                        match tokio::fs::read(&path).await {
                            Ok(observed) => break observed,
                            Err(error) if matches!(error.raw_os_error(), Some(2 | 5 | 32 | 33)) => {
                                tokio::task::yield_now().await;
                            }
                            Err(error) => panic!("runtime read failed: {error}"),
                        }
                    };
                    #[cfg(not(windows))]
                    let observed = tokio::fs::read(&path)
                        .await
                        .unwrap_or_else(|error| panic!("runtime read failed: {error}"));
                    assert!(observed == old || observed == new, "partial YAML observed");
                    tokio::task::yield_now().await;
                }
            }));
        }
        for round in 0..100 {
            store
                .replace(7, if round % 2 == 0 { new } else { old })
                .await
                .unwrap();
        }
        for reader in readers {
            reader.await.unwrap();
        }
    }

    #[tokio::test]
    async fn backup_restore_and_cleanup_preserve_complete_versions() {
        let (_guard, dir) = temp_store_dir();
        let store = RuntimeConfigStore::new(dir).await.unwrap();
        let staged = store.stage(3, b"value: old\n").await.unwrap();
        let path = store.commit_new(staged, 3).await.unwrap();
        let backup = store.backup(3, 1).await.unwrap();
        store.replace(3, b"value: new\n").await.unwrap();
        store.restore(&backup).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&path).await.unwrap(),
            "value: old\n"
        );
        store.remove_backup(backup).await.unwrap();
        store.cleanup_epoch(3).await.unwrap();
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cleanup_epoch_removes_a_real_unix_socket() {
        use std::os::unix::{fs::FileTypeExt, net::UnixListener};

        let (_guard, dir) = temp_store_dir();
        let store = RuntimeConfigStore::new(dir).await.unwrap();
        let socket_path = store.socket_path(9);
        let listener = UnixListener::bind(&socket_path).unwrap();
        assert!(
            std::fs::symlink_metadata(&socket_path)
                .unwrap()
                .file_type()
                .is_socket()
        );
        drop(listener);

        store.cleanup_epoch(9).await.unwrap();
        assert!(!socket_path.exists());
    }

    #[test]
    fn installed_commit_reports_parent_sync_uncertainty_without_becoming_an_error() {
        let path = Utf8PathBuf::from("config-4.yaml");
        let commit = installed_commit(
            path.clone(),
            Err(std::io::Error::other("injected directory fsync failure")),
        );
        assert_eq!(commit.path(), path);
        assert!(matches!(
            commit.durability(),
            RuntimeCommitDurability::Uncertain(message)
                if message.contains("atomically installed") && message.contains("injected")
        ));
    }
}
