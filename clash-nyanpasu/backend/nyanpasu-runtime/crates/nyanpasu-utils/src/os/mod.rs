#![allow(unused_imports)]
#![allow(dead_code)]

//! Operating-system and process helpers.

mod child;
pub mod elevated;
mod os_impl;
pub use child::*;
pub use elevated::*;
pub use os_impl::*;
use std::fmt::Debug;
use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};
use tracing_attributes::instrument;

use std::{ffi::OsStr, fmt::Display, io::Error as IoError, path::Path};
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

#[instrument]
pub async fn create_pid_file<T>(path: T, pid: u32) -> Result<(), std::io::Error>
where
    T: AsRef<Path> + std::fmt::Debug,
{
    let path = path.as_ref();
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .await?;
    file.write_all(pid.to_string().as_bytes()).await?;
    Ok(())
}

#[instrument]
pub async fn get_pid_from_file<T>(path: T) -> Option<u32>
where
    T: AsRef<Path> + std::fmt::Debug,
{
    let path = path.as_ref();
    let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
    let pid = content.trim().parse().ok()?;
    Some(pid)
}

#[instrument]
pub fn pid_exists<Name: AsRef<str> + Debug>(pid: u32, validator: Option<&[Name]>) -> bool {
    let kind = RefreshKind::nothing()
        .with_processes(ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet));
    let mut system = System::new_with_specifics(kind);
    system.refresh_specifics(kind);
    system
        .process(Pid::from_u32(pid))
        .is_some_and(|p| match validator {
            Some(validator) => validator
                .iter()
                .map(|v| v.as_ref())
                // FIXME: this validator is designed for core name, it use the ascii name of the process. So it always correct.
                // If in future, our tool introduce the non-ascii name, we need to change this validator, use the encoding-rs crate to decode the name.
                .any(|v| {
                    p.exe()
                        .is_some_and(|exe| exe.to_string_lossy().to_lowercase().contains(v))
                }),
            None => true,
        })
}

#[instrument]
pub async fn kill_pid<Name: AsRef<str> + Debug>(
    pid: u32,
    validator: Option<&[Name]>,
) -> Result<(), std::io::Error> {
    tracing::debug!("kill pid: {}", pid);
    if pid_exists(pid, validator) {
        tracing::debug!("pid exists, kill it");
        let list = kill_tree::tokio::kill_tree(pid)
            .await
            .map_err(|e| IoError::other(format!("kill error: {e:?}")))?;
        for p in list {
            if matches!(p, kill_tree::Output::Killed { .. }) {
                tracing::info!("process is killed: {:?}", p);
            }
        }
    }
    Ok(())
}

#[instrument]
pub async fn kill_by_pid_file<T, Name: AsRef<str> + Debug>(
    path: T,
    validator: Option<&[Name]>,
) -> Result<(), std::io::Error>
where
    T: AsRef<Path> + std::fmt::Debug,
{
    let pid = match get_pid_from_file(&path).await {
        Some(pid) => pid,
        None => {
            tracing::debug!("pid file not found or parsing error, skip");
            return Ok(());
        }
    };
    kill_pid(pid, validator).await?;
    tokio::fs::remove_file(path).await
}
