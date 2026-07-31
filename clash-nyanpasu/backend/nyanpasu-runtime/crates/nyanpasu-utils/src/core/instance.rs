//! Running and supervising a core process.

use camino::{Utf8Path, Utf8PathBuf};
use os_pipe::pipe;
use parking_lot::{Mutex, RwLock};
use shared_child::SharedChild;
use tokio::{process::Command as TokioCommand, sync::mpsc::Receiver};
use tracing_attributes::instrument;

use std::borrow::Cow;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{ffi::OsStr, process::Command as StdCommand, sync::Arc, time::Duration};

use super::{ClashCoreType, CommandEvent, CoreType, TerminatedPayload, utils::spawn_pipe_reader};
use crate::os::ChildExt;
use crate::runtime::block_on;

// const DETACHED_PROCESS: u32 = 0x00000008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// The environment variable name for the safe paths, used by Mihomo
pub const MIHOMO_SAFE_PATHS_ENV_NAME: &str = "SAFE_PATHS";

#[cfg(windows)]
const MIHOMO_SAFE_PATHS_SEPARATOR: &str = ";";
#[cfg(not(windows))]
const MIHOMO_SAFE_PATHS_SEPARATOR: &str = ":";

// TODO: migrate to https://github.com/tauri-apps/tauri-plugin-shell/blob/v2/src/commands.rs

#[derive(Builder, Debug)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct CoreInstance {
    pub core_type: CoreType,
    pub binary_path: Utf8PathBuf,
    pub app_dir: Utf8PathBuf,
    pub config_path: Utf8PathBuf,
    /// A pid hold the instance, should check it running or not while start instance
    pid_path: Utf8PathBuf,
    #[builder(default = "self.default_instance()", setter(skip))]
    instance: Mutex<Option<Arc<SharedChild>>>,
    #[builder(default = "self.default_state()", setter(skip))]
    state: Arc<RwLock<CoreInstanceState>>,
}

#[derive(Debug, Clone, Default)]
pub enum CoreInstanceState {
    Running,
    #[default]
    Stopped,
}

impl CoreInstanceBuilder {
    fn default_instance(&self) -> Mutex<Option<Arc<SharedChild>>> {
        Mutex::new(None)
    }

    fn default_state(&self) -> Arc<RwLock<CoreInstanceState>> {
        Arc::new(RwLock::new(CoreInstanceState::default()))
    }

    fn validate(&self) -> Result<(), String> {
        match self.binary_path {
            Some(ref path) if !path.exists() && path.is_dir() => {
                return Err(format!("binary_path {path:?} does not exist"));
            }
            None => {
                return Err("binary_path is required".into());
            }
            _ => {}
        }

        match self.app_dir {
            Some(ref path) if !path.exists() && path.is_dir() => {
                return Err(format!("app_dir {path:?} does not exist"));
            }
            None => {
                return Err("app_dir is required".into());
            }
            _ => {}
        }

        match self.config_path {
            Some(ref path) if !path.exists() && path.is_file() => {
                return Err(format!("config_path {path:?} does not exist"));
            }
            None => {
                return Err("config_path is required".into());
            }
            _ => {}
        }

        if self.pid_path.is_none() {
            return Err("pid_path is required".into());
        }

        if self.core_type.is_none() {
            return Err("core_type is required".into());
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CoreInstanceError {
    #[error("Failed to start instance: {0}")]
    Io(#[from] std::io::Error),
    #[error("Cfg is not correct: {0}")]
    CfgFailed(String),
    #[error("State check failed, already running or stopped")]
    StateCheckFailed,
}

impl CoreInstance {
    pub fn set_config(&mut self, config: impl Into<Utf8PathBuf>) {
        self.config_path = config.into();
    }

    pub fn state(&self) -> CoreInstanceState {
        let state = self.state.read();
        state.clone()
    }

    pub fn get_mihomo_safe_paths(
        app_dir: &Utf8Path,
        config_dir: &Utf8Path,
        other_paths: Option<&[&Utf8Path]>,
    ) -> String {
        let mut paths = Vec::with_capacity(other_paths.map_or(0, |p| p.len()) + 2);
        paths.push(app_dir.as_str());
        paths.push(config_dir.as_str());
        if let Some(other_paths) = other_paths {
            paths.extend(other_paths.iter().map(|p| p.as_str()));
        }
        paths.join(MIHOMO_SAFE_PATHS_SEPARATOR)
    }

    pub async fn check_config<T: Into<Utf8PathBuf>>(
        &self,
        config: Option<T>,
    ) -> Result<(), CoreInstanceError> {
        let config: Cow<'_, Utf8PathBuf> = config
            .map(|c| Cow::Owned(c.into()))
            .unwrap_or(Cow::Borrowed(&self.config_path));

        Self::check_config_(&self.core_type, &config, &self.binary_path, &self.app_dir).await?;

        Ok(())
    }

    pub async fn check_config_(
        core_type: &CoreType,
        config_path: &Utf8Path,
        binary_path: &Utf8Path,
        app_dir: &Utf8Path,
    ) -> Result<(), CoreInstanceError> {
        let config_dir = config_path.parent().expect("config_path is not a file");
        let mut cmd = TokioCommand::new(binary_path);
        cmd.args(vec![
            OsStr::new("-t"),
            OsStr::new("-d"),
            app_dir.as_os_str(),
            OsStr::new("-f"),
            config_path.as_os_str(),
        ])
        .env(
            MIHOMO_SAFE_PATHS_ENV_NAME,
            Self::get_mihomo_safe_paths(app_dir, config_dir, None),
        );
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = cmd.output().await?;
        if !output.status.success() {
            let error = if !matches!(core_type, CoreType::Clash(ClashCoreType::ClashRust)) {
                super::utils::parse_check_output(
                    String::from_utf8_lossy(&output.stdout).to_string(),
                )
            } else {
                // pipe stdout and stderr to the same string
                format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            };
            return Err(CoreInstanceError::CfgFailed(error));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn kill_instance_by_pid_file(&self) -> Result<(), std::io::Error> {
        tracing::debug!("kill instance by pid file: {:?}", self.pid_path);
        crate::os::kill_by_pid_file(
            &self.pid_path,
            Some(&CoreType::get_supported_cores_executables()),
        )
        .await
    }

    #[instrument(skip(self))]
    async fn write_pid_file(&self, pid: u32) -> Result<(), std::io::Error> {
        crate::os::create_pid_file(&self.pid_path, pid).await
    }

    #[instrument(skip(self))]
    /// Run the instance, it is a blocking operation
    pub async fn run(
        &self,
    ) -> Result<(Arc<SharedChild>, Receiver<CommandEvent>), CoreInstanceError> {
        {
            let state = self.state.read();
            if matches!(*state, CoreInstanceState::Running) {
                return Err(CoreInstanceError::StateCheckFailed);
            }
        }
        // kill instance by pid file if exists
        if let Err(err) = self.kill_instance_by_pid_file().await {
            tracing::error!("Failed to kill instance by pid file: {:?}", err);
        }

        let args = match self.core_type {
            CoreType::Clash(ref core_type) => {
                core_type.get_run_args(self.app_dir.as_path(), self.config_path.as_path())
            }
            CoreType::SingBox => {
                unimplemented!("SingBox is not supported yet")
            }
        };
        let args = args.iter().map(|arg| arg.as_ref()).collect::<Vec<&OsStr>>();
        let (stdout_reader, stdout_writer) = pipe()?;
        let (stderr_reader, stderr_writer) = pipe()?;
        // let (stdin_reader, stdin_writer) = pipe()?;
        let (tx, rx) = tokio::sync::mpsc::channel::<CommandEvent>(1);
        let config_dir = self
            .config_path
            .parent()
            .expect("config_path is not a file");
        let child = Arc::new({
            let mut command = StdCommand::new(&self.binary_path);
            command
                .env(
                    MIHOMO_SAFE_PATHS_ENV_NAME,
                    Self::get_mihomo_safe_paths(&self.app_dir, config_dir, None),
                )
                .args(args)
                .stderr(stderr_writer)
                .stdout(stdout_writer)
                // .stdin(stdin_reader)
                .current_dir(&self.app_dir);
            #[cfg(windows)]
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
            SharedChild::spawn(&mut command)?
        });
        let child_ = child.clone();
        let guard = Arc::new(RwLock::new(()));
        spawn_pipe_reader(
            tx.clone(),
            guard.clone(),
            stdout_reader,
            CommandEvent::Stdout,
            None,
        );
        spawn_pipe_reader(
            tx.clone(),
            guard.clone(),
            stderr_reader,
            CommandEvent::Stderr,
            None,
        );

        let state_ = self.state.clone();
        let tx_ = tx.clone();
        let guard_ = guard.clone();
        std::thread::spawn(move || {
            let _ = match child_.wait() {
                Ok(status) => {
                    tracing::trace!("instance terminated: {:?}", status);
                    let _l = guard_.write();
                    block_on(async move {
                        {
                            let mut state = state_.write();
                            *state = CoreInstanceState::Stopped;
                        }
                        tx_.send(CommandEvent::Terminated(TerminatedPayload {
                            code: status.code(),
                            #[cfg(windows)]
                            signal: None,
                            #[cfg(unix)]
                            signal: std::os::unix::process::ExitStatusExt::signal(&status),
                        }))
                        .await
                    })
                }
                Err(err) => {
                    tracing::trace!("instance terminated with error: {:?}", err);
                    let _l = guard_.write();
                    block_on(async move { tx_.send(CommandEvent::Error(err.to_string())).await })
                }
            };
        });
        let state_ = self.state.clone();
        let child_ = child.clone();
        // 等待 1.5 秒，若进程结束则表示失败
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(1500));
            let state = child_.try_wait();
            tracing::debug!("instance check point: {:?}", state);
            if let Ok(None) = state {
                {
                    let mut state = state_.write();
                    *state = CoreInstanceState::Running;
                }
                let _l = guard.read();
                let _ = block_on(async move { tx.send(CommandEvent::DelayCheckpointPass).await });
            }
        });
        if let Err(err) = self.write_pid_file(child.id()).await {
            tracing::error!("Failed to write pid file: {:?}", err);
        }
        {
            let mut instance = self.instance.lock();
            *instance = Some(child.clone());
        }
        Ok((child, rx))
    }

    // TODO: maybe we should add a timeout for this function
    /// Kill the instance, it is a blocking operation
    #[instrument(skip(self))]
    pub async fn kill(&self) -> Result<(), CoreInstanceError> {
        let instance = {
            let instance_holder = self.instance.lock();
            if instance_holder.is_none() {
                return Err(CoreInstanceError::StateCheckFailed);
            }
            instance_holder.as_ref().unwrap().clone()
        };
        let instance_ = instance.clone();
        tracing::debug!("try to gracefully kill instance...");
        match tokio::task::spawn_blocking(move || instance_.gracefully_kill()).await {
            Ok(Ok(())) => {
                for _ in 0..20 {
                    if let Some(state) = instance.try_wait()? {
                        if !state.success() {
                            tracing::warn!("instance terminated with error: {:?}", state);
                        }
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("Failed to gracefully kill instance: {:?}", e);
            }
            Err(err) => {
                tracing::warn!("Failed to spawn gracefully kill thread: {:?}", err);
            }
        }
        tracing::debug!("gracefully kill failed, try to force kill instance...");
        instance.kill()?;
        // poll the instance until it is terminated
        for i in 0..30 {
            if let Some(state) = instance.try_wait()? {
                if !state.success() {
                    tracing::warn!("instance terminated with error: {:?}", state);
                }
                break;
            } else if i == 29 {
                return Err(CoreInstanceError::Io(std::io::Error::other(
                    "Failed to kill instance: force kill timeout",
                )));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        {
            let mut instance_holder = self.instance.lock();
            *instance_holder = None;
        }
        {
            let mut state = self.state.write();
            *state = CoreInstanceState::Stopped;
        }
        Ok(())
    }
}

/// clean-up the instance when the manager is dropped
impl Drop for CoreInstance {
    fn drop(&mut self) {
        let mut instance = self.instance.lock();
        if let Some(instance) = instance.take()
            && let Err(err) = instance.kill()
        {
            tracing::error!("Failed to kill instance: {:?}", err);
        }
    }
}
