#[cfg(windows)]
pub mod acl;
pub mod os;

#[inline]
pub(crate) fn get_name_string(placeholder: &str) -> String {
    if cfg!(windows) {
        format!("\\\\.\\pipe\\{placeholder}")
    } else {
        format!("/var/run/{placeholder}.sock")
    }
}

#[cfg(unix)]
pub(crate) async fn remove_socket_if_exists(placeholder: &str) -> Result<(), std::io::Error> {
    use std::path::PathBuf;

    let path: PathBuf = PathBuf::from(format!("/var/run/{placeholder}.sock"));
    if tokio::fs::metadata(&path).await.is_ok() {
        tokio::fs::remove_file(&path).await?;
    }
    Ok(())
}

/// Get the current millisecond timestamp
pub fn get_current_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}
