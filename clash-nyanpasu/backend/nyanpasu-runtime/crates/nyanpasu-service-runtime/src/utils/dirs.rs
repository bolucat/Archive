use std::path::PathBuf;

use crate::consts;

const LOGS_DIR_NAME: &str = "logs";
const PID_FILE_NAME: &str = "service.pid";

const CORE_RUNTIME_DIR_NAME: &str = "core-runtime";

pub fn service_logs_dir() -> PathBuf {
    nyanpasu_utils::dirs::suggest_service_data_dir(consts::APP_NAME).join(LOGS_DIR_NAME)
}

pub fn service_data_dir() -> PathBuf {
    nyanpasu_utils::dirs::suggest_service_data_dir(consts::APP_NAME)
}

/// The executable registered with the service manager and replaced by updates.
pub fn service_binary_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    let directory = PathBuf::from("/usr/bin");
    #[cfg(not(target_os = "linux"))]
    let directory = service_data_dir();
    directory.join(format!(
        "{}{}",
        consts::APP_NAME,
        std::env::consts::EXE_SUFFIX
    ))
}

pub fn service_config_dir() -> PathBuf {
    nyanpasu_utils::dirs::suggest_service_config_dir(consts::APP_NAME).unwrap()
}

/// Service server PID file
pub fn service_pid_file() -> PathBuf {
    nyanpasu_utils::dirs::suggest_service_data_dir(consts::APP_NAME).join(PID_FILE_NAME)
}

/// Manager-owned runtime artifacts: per-epoch runtime configs, pid records, sockets.
pub fn service_core_runtime_dir() -> PathBuf {
    nyanpasu_utils::dirs::suggest_service_data_dir(consts::APP_NAME).join(CORE_RUNTIME_DIR_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn service_binary_is_the_installed_system_binary() {
        assert_eq!(
            service_binary_path(),
            PathBuf::from("/usr/bin/nyanpasu-service")
        );
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn service_binary_stays_in_the_service_data_directory() {
        assert_eq!(
            service_binary_path(),
            service_data_dir().join(format!("nyanpasu-service{}", std::env::consts::EXE_SUFFIX))
        );
    }
}
