use clap::Parser as _;
use tracing::{Level, info};
use wind_core::App;

use crate::{
	cli::Cli,
	conf::{persistent::PersistentConfig, resolved::ResolvedConfig},
};
mod cli;
mod conf;
mod log;
mod plugin;

// curl --socks5 127.0.0.1:6666 https://www.bing.com
#[tokio::main]
async fn main() -> eyre::Result<()> {
	log::init_log(Level::TRACE)?;
	info!(target: "wind_main", "Wind starting");
	// Use clap's own `Error::exit()` so version/help requests go to stdout
	// with exit code 0 and ACTUAL errors go to stderr with a non-zero exit
	// code. The previous `println!("{err:#}"); return Ok(())` lumped both
	// together: real argument errors disappeared into stdout with exit 0,
	// hiding misconfigurations from CI and shell pipelines.
	let cli = match Cli::try_parse() {
		Ok(v) => v,
		Err(err) => err.exit(),
	};

	if cli.version {
		// Allow build-time override via `WIND_OVERRIDE_VERSION` (e.g. nightly
		// builds stamping a dated SHA).
		const VER: &str = match option_env!("WIND_OVERRIDE_VERSION") {
			Some(v) => v,
			None => env!("CARGO_PKG_VERSION"),
		};
		println!("wind {VER}");
		return Ok(());
	}

	// Honor `--work_dir`: change the process working directory before anything
	// resolves relative paths (config load below, and the `init` generator).
	// Previously this flag was parsed but never used.
	if let Some(work_dir) = &cli.work_dir {
		std::env::set_current_dir(work_dir)
			.map_err(|e| eyre::eyre!("failed to set working directory to {}: {e}", work_dir.display()))?;
		info!(target: "wind_main", "working directory set to {}", work_dir.display());
	}

	match &cli.command {
		Some(crate::cli::Commands::Init { format }) => {
			let default_config = PersistentConfig::default();
			let format_str = match format {
				crate::cli::ConfigFormat::Yaml => "yaml",
				crate::cli::ConfigFormat::Toml => "toml",
			};
			let file_name = format!("config.{}", format_str);
			let file_path = if let Some(config_dir) = &cli.config_dir {
				std::fs::create_dir_all(config_dir)?;
				config_dir.join(&file_name)
			} else {
				std::path::PathBuf::from(&file_name)
			};
			default_config.export_to_file(&file_path, format_str)?;
			println!("Created default configuration at: {}", file_path.display());
			return Ok(());
		}
		None => {}
	}

	let persistent_config = PersistentConfig::load(cli.config, cli.config_dir)?;
	info!(target: "wind_main", "Configuration loaded successfully");

	// Resolve the persisted form into a fully-typed runtime config: this
	// parses socket addresses / strategies / congestion-control names and
	// validates tags, load-balance references and dependency cycles, so the
	// plugin never sees half-resolved strings.
	let resolved_config = ResolvedConfig::try_from(persistent_config)?;
	info!(target: "wind_main", "Configuration resolved successfully");

	// Assemble the wind App: outbounds, router, and inbounds are wired by the
	// plugin; `App::run` owns the dispatcher, task tracking, traffic flush and
	// graceful shutdown/drain.
	let app = App::<plugin::WindRouter>::new()
		.add_plugin(plugin::WindPlugin::new(resolved_config))
		.await?;
	app.run().await?;

	info!(target: "wind_main", "Shutdown complete");
	Ok(())
}
