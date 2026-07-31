use anyhow::{Result, anyhow};
use std::{fs, io::IsTerminal, sync::OnceLock};
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
        if !log_dir.exists() {
            let _ = fs::create_dir_all(&log_dir);
        }
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
