use std::{ops::Deref, sync::Arc, time::Duration};

use clap::Parser as _;
use tracing::{Level, info, warn};
use wind_core::{
	AppContext,
	dispatcher::{Dispatcher, OutboundAsAction, Router},
	inbound::AbstractInbound,
};
use wind_naive::NaiveOutbound;
use wind_socks::inbound::SocksInbound;
use wind_tuic::quinn::outbound::TuicOutbound;

use crate::conf::runtime::{InboundOpts, InboundRuntime, OutboundOpts, OutboundRuntime};

enum InboundHandle {
	Socks(SocksInbound),
}

impl AbstractInbound for InboundHandle {
	async fn listen(&self, cb: &impl wind_core::InboundCallback) -> eyre::Result<()> {
		match self {
			InboundHandle::Socks(s) => s.listen(cb).await,
		}
	}
}

// Router — always forwards to the first outbound (TODO: ACL rules)
struct DefaultRouter {
	default: String,
}

impl Router for DefaultRouter {
	async fn route(
		&self,
		_target: &wind_core::types::TargetAddr,
		_is_tcp: bool,
	) -> eyre::Result<wind_core::dispatcher::RouteAction> {
		Ok(wind_core::dispatcher::RouteAction::Forward(self.default.clone()))
	}
}

struct Manager<R: Router> {
	inbound: InboundHandle,
	dispatcher: Arc<Dispatcher<R>>,
}

impl<R: Router> Manager<R> {
	async fn run(self: Arc<Self>) -> eyre::Result<()> {
		self.inbound.listen(self.dispatcher.deref()).await?;
		Ok(())
	}
}

use crate::{cli::Cli, conf::persistent::PersistentConfig};
mod cli;
mod conf;
mod log;

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

	let runtime_config = conf::runtime::Config::from_persist(persistent_config);
	let ctx = Arc::new(AppContext::default());

	let dispatcher = build_dispatcher(runtime_config.outbounds, ctx.clone()).await?;
	let dispatcher = Arc::new(dispatcher);

	for ib in runtime_config.inbounds {
		start_inbound(ib, &dispatcher, &ctx).await?;
	}

	wind_core::shutdown_signal().await;
	info!(target: "wind_main", "shutdown signal received, shutting down");
	ctx.token.cancel();
	ctx.tasks.close();
	// A drain timeout is normal when long-lived sessions are still open; treat
	// it as a graceful (if forced) shutdown rather than a process error exit.
	if tokio::time::timeout(Duration::from_secs(10), ctx.tasks.wait()).await.is_err() {
		warn!(target: "wind_main", "shutdown drain timed out after 10s; forcing exit");
	}

	info!(target: "wind_main", "Shutdown complete");
	Ok(())
}

async fn build_dispatcher(outbounds: Vec<OutboundRuntime>, ctx: Arc<AppContext>) -> eyre::Result<Dispatcher<DefaultRouter>> {
	let default_tag = outbounds.first().map(|o| o.tag.clone()).unwrap_or_else(|| "default".into());

	let mut disp = Dispatcher::new(DefaultRouter { default: default_tag });

	for ob in outbounds {
		let tag = ob.tag;
		match ob.opts {
			OutboundOpts::Tuic(opts) => {
				let out = TuicOutbound::new(ctx.clone(), opts).await?;
				disp.add_handler(&tag, Arc::new(OutboundAsAction { inner: out }));
				info!(target: "wind_boot", "outbound '{tag}' [tuic]");
			}
			OutboundOpts::Naive(opts) => {
				let out = NaiveOutbound::new(opts).await?;
				disp.add_handler(&tag, Arc::new(OutboundAsAction { inner: out }));
				info!(target: "wind_boot", "outbound '{tag}' [naive]");
			}
		}
	}

	Ok(disp)
}

async fn start_inbound(
	ib: InboundRuntime,
	dispatcher: &Arc<Dispatcher<DefaultRouter>>,
	ctx: &Arc<AppContext>,
) -> eyre::Result<()> {
	let tag = ib.tag;

	match ib.opts {
		InboundOpts::Socks(opts) => {
			let addr = opts.listen_addr;
			let inbound = SocksInbound::new(opts, ctx.token.child_token());
			let handle = InboundHandle::Socks(inbound);

			let mgr = Arc::new(Manager {
				inbound: handle,
				dispatcher: dispatcher.clone(),
			});

			ctx.tasks.spawn(async move {
				mgr.run().await?;
				eyre::Ok(())
			});

			info!(target: "wind_boot", "inbound '{tag}' [socks] ({addr})");
		}
	}

	Ok(())
}
