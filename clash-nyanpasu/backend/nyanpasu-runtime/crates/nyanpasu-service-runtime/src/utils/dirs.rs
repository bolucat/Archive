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
