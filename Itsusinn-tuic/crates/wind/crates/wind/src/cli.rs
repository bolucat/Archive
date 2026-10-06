use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};
pub use tracing::Level;

#[derive(Parser)]
#[command(about, long_about = None)]
pub struct Cli {
	/// Set a custom config
	#[arg(short, visible_short_alias = 'f', long, value_name = "FILE/BASE64-TEXT")]
	pub config: Option<String>,

	/// Set configuration directory
	#[arg(short = 'C', visible_short_alias = 'd', long, value_name = "PATH")]
	pub config_dir: Option<PathBuf>,

	/// Set working directory
	#[arg(short = 'D', long, value_name = "PATH")]
	pub work_dir: Option<PathBuf>,

	/// Set the log level (trace, debug, info, warn or error)
	///
	/// Applies to every wind workspace crate's target; targets outside those
	/// namespaces stay at their `info` default.
	#[arg(long, visible_short_alias = 'l', value_name = "LEVEL", default_value = "info", value_parser = parse_log_level)]
	pub log_level: Level,

	/// Show current version
	#[arg(short = 'v', visible_short_alias = 'V', long, action = ArgAction::SetTrue)]
	pub version: bool,

	#[command(subcommand)]
	pub command: Option<Commands>,
}

/// Parse a `--log-level` value into a [`tracing::Level`].
fn parse_log_level(value: &str) -> Result<Level, String> {
	value
		.parse::<Level>()
		.map_err(|_| format!("invalid log level {value:?} (expected trace, debug, info, warn or error)"))
}

#[derive(Subcommand)]
pub enum Commands {
	/// Initialize a new default configuration file
	Init {
		/// Specify the configuration file format (yaml or toml)
		#[arg(short, long, value_enum, default_value = "yaml")]
		format: ConfigFormat,
	},
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ConfigFormat {
	Yaml,
	Toml,
}

#[cfg(test)]
mod tests {
	use clap::Parser as _;

	use super::{Cli, Level};

	#[test]
	fn log_level_flag_selects_the_requested_filter_level() {
		for (arg, expected) in [
			("trace", Level::TRACE),
			("debug", Level::DEBUG),
			("info", Level::INFO),
			("warn", Level::WARN),
			("error", Level::ERROR),
		] {
			let cli = Cli::try_parse_from(["wind", "--log-level", arg])
				.unwrap_or_else(|err| panic!("--log-level {arg} must parse: {err}"));
			assert_eq!(cli.log_level, expected, "--log-level {arg} must select {expected}");
		}
	}

	#[test]
	fn log_level_defaults_to_info_like_the_other_wind_binaries() {
		let cli = Cli::try_parse_from(["wind"]).expect("no-argument invocation must parse");
		assert_eq!(cli.log_level, Level::INFO, "the default log level must not be TRACE");
	}

	#[test]
	fn unknown_log_level_is_rejected_instead_of_silently_ignored() {
		let parse = Cli::try_parse_from(["wind", "--log-level", "verbose"]);
		let err = match parse {
			Ok(_) => panic!("an unknown log level must not be accepted"),
			Err(err) => err,
		};
		let rendered = err.to_string();
		assert!(
			rendered.contains("verbose") && rendered.contains("trace"),
			"the error must name the rejected value and the accepted ones, got: {rendered}"
		);
	}
}
