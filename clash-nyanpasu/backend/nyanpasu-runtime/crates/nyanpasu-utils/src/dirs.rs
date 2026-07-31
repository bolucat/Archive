//! Platform-aware application and service directory suggestions.

use dirs::{config_dir, data_local_dir};
use std::path::PathBuf;
/// Suggest a directory for configuration files.
/// * - Linux: Resolves to $XDG_CONFIG_HOME/{placeholder} or $HOME/.config/{placeholder}.
/// * - macOS: Resolves to $HOME/Library/Application Support/{placeholder}/config.
/// * - Windows: Resolves to {FOLDERID_RoamingAppData}/{placehholder}/config.
pub fn suggest_config_dir(placeholder: &str) -> Option<PathBuf> {
    let path = config_dir()?;
    #[cfg(target_os = "linux")]
    {
        Some(path.join(placeholder))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Some(path.join(placeholder).join("config"))
    }
}

/// Suggest a directory for configuration files.
/// * - Linux: Resolves to $XDG_DATA_HOME/{placeholder} or $HOME/.local/share/{placeholder}.
/// * - macOS: Resolves to $HOME/Library/Application Support/{placeholder}/data.
/// * - Windows: Resolves to {FOLDERID_LocalAppData}/{placehholder}/data.
pub fn suggest_data_dir(placeholder: &str) -> Option<PathBuf> {
    let path = data_local_dir()?;
    #[cfg(target_os = "linux")]
    {
        Some(path.join(placeholder))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Some(path.join(placeholder).join("data"))
    }
}

#[cfg(windows)]
use windows::core::Error as WindowsError;

#[derive(Debug, thiserror::Error)]
pub enum RetrieveDirError {
    #[cfg(windows)]
    #[error("System call failed: {0}")]
    SystemCallFailed(#[from] WindowsError),

    #[error("failed convert utf-16 to utf-8: {0}")]
    Utf16ToUtf8Failed(#[from] std::string::FromUtf16Error),
}

#[cfg(windows)]
fn get_program_data_dir() -> Result<PathBuf, RetrieveDirError> {
    use windows::Win32::UI::Shell::{FOLDERID_ProgramData, KF_FLAG_CREATE, SHGetKnownFolderPath};
    let path = unsafe {
        let path = SHGetKnownFolderPath(&FOLDERID_ProgramData, KF_FLAG_CREATE, None)?;
        path.to_string()?
    };
    Ok(PathBuf::from(path))
}

/// Suggest a directory for configuration files if user-spec is not allowed.
/// * - Linux: Resolves to /etc/{placeholder}.
/// * - macOS: Resolves to /Library/Application Support/{placeholder}/config.
/// * - Windows: Resolves to {FOLDERID_ProgramData}/{placehholder}/config.
pub fn suggest_service_config_dir(placeholder: &str) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        Some(PathBuf::from("/etc").join(placeholder))
    }
    #[cfg(target_os = "macos")]
    {
        Some(
            PathBuf::from("/Library/Application Support")
                .join(placeholder)
                .join("config"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        get_program_data_dir()
            .map(|path| path.join(placeholder).join("config"))
            .map_err(|err| {
                tracing::error!("Failed to get program data directory: {:?}", err);
                err
            })
            .ok()
    }
}

/// Suggest a directory for data files if user-spec is not allowed.
/// * - Linux: Resolves to /var/lib/{placeholder}.
/// * - macOS: Resolves to /Library/Application Support/{placeholder}/data.
/// * - Windows: Resolves to {FOLDERID_ProgramData}/{placehholder}/data.
pub fn suggest_service_data_dir(placeholder: &str) -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/var/lib").join(placeholder)
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support")
            .join(placeholder)
            .join("data")
    }
    #[cfg(target_os = "windows")]
    {
        get_program_data_dir()
            .map(|path| path.join(placeholder).join("data"))
            .map_err(|err| {
                tracing::error!("Failed to get program data directory: {:?}", err);
                err
            })
            .unwrap_or_else(|_| PathBuf::new())
    }
}
