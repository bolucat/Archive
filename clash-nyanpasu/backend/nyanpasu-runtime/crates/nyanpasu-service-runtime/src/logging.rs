use anyhow::{Result, anyhow};
use std::{fs, io::IsTerminal, path::Path, sync::OnceLock};
use tracing::level_filters::LevelFilter;
use tracing_appender::{
    non_blocking::{NonBlocking, WorkerGuard},
    rolling::Rotation,
};
use tracing_log::log_tracer;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

static GUARD: OnceLock<WorkerGuard> = OnceLock::new();

fn get_file_appender(max_files: usize) -> Result<(NonBlocking, WorkerGuard)> {
    let log_dir = crate::utils::dirs::service_logs_dir();
    let file_appender = tracing_appender::rolling::Builder::new()
        .filename_prefix("nyanpasu-service")
        .filename_suffix("app.log")
        .rotation(Rotation::DAILY)
        .max_log_files(max_files)
        .build(log_dir)?;
    Ok(tracing_appender::non_blocking(file_appender))
}

/// initial instance global logger
pub fn init(debug: bool, write_file: bool) -> anyhow::Result<()> {
    if write_file {
        let log_dir = crate::utils::dirs::service_logs_dir();
        ensure_plain_dir(&log_dir)?;
        harden_log_dir(&log_dir)?;
    }
    let (log_level, log_max_files) = {
        (
            {
                #[cfg(not(feature = "tracing"))]
                if debug {
                    LevelFilter::DEBUG
                } else {
                    LevelFilter::INFO
                }
                #[cfg(feature = "tracing")]
                LevelFilter::TRACE
            },
            7,
        )
    };
    let filter = EnvFilter::builder()
        .with_default_directive(log_level.into())
        .from_env_lossy();

    let terminal_layer = fmt::Layer::new()
        .with_ansi(
            std::io::stdout().is_terminal()
                && supports_color::on(supports_color::Stream::Stdout).is_some(),
        )
        .compact()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_writer(std::io::stdout);

    let subscriber = tracing_subscriber::registry();
    #[cfg(feature = "tracing")]
    let subscriber = subscriber.with(console_subscriber::spawn());
    let subscriber = subscriber.with(filter).with(terminal_layer);
    let file_layer = if write_file {
        let (appender, _guard) = get_file_appender(log_max_files)?;
        let file_layer = fmt::layer()
            .json()
            .with_writer(appender)
            .with_line_number(true)
            .with_file(true);
        Some((file_layer, _guard))
    } else {
        None
    };
    match file_layer {
        Some((file_layer, _guard)) => {
            // TODO: 改善日记注册逻辑
            use crate::server::Logger;
            let logger_layer = fmt::layer()
                .json()
                .with_writer(Logger::global().clone())
                .with_line_number(true)
                .with_file(true);
            let subscriber = subscriber.with(file_layer).with(logger_layer);
            log_tracer::LogTracer::init()?;
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|x| anyhow!("setup logging error: {}", x))?;
            GUARD.set(_guard).ok();
        }
        None => {
            log_tracer::LogTracer::init()?;
            tracing::subscriber::set_global_default(subscriber)
                .map_err(|x| anyhow!("setup logging error: {}", x))?;
        }
    };

    Ok(())
}

fn ensure_plain_dir(dir: &Path) -> Result<()> {
    match fs::symlink_metadata(dir) {
        Ok(metadata)
            if metadata.file_type().is_symlink()
                || nyanpasu_utils::io::atomic_fs::is_reparse_point(&metadata)
                || !metadata.is_dir() =>
        {
            Err(anyhow!(
                "service log path is not a plain directory: {}",
                dir.display()
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(dir)?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

/// Restrict the service log directory to the account the service runs under,
/// the way the manager's runtime directory already is.
///
/// `/status` publishes this path from L3 onward, and advertising a location is
/// a good moment to be sure of its permissions. On Windows the DACL is set
/// explicitly rather than inherited: an inherited descriptor does not carry
/// `SE_DACL_PROTECTED`, which is exactly what the verifier requires.
///
/// The writer is unaffected — the DACL grants SYSTEM full access and the
/// service runs elevated. Readers are affected, deliberately: see
/// `LogPathsInfo`'s rustdoc.
///
/// Ownership of a pre-existing directory is not verified here: the shared
/// helpers accept the current owner. Tightening that requires a
/// `nyanpasu-utils` change and is tracked as a follow-up.
fn harden_log_dir(dir: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| anyhow!("failed to protect the service log directory: {error}"))?;
    }
    #[cfg(windows)]
    {
        use nyanpasu_utils::io::atomic_fs::{
            harden_windows_directory_acl, verify_windows_directory_acl,
        };
        harden_windows_directory_acl(dir)
            .map_err(|error| anyhow!("failed to protect the service log directory: {error}"))?;
        verify_windows_directory_acl(dir)
            .map_err(|error| anyhow!("the service log directory is not protected: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_regular_file_is_rejected_as_the_service_log_directory() {
        let guard = tempfile::tempdir().unwrap();
        let path = guard.path().join("logs");
        fs::write(&path, b"not a directory").unwrap();

        let error = ensure_plain_dir(&path).unwrap_err();

        assert!(error.to_string().contains("not a plain directory"));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_is_rejected_as_the_service_log_directory() {
        let guard = tempfile::tempdir().unwrap();
        let real = guard.path().join("real");
        let path = guard.path().join("logs");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, &path).unwrap();

        let error = ensure_plain_dir(&path).unwrap_err();

        assert!(error.to_string().contains("not a plain directory"));
    }

    /// The directory `/status` advertises must not be world-readable. The
    /// Windows half also pins why the call is explicit: `verify` rejects an
    /// inherited DACL, so a directory that merely sits inside a protected
    /// parent would fail this.
    #[test]
    fn the_service_log_directory_is_owner_only() {
        let guard = tempfile::tempdir().unwrap();
        let dir = guard.path().join("logs");
        fs::create_dir_all(&dir).unwrap();

        harden_log_dir(&dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        #[cfg(windows)]
        {
            nyanpasu_utils::io::atomic_fs::verify_windows_directory_acl(&dir).unwrap();
        }
    }
}
