use time::macros::format_description;
use tracing::{Level, level_filters::LevelFilter};
use tracing_subscriber::{fmt::time::LocalTime, layer::SubscriberExt as _, util::SubscriberInitExt as _};

pub fn init_log(level: Level) -> eyre::Result<()> {
	// Apply the user-supplied level to every wind workspace crate. Previously
	// only `wind`, `wind_core`, `wind_tuic` and `wind_socks` were listed —
	// trace/debug from `wind_naive`, `wind_dns`, `wind_acme`, `wind_base`
	// fell through to the default INFO filter, making `--log-level trace`
	// silently ineffective for half the workspace.
	let filter = tracing_subscriber::filter::Targets::new()
		.with_targets(vec![
			("wind", level),
			("wind_acl", level),
			("wind_acme", level),
			("wind_base", level),
			("wind_core", level),
			("wind_dns", level),
			("wind_geodata", level),
			("wind_naive", level),
			("wind_quic", level),
			("wind_socks", level),
			("wind_tuic", level),
		])
		.with_default(LevelFilter::INFO);
	let registry = tracing_subscriber::registry();
	registry
		.with(filter)
		.with(
			tracing_subscriber::fmt::layer()
				.with_target(true)
				.with_timer(LocalTime::new(format_description!(
					"[year repr:last_two]-[month]-[day] [hour]:[minute]:[second]"
				))),
		)
		.try_init()?;

	Ok(())
}
