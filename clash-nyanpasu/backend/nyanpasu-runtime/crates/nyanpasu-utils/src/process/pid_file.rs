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

impl EpochPidFile {
    pub fn new(path: impl Into<PathBuf>, epoch: u64, runtime_config: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            epoch,
            runtime_config: runtime_config.into(),
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
    AlreadyExited,
    Killed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessIdentity {
    executable: String,
    start_token: u64,
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
                let identity = wait_for_process_identity(pid).await?.ok_or_else(|| {
                    identity_error(format!("spawned pid {pid} disappeared before recording"))
                })?;
                if !exe_names_equal(&identity.executable, expected_exe) {
                    return Err(identity_error(format!(
                        "spawned pid {pid} executable `{}` does not match `{expected_exe}`",
                        identity.executable
                    )));
                }
                let value = EpochPidRecord {
                    pid,
                    epoch: spec.epoch,
                    executable: identity.executable,
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
    let raw = tokio::fs::read_to_string(path).await?;
    parse_epoch_record(&raw).map(Some)
}

/// Kills the orphan in `path` only after validating the full epoch record and
/// proving that both pid and runtime config are contained by `runtime_dir`.
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
pub async fn reap_epoch_pid_file(
    path: impl AsRef<Path>,
    runtime_dir: impl AsRef<Path>,
) -> std::io::Result<OrphanReapOutcome> {
    let path = path.as_ref();
    let runtime_dir = tokio::fs::canonicalize(runtime_dir.as_ref()).await?;
    let parent = path
        .parent()
        .ok_or_else(|| invalid_input("pid file has no parent directory"))?;
    if tokio::fs::canonicalize(parent).await? != runtime_dir {
        return Err(invalid_input("pid file escapes the runtime directory"));
    }
    let epoch = epoch_from_file_name(path, "core-", ".pid")?;
    let Some(record) = read_epoch_pid_file(path).await? else {
        return Ok(OrphanReapOutcome::NotFound);
    };
    if record.epoch != epoch {
        return Err(invalid_data("pid filename and embedded epoch differ"));
    }
    let runtime_parent = record
        .runtime_config
        .parent()
        .ok_or_else(|| invalid_input("runtime config has no parent directory"))?;
    if tokio::fs::canonicalize(runtime_parent).await? != runtime_dir {
        return Err(invalid_input(
            "runtime config escapes the runtime directory",
        ));
    }
    let runtime_epoch = epoch_from_file_name(&record.runtime_config, "config-", ".yaml")?;
    if runtime_epoch != epoch {
        return Err(invalid_data(
            "runtime config filename and embedded epoch differ",
        ));
    }
    validate_runtime_target(&record.runtime_config).await?;

    let outcome = reap_record(&record).await?;
    remove_record_if_matches(path, &record).await?;
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
        Ok(metadata) if crate::io::atomic_fs::is_reparse_point(&metadata) => {
            Err(invalid_input(format!(
                "pid file must not be a reparse point: {}",
                path.display()
            )))
        }
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
    if !record_matches_identity(record, &identity) {
        return Err(identity_error(format!(
            "cannot prove ownership of live pid {}",
            record.pid
        )));
    }

    let descendants = capture_descendants(record.pid);
    let root_outcome =
        reap_record_with_kill(record, false, || kill_recorded_process(record)).await?;
    let mut killed_descendant = false;
    let mut failures = Vec::new();
    for descendant in descendants {
        let descendant_record = EpochPidRecord {
            pid: descendant.pid,
            epoch: record.epoch,
            executable: descendant.identity.executable,
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
    if !record_matches_identity(record, &identity) {
        if unowned_is_dead {
            return Ok(OrphanReapOutcome::AlreadyExited);
        }
        return Err(identity_error(format!(
            "cannot prove ownership of live pid {}",
            record.pid
        )));
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
    result.ok().flatten()
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

async fn wait_for_process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    let mut access_error = None;
    for attempt in 0..IDENTITY_WAIT_ATTEMPTS {
        match process_identity(pid) {
            Ok(Some(identity)) => return Ok(Some(identity)),
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
    match access_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

#[cfg(all(not(windows), not(target_os = "linux")))]
fn process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System, UpdateKind};

    let kind = RefreshKind::nothing()
        .with_processes(ProcessRefreshKind::nothing().with_exe(UpdateKind::Always));
    let mut system = System::new_with_specifics(kind);
    system.refresh_specifics(kind);
    let Some(process) = system.process(Pid::from_u32(pid)) else {
        return Ok(None);
    };
    let executable = process
        .exe()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(|| identity_error(format!("cannot resolve executable for live pid {pid}")))?;
    Ok(Some(ProcessIdentity {
        executable: executable.to_owned(),
        start_token: process.start_time(),
    }))
}

#[cfg(target_os = "linux")]
fn process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    let Some(first_ticks) = linux_start_ticks(pid)? else {
        return Ok(None);
    };
    let executable_path = match std::fs::read_link(format!("/proc/{pid}/exe")) {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let Some(second_ticks) = linux_start_ticks(pid)? else {
        return Ok(None);
    };
    if first_ticks != second_ticks {
        return Ok(None);
    }
    let executable = executable_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| identity_error(format!("cannot resolve executable for live pid {pid}")))?;
    let boot_id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    Ok(Some(ProcessIdentity {
        executable: executable.to_owned(),
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
fn process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
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
        executable: executable.to_owned(),
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
async fn kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
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
async fn kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
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
async fn kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
    kill_revalidated_by_pid(record)
}

#[cfg(not(any(unix, windows)))]
async fn kill_recorded_process(record: &EpochPidRecord) -> std::io::Result<()> {
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

fn record_matches_identity(record: &EpochPidRecord, identity: &ProcessIdentity) -> bool {
    record.start_token == identity.start_token
        && exe_names_equal(&record.executable, &identity.executable)
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
                | crate::io::atomic_fs::AtomicFsError::Contended(path) => {
                    std::io::Error::other(format!(
                        "unexpected atomic filesystem error for {}",
                        path.display()
                    ))
                }
            })
    }
    .await;
    // Publication consumes `temp` on success; on failure it remains and must
    // be cleaned up here.
    let _ = tokio::fs::remove_file(&temp).await;
    result
}

async fn remove_record_if_matches(path: &Path, expected: &EpochPidRecord) -> std::io::Result<()> {
    if read_epoch_pid_file(path).await?.as_ref() != Some(expected) {
        return Ok(());
    }
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

fn invalid_input(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into())
}

fn invalid_data(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}

fn identity_error(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            executable: "late-child".into(),
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
            executable: "old-child".into(),
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
        #[cfg(unix)]
        let mut child = std::process::Command::new("sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .unwrap();

        let pid = child.id();
        let identity = wait_for_process_identity(pid).await.unwrap().unwrap();
        let record = EpochPidRecord {
            pid,
            epoch: 1,
            executable: identity.executable,
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
