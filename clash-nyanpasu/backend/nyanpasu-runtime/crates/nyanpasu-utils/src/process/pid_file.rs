use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Duration,
};

use tokio::io::AsyncWriteExt;

const EPOCH_PID_VERSION: u32 = 2;
const IDENTITY_WAIT_ATTEMPTS: usize = 20;
const IDENTITY_WAIT_DELAY: Duration = Duration::from_millis(25);
const KILL_WAIT_ATTEMPTS: usize = 100;
const KILL_WAIT_DELAY: Duration = Duration::from_millis(50);

/// Describes one manager-owned, per-epoch pid record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochPidFile {
    path: PathBuf,
    epoch: u64,
    runtime_config: PathBuf,
}

/// The inputs one [`EpochPidFile`] is built from. The two paths have the same
/// type and sit either side of the epoch, so only a name says which is the pid
/// record and which is the runtime config it belongs to.
#[derive(Debug, Clone, Copy)]
pub struct EpochPidFileSpec<'a> {
    /// Where the pid record is written.
    pub pid_path: &'a Path,
    /// The runtime config this epoch was launched with.
    pub runtime_config: &'a Path,
    pub epoch: u64,
}

impl EpochPidFile {
    pub fn new(spec: EpochPidFileSpec<'_>) -> Self {
        Self {
            path: spec.pid_path.to_owned(),
            epoch: spec.epoch,
            runtime_config: spec.runtime_config.to_owned(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn runtime_config(&self) -> &Path {
        &self.runtime_config
    }
}

/// Versioned pid-file contents used for post-manager-kill orphan recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochPidRecord {
    pub pid: u32,
    pub epoch: u64,
    pub executable: String,
    pub start_token: u64,
    pub runtime_config: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanReapOutcome {
    NotFound,
    /// The recorded process is gone: its pid is free or now names a process
    /// with a different start token. Nothing was signaled.
    AlreadyExited,
    Killed,
}

/// Classifies failures to safely retire an epoch pid record.
#[derive(Debug, thiserror::Error)]
pub enum OrphanReapError {
    /// The record or its referenced runtime path is invalid. No process was signaled.
    #[error("unusable epoch pid record at {path}: {source}")]
    RecordUnusable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Ownership or exit could not be proven. A validated kill may already have occurred.
    #[error("cannot confirm retirement of pid {pid} recorded at {path}: {source}")]
    ProcessUnconfirmed {
        path: PathBuf,
        pid: u32,
        #[source]
        source: std::io::Error,
    },
    /// A filesystem operation failed, possibly after an ownership-verified kill.
    #[error("epoch pid record filesystem operation failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessIdentity {
    /// `None` when the start token is readable but the executable is not, as
    /// for another user's process on Linux (`/proc/<pid>/exe`).
    executable: Option<String>,
    start_token: u64,
}

/// How a live process relates to an epoch record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentityMatch {
    /// Same start token and executable: the recorded process itself.
    Same,
    /// A different start token: the recorded process exited and its pid now
    /// names another process.
    Reused,
    /// Same start token, but the executable differs or cannot be read. A live
    /// process can change its observed name (exec, a replaced binary), so this
    /// proves neither ownership nor exit.
    Uncertain,
}

/// Owns one pid file around a spawned child.
///
/// Legacy numeric files remain supported for writing and cleanup, but no longer
/// authorize residual kills because they cannot prove epoch/start identity.
/// Epoch records kill only a fully matching same-epoch leftover.
pub(crate) enum PidFileGuard {
    Legacy {
        path: PathBuf,
        pid: AtomicU32,
    },
    Epoch {
        spec: EpochPidFile,
        expected_exe: String,
        record: parking_lot::Mutex<Option<EpochPidRecord>>,
    },
}

impl PidFileGuard {
    pub(crate) async fn prepare_legacy(path: PathBuf) -> std::io::Result<Self> {
        validate_pid_target(&path).await?;
        Ok(Self::Legacy {
            path,
            pid: AtomicU32::new(0),
        })
    }

    pub(crate) async fn prepare_epoch(
        spec: EpochPidFile,
        expected_exe: String,
    ) -> std::io::Result<Self> {
        let spec = normalize_epoch_spec(spec).await?;
        validate_pid_target(&spec.path).await?;

        if let Some(record) = read_epoch_pid_file(&spec.path).await? {
            validate_record_for_spec(&record, &spec, &expected_exe)?;
            reap_record(&record).await?;
            remove_record_if_matches(&spec.path, &record).await?;
        }

        Ok(Self::Epoch {
            spec,
            expected_exe,
            record: parking_lot::Mutex::new(None),
        })
    }

    pub(crate) async fn write(&self, pid: u32) -> std::io::Result<()> {
        match self {
            Self::Legacy { path, pid: slot } => {
                crate::os::create_pid_file(path, pid).await?;
                slot.store(pid, Ordering::Relaxed);
                Ok(())
            }
            Self::Epoch {
                spec,
                expected_exe,
                record,
            } => {
                // The child is observable between group.start() and this
                // identity-bound record publication. The manager sweeps stale
                // `core-{epoch}.pid.tmp-*` files, but a hard kill inside this
                // narrow pre-record interval can leave an orphan without an
                // authoritative identity record; such a process is never
                // killed on an unproven numeric PID.
                let identity = wait_for_process_identity(pid, expected_exe)
                    .await?
                    .ok_or_else(|| {
                        identity_error(format!("spawned pid {pid} disappeared before recording"))
                    })?;
                let executable = identity.executable.ok_or_else(|| {
                    identity_error(format!("cannot resolve executable for spawned pid {pid}"))
                })?;
                if !exe_names_equal(&executable, expected_exe) {
                    return Err(identity_error(format!(
                        "spawned pid {pid} executable `{executable}` does not match `{expected_exe}`"
                    )));
                }
                let value = EpochPidRecord {
                    pid,
                    epoch: spec.epoch,
                    executable,
                    start_token: identity.start_token,
                    runtime_config: spec.runtime_config.clone(),
                };
                write_epoch_record(&spec.path, &value).await?;
                *record.lock() = Some(value);
                Ok(())
            }
        }
    }

    /// Best-effort removal; never fails the process pump.
    pub(crate) async fn cleanup(&self) {
        let result = match self {
            Self::Legacy { path, pid } => {
                let pid = pid.load(Ordering::Relaxed);
                if crate::os::get_pid_from_file(path).await != Some(pid) {
                    return;
                }
                tokio::fs::remove_file(path).await
            }
            Self::Epoch { spec, record, .. } => {
                let value = record.lock().clone();
                match value {
                    Some(value) => remove_record_if_matches(&spec.path, &value).await,
                    None => return,
                }
            }
        };
        if let Err(error) = result
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("failed to remove pid file: {error}");
        }
    }
}

/// Reads a structured epoch pid record. Numeric legacy files are rejected.
pub async fn read_epoch_pid_file(
    path: impl AsRef<Path>,
) -> std::io::Result<Option<EpochPidRecord>> {
    let path = path.as_ref();
    retry_record_io(|| read_epoch_pid_file_once(path)).await
}

async fn read_epoch_pid_file_once(path: &Path) -> std::io::Result<Option<EpochPidRecord>> {
    #[cfg(test)]
    tests::record_io_attempt(false)?;
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(invalid_input(format!(
                "pid file must not be a symlink: {}",
                path.display()
            )));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(invalid_input(format!(
                "pid file must be a regular file: {}",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let raw = tokio::fs::read(path).await?;
    let raw = String::from_utf8(raw).map_err(|_| invalid_data("epoch pid record is not UTF-8"))?;
    parse_epoch_record(&raw).map(Some)
}

/// Restricted to record filesystem IO; signaling a process is never retried.
async fn retry_record_io<T, F, Fut>(operation: F) -> std::io::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::io::Result<T>>,
{
    use backon::Retryable as _;
    operation
        .retry(crate::io::transient_backoff())
        .when(|error| crate::io::classify(error) == crate::io::IoFailure::Transient)
        .await
}

/// Read-only identity check for a copied epoch record. A missing pid, or one
/// whose start token differs from the record (reused), is dead. Inability to
/// inspect it, or a matching start token with a different executable, is an
/// error, never proof of death. This does not authorize killing a process
/// using a record in a user-owned directory.
pub fn recorded_process_is_alive(raw: &str) -> std::io::Result<bool> {
    let record = parse_epoch_record(raw)?;
    let Some(identity) = process_identity(record.pid)? else {
        return Ok(false);
    };
    match compare_identity(&record, &identity) {
        IdentityMatch::Same => Ok(true),
        IdentityMatch::Reused => Ok(false),
        IdentityMatch::Uncertain => Err(identity_error(format!(
            "cannot prove whether live pid {} is the recorded process",
            record.pid
        ))),
    }
}

/// Kills the orphan in `path` only after validating the full epoch record and
/// proving that both pid and runtime config are contained by `runtime_dir`.
///
/// A live pid whose start token differs from the record belongs to another
/// process: the record is removed and [`OrphanReapOutcome::AlreadyExited`] is
/// returned without signaling that process or its descendants. A matching
/// start token with a different or unreadable executable cannot be attributed
/// either way; it is an error and the record is kept.
///
/// Windows validates creation time and executable and terminates through the
/// same process handle. Linux uses a pidfd plus a boot-bound `/proc` start
/// token when the running kernel supports pidfds. Older Linux kernels and
/// other Unix targets revalidate immediately before signaling; those fallback
/// paths retain a minimal PID-reuse window because no portable process handle
/// is available through the crate's supported APIs.
///
/// Live descendants are captured before the recorded root is killed, and each
/// captured process is killed only while its own executable and start token
/// still match. Descendants that reparent or exit before the two enumeration
/// snapshots observe them cannot be attributed to the epoch and are
/// deliberately not killed; persistent group/job identity would be required
/// to close that gap.
///
/// # Errors
/// Returns [`OrphanReapError::RecordUnusable`] for invalid record data or paths,
/// [`OrphanReapError::ProcessUnconfirmed`] when ownership or exit is unproven,
/// and [`OrphanReapError::Io`] for filesystem failures. Record reads and removal
/// retry transient IO three times; signaling is never retried.
pub async fn reap_epoch_pid_file(
    path: impl AsRef<Path>,
    runtime_dir: impl AsRef<Path>,
) -> Result<OrphanReapOutcome, OrphanReapError> {
    let path = path.as_ref();
    let unusable = |source| OrphanReapError::RecordUnusable {
        path: path.to_owned(),
        source,
    };
    let io = |path: &Path, source| OrphanReapError::Io {
        path: path.to_owned(),
        source,
    };
    let record_error = |source: std::io::Error| {
        if is_invalid_record(&source) {
            unusable(source)
        } else {
            io(path, source)
        }
    };
    let runtime_dir = tokio::fs::canonicalize(runtime_dir.as_ref())
        .await
        .map_err(|source| io(runtime_dir.as_ref(), source))?;
    let parent = path
        .parent()
        .ok_or_else(|| unusable(invalid_input("pid file has no parent directory")))?;
    if tokio::fs::canonicalize(parent)
        .await
        .map_err(|source| io(parent, source))?
        != runtime_dir
    {
        return Err(unusable(invalid_input(
            "pid file escapes the runtime directory",
        )));
    }
    let epoch = epoch_from_file_name(path, "core-", ".pid").map_err(unusable)?;
    let Some(record) = read_epoch_pid_file(path).await.map_err(record_error)? else {
        return Ok(OrphanReapOutcome::NotFound);
    };
    if record.epoch != epoch {
        return Err(unusable(invalid_data(
            "pid filename and embedded epoch differ",
        )));
    }
    let runtime_parent = record
        .runtime_config
        .parent()
        .ok_or_else(|| unusable(invalid_input("runtime config has no parent directory")))?;
    let runtime_parent = tokio::fs::canonicalize(runtime_parent)
        .await
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                unusable(source)
            } else {
                io(runtime_parent, source)
            }
        })?;
    if runtime_parent != runtime_dir {
        return Err(unusable(invalid_input(
            "runtime config escapes the runtime directory",
        )));
    }
    let runtime_epoch =
        epoch_from_file_name(&record.runtime_config, "config-", ".yaml").map_err(unusable)?;
    if runtime_epoch != epoch {
        return Err(unusable(invalid_data(
            "runtime config filename and embedded epoch differ",
        )));
    }
    validate_runtime_target(&record.runtime_config)
        .await
        .map_err(|source| {
            if is_invalid_record(&source) {
                unusable(source)
            } else {
                io(&record.runtime_config, source)
            }
        })?;

    let outcome =
        reap_record(&record)
            .await
            .map_err(|source| OrphanReapError::ProcessUnconfirmed {
                path: path.to_owned(),
                pid: record.pid,
                source,
            })?;
    // Even malformed replacement data here is a cleanup failure after a possible kill,
    // not an unusable original record that can safely be discarded before signaling.
    remove_record_if_matches(path, &record)
        .await
        .map_err(|source| io(path, source))?;
    Ok(outcome)
}

async fn normalize_epoch_spec(spec: EpochPidFile) -> std::io::Result<EpochPidFile> {
    let pid_epoch = epoch_from_file_name(&spec.path, "core-", ".pid")?;
    let config_epoch = epoch_from_file_name(&spec.runtime_config, "config-", ".yaml")?;
    if pid_epoch != spec.epoch || config_epoch != spec.epoch {
        return Err(invalid_input(
            "epoch pid/config filenames must match the requested epoch",
        ));
    }
    let pid_parent = spec
        .path
        .parent()
        .ok_or_else(|| invalid_input("pid file has no parent directory"))?;
    let config_parent = spec
        .runtime_config
        .parent()
        .ok_or_else(|| invalid_input("runtime config has no parent directory"))?;
    let pid_parent = tokio::fs::canonicalize(pid_parent).await?;
    let config_parent = tokio::fs::canonicalize(config_parent).await?;
    if pid_parent != config_parent {
        return Err(invalid_input(
            "epoch pid and runtime config must share one directory",
        ));
    }
    validate_runtime_target(&spec.runtime_config).await?;
    Ok(EpochPidFile {
        path: pid_parent.join(format!("core-{}.pid", spec.epoch)),
        epoch: spec.epoch,
        runtime_config: config_parent.join(format!("config-{}.yaml", spec.epoch)),
    })
}

async fn validate_pid_target(path: &Path) -> std::io::Result<()> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(invalid_input(format!(
            "pid file must not be a symlink: {}",
            path.display()
        ))),
        Ok(metadata) if !metadata.is_file() => Err(invalid_input(format!(
            "pid file must be a regular file: {}",
            path.display()
        ))),
        Ok(metadata) if crate::io::atomic_fs::is_reparse_point(&metadata) => Err(invalid_input(
            format!("pid file must not be a reparse point: {}", path.display()),
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

async fn validate_runtime_target(path: &Path) -> std::io::Result<()> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(invalid_input(format!(
            "runtime config must not be a symlink: {}",
            path.display()
        ))),
        Ok(metadata) if !metadata.is_file() => Err(invalid_input(format!(
            "runtime config must be a regular file: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn validate_record_for_spec(
    record: &EpochPidRecord,
    spec: &EpochPidFile,
    expected_exe: &str,
) -> std::io::Result<()> {
    if record.epoch != spec.epoch {
        return Err(identity_error(format!(
            "pid file belongs to epoch {}, not {}",
            record.epoch, spec.epoch
        )));
    }
    if record.runtime_config != spec.runtime_config {
        return Err(identity_error(
            "pid record runtime config does not match the requested epoch",
        ));
    }
    if !exe_names_equal(&record.executable, expected_exe) {
        return Err(identity_error(format!(
            "pid record executable `{}` does not match `{expected_exe}`",
            record.executable
        )));
    }
    Ok(())
}

async fn reap_record(record: &EpochPidRecord) -> std::io::Result<OrphanReapOutcome> {
    let Some(identity) = identity_for_reap(record.pid).await? else {
        return Ok(OrphanReapOutcome::AlreadyExited);
    };
    match compare_identity(record, &identity) {
        IdentityMatch::Same => {}
        // Returned before capturing descendants: the tree under a reused pid
        // belongs to another process.
        IdentityMatch::Reused => return Ok(OrphanReapOutcome::AlreadyExited),
        IdentityMatch::Uncertain => {
            return Err(identity_error(format!(
                "cannot prove ownership of live pid {}",
                record.pid
            )));
        }
    }

    let descendants = capture_descendants(record.pid);
    let root_outcome =
        reap_record_with_kill(record, false, || kill_recorded_process(record)).await?;
    let mut killed_descendant = false;
    let mut failures = Vec::new();
    for descendant in descendants {
        // `attributable_identity` admits only identities with an executable.
        let Some(executable) = descendant.identity.executable else {
            continue;
        };
        let descendant_record = EpochPidRecord {
            pid: descendant.pid,
            epoch: record.epoch,
            executable,
            start_token: descendant.identity.start_token,
            runtime_config: record.runtime_config.clone(),
        };
        match reap_record_with_kill(&descendant_record, true, || {
            kill_recorded_process(&descendant_record)
        })
        .await
        {
            Ok(OrphanReapOutcome::Killed) => killed_descendant = true,
            Ok(OrphanReapOutcome::AlreadyExited | OrphanReapOutcome::NotFound) => {}
            Err(error) => failures.push(format!("pid {}: {error}", descendant_record.pid)),
        }
    }
    if !failures.is_empty() {
        return Err(std::io::Error::other(format!(
            "failed to confirm death of captured descendants: {}",
            failures.join("; ")
        )));
    }
    if killed_descendant {
        Ok(OrphanReapOutcome::Killed)
    } else {
        Ok(root_outcome)
    }
}

async fn reap_record_with_kill<K, F>(
    record: &EpochPidRecord,
    unowned_is_dead: bool,
    kill: K,
) -> std::io::Result<OrphanReapOutcome>
where
    K: FnOnce() -> F,
    F: std::future::Future<Output = std::io::Result<()>>,
{
    let identity = identity_for_confirmation(identity_for_reap(record.pid).await, unowned_is_dead)?;
    let Some(identity) = identity else {
        return Ok(OrphanReapOutcome::AlreadyExited);
    };
    match compare_identity(record, &identity) {
        IdentityMatch::Same => {}
        IdentityMatch::Reused => return Ok(OrphanReapOutcome::AlreadyExited),
        IdentityMatch::Uncertain if unowned_is_dead => {
            return Ok(OrphanReapOutcome::AlreadyExited);
        }
        IdentityMatch::Uncertain => {
            return Err(identity_error(format!(
                "cannot prove ownership of live pid {}",
                record.pid
            )));
        }
    }

    // Windows may report access denied when termination is already in flight.
    // A kill error is therefore provisional until the bounded identity window
    // proves that this exact process survived it.
    let kill_error = kill().await.err();
    let mut identity_error = None;
    for attempt in 0..KILL_WAIT_ATTEMPTS {
        match process_identity(record.pid) {
            Ok(None) => return Ok(OrphanReapOutcome::Killed),
            Ok(Some(identity)) if !record_matches_identity(record, &identity) => {
                return Ok(OrphanReapOutcome::Killed);
            }
            Ok(Some(_)) => identity_error = None,
            Err(error) if identity_query_is_provisional(&error) => {
                if unowned_is_dead {
                    return Ok(OrphanReapOutcome::Killed);
                }
                identity_error = Some(error);
            }
            Err(_) if unowned_is_dead => return Ok(OrphanReapOutcome::Killed),
            Err(error) => return Err(error),
        }
        if attempt + 1 < KILL_WAIT_ATTEMPTS {
            tokio::time::sleep(KILL_WAIT_DELAY).await;
        }
    }
    if let Some(error) = kill_error {
        return Err(error);
    }
    if let Some(error) = identity_error.take() {
        return Err(error);
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        format!("pid {} remained alive after validated kill", record.pid),
    ))
}

fn identity_for_confirmation(
    result: std::io::Result<Option<ProcessIdentity>>,
    unowned_is_dead: bool,
) -> std::io::Result<Option<ProcessIdentity>> {
    match result {
        Ok(identity) => Ok(identity),
        Err(_) if unowned_is_dead => Ok(None),
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
struct CapturedDescendant {
    pid: u32,
    identity: ProcessIdentity,
}

fn capture_descendants(root_pid: u32) -> Vec<CapturedDescendant> {
    let first = descendant_pids(root_pid);
    let mut first_identities = BTreeMap::new();
    for pid in first {
        if let Some(identity) = attributable_identity(process_identity(pid)) {
            first_identities.insert(pid, identity);
        }
    }

    let second = descendant_pids(root_pid);
    let mut second_identities = BTreeMap::new();
    for pid in second {
        second_identities.insert(pid, attributable_identity(process_identity(pid)));
    }
    merge_descendant_captures(first_identities, second_identities)
}

fn attributable_identity(
    result: std::io::Result<Option<ProcessIdentity>>,
) -> Option<ProcessIdentity> {
    result
        .ok()
        .flatten()
        .filter(|identity| identity.executable.is_some())
}

fn merge_descendant_captures(
    mut captured: BTreeMap<u32, ProcessIdentity>,
    second: BTreeMap<u32, Option<ProcessIdentity>>,
) -> Vec<CapturedDescendant> {
    for (pid, identity) in second {
        match identity {
            Some(identity) => {
                captured.insert(pid, identity);
            }
            None => {
                captured.remove(&pid);
            }
        }
    }
    captured
        .into_iter()
        .map(|(pid, identity)| CapturedDescendant { pid, identity })
        .collect()
}

fn descendant_pids(root_pid: u32) -> BTreeSet<u32> {
    #[cfg(test)]
    if let Some(pids) = tests::injected_descendants(root_pid) {
        return pids;
    }
    use sysinfo::{Pid, System};

    let system = System::new_all();
    let mut children = BTreeMap::<Pid, Vec<Pid>>::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }

    let mut descendants = BTreeSet::new();
    let mut pending = vec![Pid::from_u32(root_pid)];
    while let Some(parent) = pending.pop() {
        let Some(direct) = children.get(&parent) else {
            continue;
        };
        for pid in direct {
            let pid = pid.as_u32();
            if descendants.insert(pid) {
                pending.push(Pid::from_u32(pid));
            }
        }
    }
    descendants
}

async fn identity_for_reap(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    let mut access_error = None;
    for attempt in 0..IDENTITY_WAIT_ATTEMPTS {
        match process_identity(pid) {
            Ok(identity) => return Ok(identity),
            Err(error) if identity_query_is_provisional(&error) => {
                access_error = Some(error);
            }
            Err(error) => return Err(error),
        }
        if attempt + 1 < IDENTITY_WAIT_ATTEMPTS {
            tokio::time::sleep(IDENTITY_WAIT_DELAY).await;
        }
    }
    Err(access_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("cannot inspect live pid {pid}"),
        )
    }))
}

fn identity_query_is_provisional(error: &std::io::Error) -> bool {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        return true;
    }
    #[cfg(windows)]
    {
        // Windows can surface these while an already-open process handle is
        // transitioning to the terminated state.
        matches!(error.raw_os_error(), Some(6 | 31 | 87 | 1168))
    }
    #[cfg(not(windows))]
    false
}

/// Polls until `pid` reports `expected_exe`, returning the last identity seen
/// when it never does. A vfork-based spawn (glibc `posix_spawn`) resumes the
/// parent once the child releases the parent's mm, before exec installs the
/// new one, so `/proc/<pid>/exe` can briefly name the parent's binary. The
/// start token survives exec, so the matching identity is the spawned one.
async fn wait_for_process_identity(
    pid: u32,
    expected_exe: &str,
) -> std::io::Result<Option<ProcessIdentity>> {
    let mut access_error = None;
    let mut observed = None;
    for attempt in 0..IDENTITY_WAIT_ATTEMPTS {
        match process_identity(pid) {
            Ok(Some(identity)) => {
                if identity
                    .executable
                    .as_deref()
                    .is_some_and(|executable| exe_names_equal(executable, expected_exe))
                {
                    return Ok(Some(identity));
                }
                observed = Some(identity);
            }
            Ok(None) => {}
            Err(error) if identity_query_is_provisional(&error) => {
                access_error = Some(error);
            }
            Err(error) => return Err(error),
        }
        if attempt + 1 < IDENTITY_WAIT_ATTEMPTS {
            tokio::time::sleep(IDENTITY_WAIT_DELAY).await;
        }
    }
    match (observed, access_error) {
        (Some(identity), _) => Ok(Some(identity)),
        (None, Some(error)) => Err(error),
        (None, None) => Ok(None),
    }
}

fn process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    #[cfg(test)]
    if let Some(result) = tests::injected_identity(pid) {
        return result;
    }
    native_process_identity(pid)
}

async fn kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
    #[cfg(test)]
    if let Some(result) = tests::injected_kill(record.pid) {
        return result;
    }
    native_kill_recorded_process(record).await
}

/// Reads the executable's file name and start time through sysinfo. `None`
/// when sysinfo does not list the pid.
#[cfg(all(not(windows), not(target_os = "linux")))]
fn sysinfo_identity(pid: u32) -> Option<ProcessIdentity> {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind};

    let kind = RefreshKind::nothing()
        .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
    let mut system = System::new_with_specifics(kind);
    system.refresh_specifics(kind);
    let process = system.process(Pid::from_u32(pid))?;
    let executable = process
        .exe()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(str::to_owned);
    Some(ProcessIdentity {
        executable,
        start_token: process.start_time(),
    })
}

#[cfg(all(not(windows), not(target_os = "linux"), not(target_os = "macos")))]
fn native_process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    Ok(sysinfo_identity(pid))
}

/// macOS takes the start token from `sysctl(KERN_PROC_PID)`, which reports
/// processes of every uid. sysinfo reads it through `PROC_PIDTBSDINFO`, which
/// is refused for another uid; sysinfo then reports a start time of 0, which
/// cannot tell a reused pid from an unreadable one. Both read the kernel's
/// `p_start.tv_sec`, so records written from sysinfo's value still compare.
/// The executable still comes from sysinfo, the source the records were
/// written with.
#[cfg(target_os = "macos")]
fn native_process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    let Some(start_token) = darwin::start_seconds(pid)? else {
        return Ok(None);
    };
    let executable = sysinfo_identity(pid).and_then(|identity| identity.executable);
    // A pid that changed hands while the name was read would mix two
    // processes into one identity.
    if darwin::start_seconds(pid)? != Some(start_token) {
        return Ok(None);
    }
    Ok(Some(ProcessIdentity {
        executable,
        start_token,
    }))
}

#[cfg(target_os = "macos")]
mod darwin {
    use std::mem::size_of;

    // FFI-BINDING: `libc` does not bind Darwin's `struct kinfo_proc` (declared
    // in the public SDK header <sys/sysctl.h>), so this declares the prefix
    // that is read. `sysctl(KERN_PROC_PID)` is the only public interface that
    // returns the start time of another uid's process:
    // `proc_pidinfo(PROC_PIDTBSDINFO)` and `proc_pid_rusage` refuse other uids
    // with EPERM, `PROC_PIDT_SHORTBSDINFO` carries no start time, and
    // `PROC_PIDUNIQIDENTIFIERINFO` is Apple-private.
    // Layout (XNU `struct user64_extern_proc`, bsd/sys/proc_internal.h, the
    // record a 64-bit caller receives on arm64 and x86_64): the start time is
    // the `timeval` member of the leading `p_un` union at offset 0, followed
    // by two pointers, `p_flag`, `p_stat` and `p_pid`. The kernel sizes the
    // buffer and the returned length is checked before the prefix is read.
    // The tests compare the decoded start time with sysinfo's and the pid with
    // the requested one.
    // Removal: use `libc::kinfo_proc` once the `libc` crate binds it.
    #[repr(C)]
    struct KinfoProcPrefix {
        start_time: libc::timeval,
        _vmspace: *mut libc::c_void,
        _sigacts: *mut libc::c_void,
        _flag: libc::c_int,
        stat: libc::c_char,
        pid: libc::pid_t,
    }

    // Offsets of `struct user64_extern_proc` on both 64-bit targets.
    const _: () = assert!(std::mem::offset_of!(KinfoProcPrefix, start_time) == 0);
    const _: () = assert!(std::mem::offset_of!(KinfoProcPrefix, stat) == 36);
    const _: () = assert!(std::mem::offset_of!(KinfoProcPrefix, pid) == 40);

    /// `SZOMB` in bsd/sys/proc.h: exited, waiting for its parent.
    const SZOMB: libc::c_char = 5;

    /// The start time in seconds of a live process, or `None` when the pid
    /// is free or a zombie.
    pub(super) fn start_seconds(pid: u32) -> std::io::Result<Option<u64>> {
        let pid = libc::pid_t::try_from(pid)
            .map_err(|_| super::invalid_input(format!("pid {pid} is out of range")))?;
        let mut name = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_PID, pid];
        let mut len: libc::size_t = 0;
        // SAFETY: `name` holds four initialized ints and its length is passed;
        // a null old pointer with a valid length pointer asks only for the
        // buffer size, and no new value is written.
        if unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                name.len() as libc::c_uint,
                std::ptr::null_mut(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        // `u64` elements keep the record 8-byte aligned.
        let mut buffer = vec![0_u64; len.div_ceil(size_of::<u64>())];
        let mut len = buffer.len() * size_of::<u64>();
        // SAFETY: `buffer` is writable for `len` bytes, `len` is a valid
        // in/out pointer, and `name` is as above.
        if unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                name.len() as libc::c_uint,
                buffer.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        if len == 0 {
            return Ok(None);
        }
        if len < size_of::<KinfoProcPrefix>() {
            return Err(super::invalid_data(format!(
                "kinfo_proc for pid {pid} is truncated to {len} bytes"
            )));
        }
        // SAFETY: the kernel wrote at least `size_of::<KinfoProcPrefix>()`
        // bytes, the buffer is 8-byte aligned, and every field is an integer or
        // a raw pointer, valid for any bit pattern.
        let record = unsafe { buffer.as_ptr().cast::<KinfoProcPrefix>().read() };
        if record.pid != pid {
            return Err(super::invalid_data(format!(
                "kinfo_proc for pid {pid} describes pid {}",
                record.pid
            )));
        }
        if record.stat == SZOMB {
            return Ok(None);
        }
        u64::try_from(record.start_time.tv_sec)
            .map(Some)
            .map_err(|_| super::invalid_data(format!("negative start time for pid {pid}")))
    }
}

#[cfg(target_os = "linux")]
fn native_process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    let Some(first_ticks) = linux_start_ticks(pid)? else {
        return Ok(None);
    };
    // `/proc/<pid>/stat` is world-readable, `/proc/<pid>/exe` is not: another
    // user's process has a start token but no readable executable.
    let executable_path = match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(path) => Some(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => None,
        Err(error) => return Err(error),
    };
    let Some(second_ticks) = linux_start_ticks(pid)? else {
        return Ok(None);
    };
    if first_ticks != second_ticks {
        return Ok(None);
    }
    let executable = executable_path
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .ok_or_else(|| {
                    identity_error(format!("cannot resolve executable for live pid {pid}"))
                })
        })
        .transpose()?;
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    Ok(Some(ProcessIdentity {
        executable,
        start_token: boot_bound_start_token(boot_id.trim(), first_ticks),
    }))
}

#[cfg(target_os = "linux")]
fn linux_start_ticks(pid: u32) -> std::io::Result<Option<u64>> {
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let command_end = stat
        .rfind(')')
        .ok_or_else(|| invalid_data(format!("malformed /proc/{pid}/stat")))?;
    let ticks = stat[command_end + 1..]
        .split_whitespace()
        .nth(19)
        .ok_or_else(|| invalid_data(format!("missing start time in /proc/{pid}/stat")))?
        .parse()
        .map_err(|_| invalid_data(format!("invalid start time in /proc/{pid}/stat")))?;
    Ok(Some(ticks))
}

#[cfg(target_os = "linux")]
fn boot_bound_start_token(boot_id: &str, ticks: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in boot_id.bytes().chain(ticks.to_le_bytes()) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(windows)]
fn native_process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    use windows::Win32::System::Threading::{
        PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const PROCESS_SYNCHRONIZE: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(0x0010_0000);

    let handle = match open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE) {
        Ok(handle) => handle,
        Err(error) if error.raw_os_error() == Some(87) => return Ok(None),
        Err(error) => return Err(error),
    };
    match process_identity_from_handle(&handle, pid) {
        Ok(identity) => Ok(Some(identity)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(windows)]
struct OwnedProcessHandle(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for OwnedProcessHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper uniquely owns the handle returned by OpenProcess.
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(windows)]
fn open_process(
    pid: u32,
    access: windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS,
) -> std::io::Result<OwnedProcessHandle> {
    // SAFETY: pid and access flags are plain values; handle ownership is
    // immediately transferred to OwnedProcessHandle.
    unsafe { windows::Win32::System::Threading::OpenProcess(access, false, pid) }
        .map(OwnedProcessHandle)
        .map_err(windows_io_error)
}

#[cfg(windows)]
fn process_identity_from_handle(
    handle: &OwnedProcessHandle,
    pid: u32,
) -> std::io::Result<ProcessIdentity> {
    use std::os::windows::ffi::OsStringExt;
    use windows::{
        Win32::{
            Foundation::FILETIME,
            System::Threading::{GetProcessTimes, PROCESS_NAME_WIN32, QueryFullProcessImageNameW},
        },
        core::PWSTR,
    };

    if process_handle_is_terminated(handle)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pid {pid} has terminated"),
        ));
    }

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all output pointers reference initialized writable FILETIME values.
    unsafe { GetProcessTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user) }
        .map_err(windows_io_error)?;

    let mut executable = vec![0_u16; 32_768];
    let mut len = executable.len() as u32;
    // SAFETY: the buffer is writable for len UTF-16 code units and len remains
    // live for the call.
    unsafe {
        QueryFullProcessImageNameW(
            handle.0,
            PROCESS_NAME_WIN32,
            PWSTR(executable.as_mut_ptr()),
            &mut len,
        )
    }
    .map_err(windows_io_error)?;
    let path = PathBuf::from(std::ffi::OsString::from_wide(&executable[..len as usize]));
    let executable = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| identity_error(format!("cannot resolve executable for live pid {pid}")))?;
    Ok(ProcessIdentity {
        executable: Some(executable.to_owned()),
        start_token: (u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime),
    })
}

#[cfg(windows)]
fn process_handle_is_terminated(handle: &OwnedProcessHandle) -> std::io::Result<bool> {
    use windows::Win32::{
        Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::WaitForSingleObject,
    };

    // SAFETY: handle is live and a zero timeout performs a nonblocking query.
    match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(std::io::Error::last_os_error()),
    }
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

#[cfg(windows)]
async fn native_kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
    use windows::Win32::System::Threading::{
        PROCESS_ACCESS_RIGHTS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        TerminateProcess,
    };

    const PROCESS_SYNCHRONIZE: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(0x0010_0000);

    let handle = open_process(
        record.pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
    )?;
    let identity = process_identity_from_handle(&handle, record.pid)?;
    if !record_matches_identity(record, &identity) {
        return Err(identity_error(format!(
            "cannot prove ownership of live pid {}",
            record.pid
        )));
    }
    // SAFETY: identity was read from this same open handle, which remains live
    // for the termination call.
    unsafe { TerminateProcess(handle.0, 1) }.map_err(windows_io_error)
}

#[cfg(target_os = "linux")]
async fn native_kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
    let pidfd = match LinuxPidFd::open(record.pid) {
        Ok(pidfd) => pidfd,
        Err(error) if error.raw_os_error() == Some(libc::ENOSYS) => {
            return kill_revalidated_by_pid(record);
        }
        Err(error) => return Err(error),
    };
    let identity = process_identity(record.pid)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pid {} exited before pidfd validation", record.pid),
        )
    })?;
    if !record_matches_identity(record, &identity) {
        return Err(identity_error(format!(
            "cannot prove ownership of live pid {}",
            record.pid
        )));
    }
    pidfd.kill()
}

#[cfg(target_os = "linux")]
struct LinuxPidFd(std::os::fd::RawFd);

#[cfg(target_os = "linux")]
impl LinuxPidFd {
    fn open(pid: u32) -> std::io::Result<Self> {
        // SAFETY: pidfd_open receives a numeric pid and zero flags.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0_u32) };
        if fd < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(Self(fd as std::os::fd::RawFd))
        }
    }

    fn kill(&self) -> std::io::Result<()> {
        // SAFETY: self.0 is a live pidfd and the optional siginfo pointer is null.
        let result = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0,
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            )
        };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error)
            }
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for LinuxPidFd {
    fn drop(&mut self) {
        // SAFETY: self uniquely owns this pidfd.
        unsafe { libc::close(self.0) };
    }
}

#[cfg(unix)]
fn kill_revalidated_by_pid(record: &EpochPidRecord) -> std::io::Result<()> {
    let identity = process_identity(record.pid)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pid {} exited before final validation", record.pid),
        )
    })?;
    if !record_matches_identity(record, &identity) {
        return Err(identity_error(format!(
            "cannot prove ownership of live pid {}",
            record.pid
        )));
    }
    // SAFETY: the exact identity was revalidated immediately before signaling.
    if unsafe { libc::kill(record.pid as libc::pid_t, libc::SIGKILL) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
async fn native_kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
    kill_revalidated_by_pid(record)
}

#[cfg(not(any(unix, windows)))]
async fn native_kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
    let identity = process_identity(record.pid)?.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("pid {} exited before final validation", record.pid),
        )
    })?;
    if !record_matches_identity(record, &identity) {
        return Err(identity_error(format!(
            "cannot prove ownership of live pid {}",
            record.pid
        )));
    }
    crate::os::kill_pid::<String>(record.pid, None).await
}

fn compare_identity(record: &EpochPidRecord, identity: &ProcessIdentity) -> IdentityMatch {
    if record.start_token != identity.start_token {
        return IdentityMatch::Reused;
    }
    match &identity.executable {
        Some(observed) if recorded_executable_matches(&record.executable, observed) => {
            IdentityMatch::Same
        }
        _ => IdentityMatch::Uncertain,
    }
}

fn record_matches_identity(record: &EpochPidRecord, identity: &ProcessIdentity) -> bool {
    compare_identity(record, identity) == IdentityMatch::Same
}

/// Compares a recorded executable name with a live observation of a process
/// whose start token already matches.
fn recorded_executable_matches(recorded: &str, observed: &str) -> bool {
    if exe_names_equal(recorded, observed) {
        return true;
    }
    // Linux reports an executable unlinked after exec, such as a core binary
    // replaced by an upgrade, as `<name> (deleted)`. The start token already
    // identifies the process; only that exact suffix is tolerated.
    #[cfg(target_os = "linux")]
    if let Some(observed) = observed.strip_suffix(" (deleted)") {
        return exe_names_equal(recorded, observed);
    }
    false
}

#[cfg(windows)]
fn exe_names_equal(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(not(windows))]
fn exe_names_equal(left: &str, right: &str) -> bool {
    left == right
}

async fn write_epoch_record(path: &Path, record: &EpochPidRecord) -> std::io::Result<()> {
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    validate_pid_target(path).await?;
    if tokio::fs::try_exists(path).await? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("pid file unexpectedly exists: {}", path.display()),
        ));
    }
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!("pid.tmp-{}-{counter}", std::process::id()));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .await?;
        file.write_all(serialize_epoch_record(record)?.as_bytes())
            .await?;
        file.flush().await?;
        file.sync_all().await?;
        drop(file);
        // The shared publisher fails atomically if the destination appeared
        // after the `try_exists` check above. The staging path shares the
        // destination directory, as required by both platform implementations.
        crate::io::atomic_fs::atomic_move_new(&temp, path)
            .await
            .map_err(|error| match error {
                crate::io::atomic_fs::AtomicFsError::Io(error)
                    if error.kind() == std::io::ErrorKind::AlreadyExists =>
                {
                    std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        format!("pid file unexpectedly exists: {}", path.display()),
                    )
                }
                crate::io::atomic_fs::AtomicFsError::Io(error) => error,
                crate::io::atomic_fs::AtomicFsError::UnsafePath(path)
                | crate::io::atomic_fs::AtomicFsError::Contended(path) => std::io::Error::other(
                    format!("unexpected atomic filesystem error for {}", path.display()),
                ),
            })
    }
    .await;
    // Publication consumes `temp` on success; on failure it remains and must
    // be cleaned up here.
    let _ = tokio::fs::remove_file(&temp).await;
    result
}

async fn remove_record_if_matches(path: &Path, expected: &EpochPidRecord) -> std::io::Result<()> {
    retry_record_io(|| remove_record_if_matches_once(path, expected)).await
}

async fn remove_record_if_matches_once(
    path: &Path,
    expected: &EpochPidRecord,
) -> std::io::Result<()> {
    // Retry the identity-bound read and removal together, without nested retries.
    if read_epoch_pid_file_once(path).await?.as_ref() != Some(expected) {
        return Ok(());
    }
    #[cfg(test)]
    tests::record_io_attempt(true)?;
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn serialize_epoch_record(record: &EpochPidRecord) -> std::io::Result<String> {
    let runtime_config = record.runtime_config.to_str().ok_or_else(|| {
        invalid_input("runtime config path must be UTF-8 for an epoch pid record")
    })?;
    Ok(format!(
        "version={EPOCH_PID_VERSION}\npid={}\nepoch={}\nexecutable={}\nstart-token={}\nruntime-config={}\n",
        record.pid,
        record.epoch,
        hex_encode(record.executable.as_bytes()),
        record.start_token,
        hex_encode(runtime_config.as_bytes()),
    ))
}

fn parse_epoch_record(raw: &str) -> std::io::Result<EpochPidRecord> {
    let mut fields = BTreeMap::new();
    for line in raw.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| invalid_data("malformed epoch pid record line"))?;
        if fields.insert(key, value).is_some() {
            return Err(invalid_data("duplicate epoch pid record field"));
        }
    }
    let expected = [
        "epoch",
        "executable",
        "pid",
        "runtime-config",
        "start-token",
        "version",
    ];
    if fields.len() != expected.len() || !expected.iter().all(|key| fields.contains_key(key)) {
        return Err(invalid_data("epoch pid record fields are incomplete"));
    }
    let version = parse_field::<u32>(&fields, "version")?;
    if version != EPOCH_PID_VERSION {
        return Err(invalid_data(format!(
            "unsupported epoch pid record version {version}"
        )));
    }
    let executable = String::from_utf8(hex_decode(required(&fields, "executable")?)?)
        .map_err(|_| invalid_data("epoch pid executable is not UTF-8"))?;
    let runtime_config = String::from_utf8(hex_decode(required(&fields, "runtime-config")?)?)
        .map_err(|_| invalid_data("epoch pid runtime path is not UTF-8"))?;
    Ok(EpochPidRecord {
        pid: parse_field(&fields, "pid")?,
        epoch: parse_field(&fields, "epoch")?,
        executable,
        start_token: parse_field(&fields, "start-token")?,
        runtime_config: PathBuf::from(runtime_config),
    })
}

fn required<'a>(fields: &'a BTreeMap<&str, &str>, key: &str) -> std::io::Result<&'a str> {
    fields
        .get(key)
        .copied()
        .ok_or_else(|| invalid_data(format!("missing epoch pid field `{key}`")))
}

fn parse_field<T: std::str::FromStr>(
    fields: &BTreeMap<&str, &str>,
    key: &str,
) -> std::io::Result<T> {
    required(fields, key)?
        .parse()
        .map_err(|_| invalid_data(format!("invalid epoch pid field `{key}`")))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_decode(value: &str) -> std::io::Result<Vec<u8>> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(invalid_data("hex field has odd length"));
    }
    pairs
        .iter()
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> std::io::Result<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(invalid_data("invalid hex field")),
    }
}

fn epoch_from_file_name(path: &Path, prefix: &str, suffix: &str) -> std::io::Result<u64> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_input("epoch artifact filename must be UTF-8"))?;
    let epoch = name
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .ok_or_else(|| invalid_input(format!("invalid epoch artifact name `{name}`")))?;
    epoch
        .parse()
        .map_err(|_| invalid_input(format!("invalid epoch in artifact name `{name}`")))
}

// Keep validation failures distinct from OS errors with the same ErrorKind.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct InvalidRecord(String);

fn is_invalid_record(error: &std::io::Error) -> bool {
    error
        .get_ref()
        .is_some_and(|source| source.is::<InvalidRecord>())
}

fn invalid_input(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        InvalidRecord(message.into()),
    )
}

fn invalid_data(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        InvalidRecord(message.into()),
    )
}

fn identity_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{cell::RefCell, collections::VecDeque, io::ErrorKind};

    tokio::task_local! {
        static FAULTS: RefCell<Faults>;
    }

    #[derive(Default)]
    struct Faults {
        reads: VecDeque<Option<std::io::Error>>,
        removals: VecDeque<Option<std::io::Error>>,
        read_attempts: usize,
        remove_attempts: usize,
        processes: BTreeMap<u32, FakeProcess>,
    }

    struct FakeProcess {
        identity: ProcessIdentity,
        denied: bool,
        survives: bool,
        signals: usize,
    }

    pub(super) fn record_io_attempt(removing: bool) -> std::io::Result<()> {
        FAULTS
            .try_with(|faults| {
                let mut faults = faults.borrow_mut();
                let errors = if removing {
                    faults.remove_attempts += 1;
                    &mut faults.removals
                } else {
                    faults.read_attempts += 1;
                    &mut faults.reads
                };
                match errors.pop_front().flatten() {
                    Some(error) => Err(error),
                    None => Ok(()),
                }
            })
            .unwrap_or(Ok(()))
    }

    pub(super) fn injected_identity(pid: u32) -> Option<std::io::Result<Option<ProcessIdentity>>> {
        FAULTS
            .try_with(|faults| {
                let faults = faults.borrow();
                let process = faults.processes.get(&pid)?;
                Some(if process.denied {
                    Err(ErrorKind::PermissionDenied.into())
                } else if process.signals > 0 && !process.survives {
                    Ok(None)
                } else {
                    Ok(Some(process.identity.clone()))
                })
            })
            .ok()
            .flatten()
    }

    pub(super) fn injected_kill(pid: u32) -> Option<std::io::Result<()>> {
        FAULTS
            .try_with(|faults| {
                let mut faults = faults.borrow_mut();
                let process = faults.processes.get_mut(&pid)?;
                process.signals += 1;
                Some(Ok(()))
            })
            .ok()
            .flatten()
    }

    pub(super) fn injected_descendants(pid: u32) -> Option<BTreeSet<u32>> {
        FAULTS
            .try_with(|faults| {
                let faults = faults.borrow();
                faults.processes.contains_key(&pid).then(|| {
                    faults
                        .processes
                        .keys()
                        .copied()
                        .filter(|child| *child != pid)
                        .collect()
                })
            })
            .ok()
            .flatten()
    }

    fn reap_fixture() -> (tempfile::TempDir, PathBuf, EpochPidRecord, Faults) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("core-1.pid");
        let record = EpochPidRecord {
            pid: u32::MAX - 1,
            epoch: 1,
            executable: "test-core".into(),
            start_token: 7,
            runtime_config: dir.path().join("config-1.yaml"),
        };
        std::fs::write(&path, serialize_epoch_record(&record).unwrap()).unwrap();
        std::fs::write(&record.runtime_config, "").unwrap();
        let mut faults = Faults::default();
        faults.processes.insert(
            record.pid,
            FakeProcess {
                identity: observed(Some(&record.executable), record.start_token),
                denied: false,
                survives: false,
                signals: 0,
            },
        );
        (dir, path, record, faults)
    }

    #[tokio::test(start_paused = true)]
    async fn public_reap_classifies_unusable_records_before_signaling() {
        for case in [
            "garbage",
            "utf8",
            "version",
            "epoch",
            "missing-parent",
            "no-parent",
            "escape",
            "config-epoch",
            "config-directory",
            "pid-directory",
        ] {
            let (dir, path, mut record, faults) = reap_fixture();
            let outside = tempfile::tempdir().unwrap();
            match case {
                "missing-parent" => record.runtime_config = dir.path().join("absent/config-1.yaml"),
                "no-parent" => record.runtime_config = PathBuf::new(),
                "escape" => record.runtime_config = outside.path().join("config-1.yaml"),
                "config-epoch" => record.runtime_config = dir.path().join("config-2.yaml"),
                "config-directory" => {
                    std::fs::remove_file(&record.runtime_config).unwrap();
                    std::fs::create_dir(&record.runtime_config).unwrap();
                }
                "epoch" => record.epoch = 2,
                _ => {}
            }
            let raw = match case {
                "garbage" => b"garbage".to_vec(),
                "utf8" => vec![0xff],
                "version" => serialize_epoch_record(&record)
                    .unwrap()
                    .replace("version=2", "version=3")
                    .into_bytes(),
                _ => serialize_epoch_record(&record).unwrap().into_bytes(),
            };
            std::fs::write(&path, raw).unwrap();
            if case == "pid-directory" {
                std::fs::remove_file(&path).unwrap();
                std::fs::create_dir(&path).unwrap();
            }
            FAULTS
                .scope(RefCell::new(faults), async {
                    let error = reap_epoch_pid_file(&path, dir.path()).await.unwrap_err();
                    assert!(
                        matches!(error, OrphanReapError::RecordUnusable { .. }),
                        "{case}: {error:?}"
                    );
                    assert!(path.exists(), "{case}");
                    FAULTS.with(|f| {
                        assert_eq!(f.borrow().processes[&record.pid].signals, 0, "{case}")
                    });
                })
                .await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn public_reap_classifies_identity_query_denial_without_signaling() {
        let (dir, path, record, mut faults) = reap_fixture();
        faults.processes.get_mut(&record.pid).unwrap().denied = true;
        FAULTS
            .scope(RefCell::new(faults), async {
                let error = reap_epoch_pid_file(&path, dir.path()).await.unwrap_err();
                assert!(
                    matches!(error, OrphanReapError::ProcessUnconfirmed { source, .. }
                if source.kind() == ErrorKind::PermissionDenied)
                );
                assert!(path.exists());
                FAULTS.with(|f| assert_eq!(f.borrow().processes[&record.pid].signals, 0));
            })
            .await;
    }

    #[tokio::test(start_paused = true)]
    async fn public_reap_classifies_exit_unconfirmed_after_one_signal() {
        for descendant in [false, true] {
            let (dir, path, record, mut faults) = reap_fixture();
            let survivor = if descendant {
                record.pid - 1
            } else {
                record.pid
            };
            faults.processes.insert(
                survivor,
                FakeProcess {
                    identity: observed(Some(&record.executable), record.start_token),
                    denied: false,
                    survives: true,
                    signals: 0,
                },
            );
            FAULTS
                .scope(RefCell::new(faults), async {
                    let error = reap_epoch_pid_file(&path, dir.path()).await.unwrap_err();
                    assert!(matches!(error, OrphanReapError::ProcessUnconfirmed { .. }));
                    assert!(path.exists());
                    FAULTS.with(|f| {
                        let f = f.borrow();
                        assert_eq!(f.processes[&record.pid].signals, 1);
                        assert_eq!(f.processes[&survivor].signals, 1);
                    });
                })
                .await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn public_reap_retries_record_io_without_repeating_signals() {
        let kinds = [
            ErrorKind::Interrupted,
            ErrorKind::WouldBlock,
            ErrorKind::TimedOut,
            ErrorKind::ResourceBusy,
        ];
        // raw Windows sharing/lock errors may map to PermissionDenied;
        // they must be recognized before the permission rule.
        for index in 0..kinds.len() + if cfg!(windows) { 2 } else { 0 } {
            let kind = kinds
                .get(index)
                .copied()
                .unwrap_or(ErrorKind::PermissionDenied);
            for removing in [false, true] {
                for exhausted in [false, true] {
                    let (dir, path, record, mut faults) = reap_fixture();
                    let errors = (0..if exhausted { 4 } else { 3 })
                        .map(|_| {
                            #[cfg(windows)]
                            if index >= 4 {
                                return Some(std::io::Error::from_raw_os_error(
                                    32 + (index - 4) as i32,
                                ));
                            }
                            let _ = index;
                            Some(std::io::Error::from(kind))
                        })
                        .collect();
                    if removing {
                        faults.removals = errors;
                    } else {
                        faults.reads = errors;
                    }
                    FAULTS
                        .scope(RefCell::new(faults), async {
                            let start = tokio::time::Instant::now();
                            let result = reap_epoch_pid_file(&path, dir.path()).await;
                            assert_eq!(start.elapsed(), Duration::from_millis(350));
                            if exhausted {
                                assert!(
                                    matches!(result, Err(OrphanReapError::Io { .. })),
                                    "{result:?}"
                                );
                                assert!(path.exists());
                            } else {
                                assert_eq!(result.unwrap(), OrphanReapOutcome::Killed);
                                assert!(!path.exists());
                            }
                            FAULTS.with(|f| {
                                let f = f.borrow();
                                assert_eq!(
                                    f.processes[&record.pid].signals,
                                    usize::from(removing || !exhausted)
                                );
                                if removing {
                                    assert_eq!(f.remove_attempts, 4);
                                } else {
                                    assert_eq!(f.read_attempts, if exhausted { 4 } else { 5 });
                                }
                            });
                        })
                        .await;
                }
            }
        }
    }

    #[tokio::test(start_paused = true)]
    async fn public_reap_does_not_retry_or_degrade_nontransient_io() {
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::InvalidInput,
            ErrorKind::InvalidData,
            ErrorKind::Other,
        ] {
            for removing in [false, true] {
                let (dir, path, record, mut faults) = reap_fixture();
                if removing {
                    faults.removals.push_back(Some(kind.into()));
                } else {
                    faults.reads.push_back(Some(kind.into()));
                }
                FAULTS.scope(RefCell::new(faults), async {
                    let start = tokio::time::Instant::now();
                    let error = reap_epoch_pid_file(&path, dir.path()).await.unwrap_err();
                    assert!(matches!(error, OrphanReapError::Io { source, .. } if source.kind() == kind));
                    assert_eq!(start.elapsed(), Duration::ZERO);
                    assert!(path.exists());
                    FAULTS.with(|f| {
                        let f = f.borrow();
                        assert_eq!(f.processes[&record.pid].signals, usize::from(removing));
                        assert_eq!(f.remove_attempts, usize::from(removing));
                        assert_eq!(f.read_attempts, if removing { 2 } else { 1 });
                    });
                }).await;
            }
        }
    }

    #[tokio::test]
    async fn public_reap_runtime_directory_io_keeps_source_and_path() {
        let (dir, path, _, _) = reap_fixture();
        let missing = dir.path().join("missing");
        let error = reap_epoch_pid_file(&path, &missing).await.unwrap_err();
        assert!(matches!(error, OrphanReapError::Io { path, source }
            if path == missing && source.kind() == ErrorKind::NotFound));
    }

    #[tokio::test(start_paused = true)]
    async fn public_reap_cleanup_read_failure_is_io_after_signal() {
        let (dir, path, record, mut faults) = reap_fixture();
        faults.reads = [None, Some(invalid_data("replacement record is malformed"))].into();
        FAULTS
            .scope(RefCell::new(faults), async {
                let error = reap_epoch_pid_file(&path, dir.path()).await.unwrap_err();
                assert!(matches!(error, OrphanReapError::Io { .. }));
                assert!(path.exists());
                FAULTS.with(|f| assert_eq!(f.borrow().processes[&record.pid].signals, 1));
            })
            .await;
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn public_reap_acl_denied_record_is_permission_io() {
        let (dir, path, _, _) = reap_fixture();
        let account = std::process::Command::new("whoami").output().unwrap();
        assert!(account.status.success());
        let account = String::from_utf8(account.stdout).unwrap();
        let account = account.trim();
        let deny = std::process::Command::new("icacls")
            .arg(&path)
            .args(["/deny", &format!("{account}:(R)")])
            .output()
            .unwrap();
        assert!(deny.status.success(), "{deny:?}");
        let result = reap_epoch_pid_file(&path, dir.path()).await;
        let restore = std::process::Command::new("icacls")
            .arg(&path)
            .args(["/remove:d", account])
            .output()
            .unwrap();
        assert!(restore.status.success(), "{restore:?}");
        assert!(matches!(result, Err(OrphanReapError::Io { source, .. })
            if source.kind() == ErrorKind::PermissionDenied));
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn public_reap_unreadable_record_is_permission_io() {
        use std::os::unix::fs::PermissionsExt;
        if nix::unistd::geteuid().is_root() {
            eprintln!("skipped: root bypasses Unix file permissions");
            return;
        }
        let (dir, path, _, _) = reap_fixture();
        let original = std::fs::metadata(&path).unwrap().permissions();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
        let result = reap_epoch_pid_file(&path, dir.path()).await;
        std::fs::set_permissions(&path, original).unwrap();
        assert!(matches!(result, Err(OrphanReapError::Io { source, .. })
            if source.kind() == ErrorKind::PermissionDenied));
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[tokio::test(start_paused = true)]
    async fn public_reap_rejects_symlink_records_and_runtime_targets() {
        for pid_link in [false, true] {
            let (dir, path, record, faults) = reap_fixture();
            let link = if pid_link {
                &path
            } else {
                &record.runtime_config
            };
            let target = dir.path().join("target");
            std::fs::rename(link, &target).unwrap();
            std::os::unix::fs::symlink(&target, link).unwrap();
            FAULTS
                .scope(RefCell::new(faults), async {
                    assert!(matches!(
                        reap_epoch_pid_file(&path, dir.path()).await,
                        Err(OrphanReapError::RecordUnusable { .. })
                    ));
                    assert!(link.is_symlink());
                    FAULTS.with(|f| assert_eq!(f.borrow().processes[&record.pid].signals, 0));
                })
                .await;
        }
    }

    #[test]
    fn copied_record_checks_identity_without_killing_the_process() {
        let pid = std::process::id();
        let identity = process_identity(pid).unwrap().unwrap();
        let mut record = EpochPidRecord {
            pid,
            epoch: 1,
            executable: identity.executable.unwrap(),
            start_token: identity.start_token,
            runtime_config: PathBuf::from("config-1.yaml"),
        };
        assert!(recorded_process_is_alive(&serialize_epoch_record(&record).unwrap()).unwrap());
        let mut renamed = record.clone();
        renamed.executable = "another-name".into();
        assert_eq!(
            recorded_process_is_alive(&serialize_epoch_record(&renamed).unwrap())
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied,
            "a matching start token with another executable is not proof of death"
        );
        record.start_token = record.start_token.wrapping_add(1);
        assert!(!recorded_process_is_alive(&serialize_epoch_record(&record).unwrap()).unwrap());
        assert!(recorded_process_is_alive("incomplete").is_err());
    }

    fn record_for(executable: &str, start_token: u64) -> EpochPidRecord {
        EpochPidRecord {
            pid: 1,
            epoch: 1,
            executable: executable.into(),
            start_token,
            runtime_config: PathBuf::from("config-1.yaml"),
        }
    }

    fn observed(executable: Option<&str>, start_token: u64) -> ProcessIdentity {
        ProcessIdentity {
            executable: executable.map(str::to_owned),
            start_token,
        }
    }

    #[test]
    fn start_token_decides_reuse_before_the_executable_is_needed() {
        let record = record_for("mihomo", 7);
        assert_eq!(
            compare_identity(&record, &observed(Some("mihomo"), 7)),
            IdentityMatch::Same
        );
        assert_eq!(
            compare_identity(&record, &observed(Some("launchd"), 8)),
            IdentityMatch::Reused
        );
        assert_eq!(
            compare_identity(&record, &observed(None, 8)),
            IdentityMatch::Reused,
            "another user's process has an unreadable executable but a readable start token"
        );
        assert_eq!(
            compare_identity(&record, &observed(Some("other"), 7)),
            IdentityMatch::Uncertain
        );
        assert_eq!(
            compare_identity(&record, &observed(None, 7)),
            IdentityMatch::Uncertain
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn deleted_executable_suffix_matches_only_with_an_equal_start_token() {
        let record = record_for("mihomo", 7);
        assert_eq!(
            compare_identity(&record, &observed(Some("mihomo (deleted)"), 7)),
            IdentityMatch::Same
        );
        assert_eq!(
            compare_identity(&record, &observed(Some("other (deleted)"), 7)),
            IdentityMatch::Uncertain
        );
        assert_eq!(
            compare_identity(&record, &observed(Some("mihomo (deleted)"), 8)),
            IdentityMatch::Reused
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn darwin_start_time_matches_sysinfo_for_an_own_process() {
        let pid = std::process::id();
        let start = darwin::start_seconds(pid).unwrap().unwrap();
        assert_eq!(
            Some(start),
            sysinfo_identity(pid).map(|identity| identity.start_token)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn darwin_start_time_reads_another_users_process() {
        // launchd runs as root; sysinfo reports a start time of 0 for it when
        // the tests run unprivileged.
        let start = darwin::start_seconds(1).unwrap();
        assert!(
            start.is_some_and(|start| start > 0),
            "launchd start time: {start:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn darwin_start_time_reports_a_free_pid_as_absent() {
        // macOS allocates pids below 99999.
        assert_eq!(darwin::start_seconds(99_999_999).unwrap(), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn reused_pid_owned_by_root_is_retired() {
        // pid 1 runs as root. An unprivileged run cannot read its executable
        // on Linux, nor its BSD info on macOS, yet its start token decides.
        let identity = process_identity(1).unwrap().expect("pid 1 is alive");
        let record = EpochPidRecord {
            pid: 1,
            epoch: 1,
            executable: "mihomo".into(),
            start_token: identity.start_token.wrapping_add(1),
            runtime_config: PathBuf::from("config-1.yaml"),
        };

        assert_eq!(
            reap_record(&record).await.unwrap(),
            OrphanReapOutcome::AlreadyExited
        );
    }

    #[tokio::test]
    async fn reused_pid_is_not_signaled_by_the_kill_path() {
        let pid = std::process::id();
        let identity = process_identity(pid).unwrap().unwrap();
        let record = EpochPidRecord {
            pid,
            epoch: 1,
            executable: identity.executable.unwrap(),
            start_token: identity.start_token.wrapping_add(1),
            runtime_config: PathBuf::from("config-1.yaml"),
        };

        let outcome = reap_record_with_kill(&record, false, || async {
            panic!("a reused pid must not be signaled");
        })
        .await
        .unwrap();

        assert_eq!(outcome, OrphanReapOutcome::AlreadyExited);
    }

    #[test]
    fn epoch_record_round_trips() {
        let record = EpochPidRecord {
            pid: 42,
            epoch: 7,
            executable: "core=name.exe".into(),
            start_token: 99,
            runtime_config: PathBuf::from(r"C:\run dir\config-7.yaml"),
        };
        assert_eq!(
            parse_epoch_record(&serialize_epoch_record(&record).unwrap()).unwrap(),
            record
        );
    }

    #[tokio::test]
    async fn write_epoch_record_second_publish_does_not_clobber_first() {
        let dir = tempfile::tempdir().unwrap();
        let pid_path = dir.path().join("core-1.pid");
        let first = EpochPidRecord {
            pid: 111,
            epoch: 1,
            executable: "first.exe".into(),
            start_token: 1,
            runtime_config: dir.path().join("config-1.yaml"),
        };
        let second = EpochPidRecord {
            pid: 222,
            epoch: 1,
            executable: "second.exe".into(),
            start_token: 2,
            runtime_config: dir.path().join("config-1.yaml"),
        };

        write_epoch_record(&pid_path, &first).await.unwrap();

        let error = write_epoch_record(&pid_path, &second)
            .await
            .expect_err("publishing over an existing epoch record must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);

        // The destination must still contain the first record: a rename-based
        // publish would have silently replaced it (the TOCTOU this guards
        // against), while hard_link fails atomically without touching it.
        let raw = tokio::fs::read_to_string(&pid_path).await.unwrap();
        assert_eq!(parse_epoch_record(&raw).unwrap(), first);
    }

    #[test]
    fn second_snapshot_only_descendant_is_captured() {
        let late_identity = ProcessIdentity {
            executable: Some("late-child".into()),
            start_token: 22,
        };
        let captured =
            merge_descendant_captures(BTreeMap::new(), [(22, Some(late_identity.clone()))].into());

        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].pid, 22);
        assert_eq!(captured[0].identity, late_identity);
    }

    #[test]
    fn unreadable_second_snapshot_identity_is_not_attributed() {
        let first_identity = ProcessIdentity {
            executable: Some("old-child".into()),
            start_token: 7,
        };
        let captured = merge_descendant_captures(
            [(7, first_identity)].into(),
            [(
                7,
                attributable_identity(Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "simulated recycled foreign process",
                ))),
            )]
            .into(),
        );

        assert!(captured.is_empty());
    }

    #[test]
    fn unreadable_descendant_confirmation_counts_as_unowned() {
        let error = || {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "simulated recycled foreign process",
            ))
        };

        assert!(identity_for_confirmation(error(), true).unwrap().is_none());
        assert_eq!(
            identity_for_confirmation(error(), false)
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
    }

    #[tokio::test]
    async fn kill_failure_is_ignored_only_after_recorded_identity_disappears() {
        #[cfg(windows)]
        let mut child = std::process::Command::new("cmd")
            .args(["/D", "/S", "/C", "ping -n 30 127.0.0.1 >NUL"])
            .spawn()
            .unwrap();
        // Spawned directly: `sh -c` may exec into `sleep`, changing the
        // executable after the identity was read.
        #[cfg(unix)]
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();

        let pid = child.id();
        #[cfg(windows)]
        let expected_exe = "cmd.exe";
        #[cfg(unix)]
        let expected_exe = "sleep";
        let identity = wait_for_process_identity(pid, expected_exe)
            .await
            .unwrap()
            .unwrap();
        let record = EpochPidRecord {
            pid,
            epoch: 1,
            executable: identity.executable.unwrap(),
            start_token: identity.start_token,
            runtime_config: PathBuf::from("config-1.yaml"),
        };

        let outcome = reap_record_with_kill(&record, false, || async move {
            crate::os::kill_pid::<String>(pid, None).await?;
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "simulated already-terminating process",
            ))
        })
        .await
        .unwrap();

        assert_eq!(outcome, OrphanReapOutcome::Killed);
        let _ = child.wait();
    }
}
