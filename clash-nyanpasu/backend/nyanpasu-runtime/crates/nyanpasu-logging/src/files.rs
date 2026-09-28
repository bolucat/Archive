use crate::{LogError, LogFileInfo, LogResult, MAX_BATCH};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

pub struct FileRead {
    pub identity: String,
    pub name: String,
    pub len: u64,
    pub prefix: Vec<u8>,
    pub bytes: Vec<u8>,
}
/// Implementations must enforce the range limit and reject paths outside their configured source.
pub trait LogFiles: Send + Sync + 'static {
    fn catalog(&self) -> LogResult<Vec<LogFileInfo>>;
    fn read(&self, file: &str, offset: u64, length: usize) -> LogResult<FileRead>;
}
pub struct FsLogFiles {
    directory: PathBuf,
    prefix: String,
}
impl FsLogFiles {
    pub fn new(directory: PathBuf, prefix: String) -> Self {
        Self { directory, prefix }
    }
    /// Accepts both `tracing-appender`'s `{prefix}.{date}.app.log` and
    /// `flexi_logger`'s `{prefix}_{timestamp}[.restart-NNNN].log`. Both sort by
    /// name in write order, and `_` sorts after `.`, so the newer scheme's
    /// files come first in a descending catalog.
    fn valid(&self, name: &str) -> bool {
        let appender = name.starts_with(&format!("{}.", self.prefix)) && name.ends_with(".app.log");
        let flexi = name.starts_with(&format!("{}_", self.prefix)) && name.ends_with(".log");
        (appender || flexi)
            && name.len() <= 255
            && !name.contains(['/', '\\', ':'])
            && !name.contains("..")
    }
}
impl LogFiles for FsLogFiles {
    fn catalog(&self) -> LogResult<Vec<LogFileInfo>> {
        let directory = match fs::read_dir(&self.directory) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(LogError::Unavailable),
        };
        let mut files = Vec::new();
        for entry in directory.take(4096) {
            let entry = entry.map_err(|_| LogError::Unavailable)?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !self.valid(&name)
                || !entry
                    .file_type()
                    .map_err(|_| LogError::Unavailable)?
                    .is_file()
            {
                continue;
            }
            let meta = entry.metadata().map_err(|_| LogError::FileGone)?;
            files.push(LogFileInfo {
                id: name.clone(),
                name,
                bytes: meta.len().to_string(),
            });
        }
        files.sort_by(|a, b| b.name.cmp(&a.name));
        files.truncate(128);
        Ok(files)
    }
    fn read(&self, file: &str, offset: u64, length: usize) -> LogResult<FileRead> {
        if !self.valid(file) || length > MAX_BATCH {
            return Err(LogError::InvalidRequest);
        }
        let path = self.directory.join(file);
        let metadata = fs::symlink_metadata(&path).map_err(|_| LogError::FileGone)?;
        if !metadata.file_type().is_file() {
            return Err(LogError::InvalidRequest);
        }
        let identity = file_id::get_file_id(&path).map_err(|_| LogError::FileGone)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1 | 2 | 4);
        }
        let mut handle = options.open(&path).map_err(|_| LogError::FileGone)?;
        let len = handle.metadata().map_err(|_| LogError::FileGone)?.len();
        let mut prefix = Vec::new();
        handle
            .by_ref()
            .take(256)
            .read_to_end(&mut prefix)
            .map_err(|_| LogError::Unavailable)?;
        handle
            .seek(SeekFrom::Start(offset))
            .map_err(|_| LogError::Unavailable)?;
        let mut bytes = Vec::new();
        handle
            .by_ref()
            .take(length as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| LogError::Unavailable)?;
        if file_id::get_file_id(&path).map_err(|_| LogError::FileGone)? != identity {
            return Err(LogError::CursorReset);
        }
        Ok(FileRead {
            identity: format!("{identity:?}"),
            name: file.into(),
            len,
            prefix,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_lists_both_naming_schemes_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "app.2026-09-28.app.log",
            "app_2026-09-29_08-00-00.log",
            "app_2026-09-29_08-00-00.restart-0000.log",
            "app_2026-09-29_09-30-00.log",
            "app_2026-09-29_09-30-00.txt",
            "other_2026-09-29_09-30-00.log",
            "app.2026-09-29.log",
        ] {
            std::fs::write(dir.path().join(name), b"{}\n").unwrap();
        }
        let files = FsLogFiles::new(dir.path().into(), "app".into());
        let names: Vec<_> = files
            .catalog()
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(
            names,
            [
                "app_2026-09-29_09-30-00.log",
                "app_2026-09-29_08-00-00.restart-0000.log",
                "app_2026-09-29_08-00-00.log",
                "app.2026-09-28.app.log",
            ]
        );
        assert!(files.read("app_2026-09-29_09-30-00.log", 0, 3).is_ok());
        assert_eq!(
            files.read("app_../outside.log", 0, 1).err(),
            Some(LogError::InvalidRequest)
        );
    }
}
