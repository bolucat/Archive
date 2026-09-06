use crate::logging;
use clap::{Parser, Subcommand};

mod completions;
mod install;
mod restart;
mod rpc;
mod server;
mod start;
mod status;
mod stop;
mod uninstall;
mod update;

// macOS routes start/stop through launchctl instead of the injected manager
// (see utils::service), so the control-plane mock has nothing to observe there.
#[cfg(all(test, not(target_os = "macos")))]
mod test_support;

pub use server::SHUTDOWN_TOKEN as SERVER_SHUTDOWN_TOKEN;

/// The CLI spelling of [`nyanpasu_core_manager::LocalIpcPolicy`].
///
/// A mirror rather than a `ValueEnum` derive on the manager type: core-manager
/// is a library the GUI also links, and it must not take a `clap` dependency.
/// These value names are protocol — `install` writes the chosen one into the
/// persisted service definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum LocalIpcPolicyArg {
    /// Require the platform-local transport; refuse to start a core without it.
    Force,
    /// Use the platform-local transport when the core supports it, otherwise
    /// the config's HTTP `external-controller`.
    Prefer,
    /// Always use the config's HTTP `external-controller`.
    Disable,
}

impl From<LocalIpcPolicyArg> for nyanpasu_core_manager::LocalIpcPolicy {
    fn from(value: LocalIpcPolicyArg) -> Self {
        match value {
            LocalIpcPolicyArg::Force => Self::Force,
            LocalIpcPolicyArg::Prefer => Self::Prefer,
            LocalIpcPolicyArg::Disable => Self::Disable,
        }
    }
}

/// Nyanpasu Service, a privileged service for managing the core service.
///
/// The main entry point for the service, Other commands are the control plane for the service.
///
/// rpc subcommands are shortcuts for client rpc calls,
/// It is useful for testing and debugging service rpc calls.
#[derive(Parser)]
#[command(
    name = "nyanpasu-service",
    version,
    author,
    about,
    long_about,
    disable_version_flag = true,
    arg_required_else_help = true
)]
struct Cli {
    /// Enable verbose logging
    #[clap(short = 'V', long, default_value = "false")]
    verbose: bool,

    /// Print the version
    #[clap(short, long, default_value = "false")]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install the service
    Install(install::InstallCommand),
    /// Uninstall the service
    Uninstall,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Restart the service
    Restart,
    /// Run the server. It should be called by the service manager.
    Server(server::ServerContext), // The main entry point for the service, other commands are the control plane for the service
    /// Get the status of the service
    Status(status::StatusCommand),
    /// Update the service
    Update(update::UpdateCommand),
    /// RPC commands, a shortcut for client rpc calls
    #[command(subcommand)]
    Rpc(rpc::RpcCommand),
    /// Print a shell completion script on stdout
    #[command(hide = true)]
    Completions(completions::CompletionsCommand),
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("service not installed")]
    ServiceNotInstalled,
    #[error("service not running")]
    ServiceAlreadyInstalled,
    #[error("service not running")]
    ServiceAlreadyStopped,
    #[error("service already running")]
    ServiceAlreadyRunning,
    #[error("join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("io error: {0}")]
    Io(
        #[from]
        #[backtrace]
        std::io::Error,
    ),
    #[error("serde error: {0}")]
    SimdError(
        #[from]
        #[backtrace]
        simd_json::Error,
    ),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Commands that only read state or print text, and therefore run unelevated.
///
/// `None` is in the list because a bare `-V` reaches the "no command" arm and
/// must not demand administrator rights to say so.
fn is_unprivileged(command: &Option<Commands>) -> bool {
    match command {
        None
        | Some(Commands::Status(_))
        | Some(Commands::Rpc(_))
        | Some(Commands::Completions(_)) => true,
        // `--check` only compares versions; a real update still writes to the
        // service data dir and still needs elevation.
        Some(Commands::Update(ctx)) => ctx.check,
        _ => false,
    }
}

pub async fn process() -> Result<(), CommandError> {
    let cli = Cli::parse();
    if cli.version {
        print_version();
    }

    if !is_unprivileged(&cli.command) && !crate::utils::must_check_elevation() {
        return Err(CommandError::PermissionDenied);
    }
    if matches!(cli.command, Some(Commands::Server(_))) {
        logging::init(cli.verbose, true)?;
    } else {
        logging::init(cli.verbose, false)?;
    }

    match cli.command {
        Some(Commands::Install(ctx)) => {
            Ok(tokio::task::spawn_blocking(move || install::install(ctx)).await??)
        }
        Some(Commands::Uninstall) => Ok(tokio::task::spawn_blocking(uninstall::uninstall).await??),
        Some(Commands::Start) => Ok(tokio::task::spawn_blocking(start::start).await??),
        Some(Commands::Stop) => Ok(tokio::task::spawn_blocking(stop::stop).await??),
        Some(Commands::Restart) => Ok(tokio::task::spawn_blocking(restart::restart).await??),
        Some(Commands::Server(ctx)) => {
            server::server(ctx).await?;
            Ok(())
        }
        Some(Commands::Status(ctx)) => Ok(status::status(ctx).await?),
        Some(Commands::Update(ctx)) => {
            update::update(ctx).await?;
            Ok(())
        }
        Some(Commands::Rpc(ctx)) => {
            rpc::rpc(ctx).await?;
            Ok(())
        }
        Some(Commands::Completions(ctx)) => {
            completions::completions(ctx);
            Ok(())
        }
        None => {
            eprintln!("No command specified");
            Ok(())
        }
    }
}

pub fn print_version() {
    use crate::consts::*;
    use ansi_str::AnsiStr;
    use chrono::{DateTime, Utc};
    use colored::*;
    use timeago::Formatter;

    let now = Utc::now();
    let formatter = Formatter::new();
    let commit_time =
        formatter.convert_chrono(DateTime::parse_from_rfc3339(COMMIT_DATE).unwrap(), now);
    let commit_time_width = commit_time.len() + COMMIT_DATE.len() + 3;
    let build_time =
        formatter.convert_chrono(DateTime::parse_from_rfc3339(BUILD_DATE).unwrap(), now);
    let build_time_width = build_time.len() + BUILD_DATE.len() + 3;
    let commit_info_width = COMMIT_HASH.len() + COMMIT_AUTHOR.len() + 4;
    let col_width = commit_info_width
        .max(commit_time_width)
        .max(build_time_width)
        .max(BUILD_PLATFORM.len())
        .max(RUSTC_VERSION.len())
        .max(LLVM_VERSION.len())
        + 2;
    let header_width = col_width + 16;
    println!(
        "{} v{} ({} Build)\n",
        APP_NAME,
        APP_VERSION,
        BUILD_PROFILE.yellow()
    );
    println!("╭{:─^width$}╮", " Build Information ", width = header_width);

    let mut line = format!("{} by {}", COMMIT_HASH.green(), COMMIT_AUTHOR.blue());
    let mut pad = col_width - line.ansi_strip().len();
    println!("│{:>14}: {}{}│", "Commit Info", line, " ".repeat(pad));

    line = format!("{} ({})", commit_time.red(), COMMIT_DATE.cyan());
    pad = col_width - line.ansi_strip().len();
    println!("│{:>14}: {}{}│", "Commit Time", line, " ".repeat(pad));

    line = format!("{} ({})", build_time.red(), BUILD_DATE.cyan());
    pad = col_width - line.ansi_strip().len();
    println!("│{:>14}: {}{}│", "Build Time", line, " ".repeat(pad));

    println!(
        "│{:>14}: {:<col_width$}│",
        "Build Target",
        BUILD_PLATFORM.bright_yellow()
    );
    println!(
        "│{:>14}: {:<col_width$}│",
        "Rust Version",
        RUSTC_VERSION.bright_yellow()
    );
    println!(
        "│{:>14}: {:<col_width$}│",
        "LLVM Version",
        LLVM_VERSION.bright_yellow()
    );
    println!("╰{:─^width$}╯", "", width = header_width);
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;

    /// Every invocation that worked before S5. If one of these stops parsing,
    /// the change is breaking and must be reverted, not "fixed" here.
    const LEGACY_INVOCATIONS: &[&[&str]] = &[
        &[
            "nyanpasu-service",
            "install",
            "--user",
            "alice",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-app-dir",
            "app",
        ],
        &["nyanpasu-service", "uninstall"],
        &["nyanpasu-service", "start"],
        &["nyanpasu-service", "stop"],
        &["nyanpasu-service", "restart"],
        &[
            "nyanpasu-service",
            "server",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-app-dir",
            "app",
        ],
        // The exact argv `install` wrote before S10. It has no
        // `--local-ipc-policy`, and an upgraded binary must still start from a
        // service definition containing it — which is what
        // `the_server_local_ipc_policy_defaults_to_disable` pins.
        &[
            "nyanpasu-service",
            "server",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-app-dir",
            "app",
            "--service",
        ],
        &["nyanpasu-service", "status"],
        &["nyanpasu-service", "status", "--json"],
        &["nyanpasu-service", "status", "--skip-service-check"],
        &[
            "nyanpasu-service",
            "status",
            "--json",
            "--skip-service-check",
        ],
        &["nyanpasu-service", "update"],
        &[
            "nyanpasu-service",
            "rpc",
            "start-core",
            "--core-type",
            r#"{"clash":"mihomo"}"#,
            "--config-file",
            "config.yaml",
        ],
        &["nyanpasu-service", "rpc", "stop-core"],
        &["nyanpasu-service", "rpc", "inspect-logs"],
        &["nyanpasu-service", "rpc", "set-dns"],
        &["nyanpasu-service", "rpc", "set-dns", "1.1.1.1", "8.8.8.8"],
        &["nyanpasu-service", "-V"],
        &["nyanpasu-service", "--verbose"],
        &["nyanpasu-service", "-v"],
        &["nyanpasu-service", "--version"],
    ];

    /// Everything S5 adds. All additive: none of these parsed before.
    const NEW_INVOCATIONS: &[&[&str]] = &[
        &["nyanpasu-service", "status", "--exit-code"],
        &["nyanpasu-service", "update", "--check"],
        &["nyanpasu-service", "update", "--from", "candidate.exe"],
        &["nyanpasu-service", "completions", "bash"],
        &["nyanpasu-service", "completions", "zsh"],
        &["nyanpasu-service", "completions", "fish"],
        &["nyanpasu-service", "completions", "powershell"],
        &["nyanpasu-service", "completions", "elvish"],
        &[
            "nyanpasu-service",
            "rpc",
            "start-core",
            "--core-type",
            "mihomo",
            "--config-file",
            "config.yaml",
        ],
        &[
            "nyanpasu-service",
            "rpc",
            "check-config",
            "--core-type",
            "mihomo",
            "--config-file",
            "config.yaml",
        ],
        // The exact argv `install` writes after S10.
        &[
            "nyanpasu-service",
            "server",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-app-dir",
            "app",
            "--local-ipc-policy",
            "disable",
            "--service",
        ],
        &[
            "nyanpasu-service",
            "install",
            "--user",
            "alice",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-app-dir",
            "app",
            "--local-ipc-policy",
            "force",
        ],
    ];

    #[test]
    fn cli_keeps_the_legacy_command_name() {
        assert_eq!(super::Cli::command().get_name(), crate::consts::APP_NAME);
    }

    #[test]
    fn the_cli_definition_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn every_legacy_invocation_still_parses() {
        for argv in LEGACY_INVOCATIONS {
            Cli::try_parse_from(*argv)
                .unwrap_or_else(|err| panic!("{argv:?} no longer parses:\n{err}"));
        }
    }

    #[test]
    fn every_new_invocation_parses() {
        for argv in NEW_INVOCATIONS {
            Cli::try_parse_from(*argv)
                .unwrap_or_else(|err| panic!("{argv:?} does not parse:\n{err}"));
        }
    }

    /// A bare call used to print "No command specified" and exit 0; it now
    /// prints the help. `-V` must still reach the no-command arm.
    #[test]
    fn a_bare_invocation_asks_for_help() {
        let err = Cli::try_parse_from(["nyanpasu-service"])
            .err()
            .expect("a bare invocation must fail");
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    /// `help` is clap-provided and must survive the required-help change.
    #[test]
    fn the_help_subcommand_still_prints_help() {
        let err = Cli::try_parse_from(["nyanpasu-service", "help"])
            .err()
            .expect("help must not parse into a command");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    /// `-V` is verbose and `-v` is version — the inverse of the usual
    /// convention, and deliberately kept until the GUI-coordinated CLI v2.
    #[test]
    fn the_legacy_short_flags_keep_their_meaning() {
        let verbose = Cli::try_parse_from(["nyanpasu-service", "-V"]).unwrap();
        assert!(verbose.verbose);
        assert!(!verbose.version);
        assert!(verbose.command.is_none());

        let version = Cli::try_parse_from(["nyanpasu-service", "-v"]).unwrap();
        assert!(version.version);
        assert!(!version.verbose);
        assert!(version.command.is_none());
    }

    #[test]
    fn install_arguments_fall_back_to_environment_variables() {
        let cli = Cli::command();
        let install = cli.find_subcommand("install").expect("install is declared");
        let envs: Vec<(&str, &str)> = install
            .get_arguments()
            .filter_map(|arg| Some((arg.get_id().as_str(), arg.get_env()?.to_str()?)))
            .collect();

        assert_eq!(
            envs,
            [
                ("user", "NYANPASU_USER"),
                ("nyanpasu_data_dir", "NYANPASU_DATA_DIR"),
                ("nyanpasu_config_dir", "NYANPASU_CONFIG_DIR"),
                ("nyanpasu_app_dir", "NYANPASU_APP_DIR"),
                ("local_ipc_policy", "NYANPASU_LOCAL_IPC_POLICY"),
            ]
        );
    }

    /// `--help` must look exactly as it did before S5.
    #[test]
    fn the_completions_subcommand_is_hidden() {
        let cli = Cli::command();
        assert!(
            cli.find_subcommand("completions")
                .expect("completions is declared")
                .is_hide_set()
        );
        for name in ["install", "status", "update", "rpc", "server"] {
            assert!(
                !cli.find_subcommand(name)
                    .unwrap_or_else(|| panic!("{name} is declared"))
                    .is_hide_set(),
                "{name} must stay visible"
            );
        }
    }

    #[test]
    fn only_the_read_only_commands_skip_the_elevation_check() {
        fn unprivileged(argv: &[&str]) -> bool {
            let cli = Cli::try_parse_from(argv)
                .unwrap_or_else(|err| panic!("{argv:?} does not parse:\n{err}"));
            is_unprivileged(&cli.command)
        }

        assert!(unprivileged(&["nyanpasu-service", "status"]));
        assert!(unprivileged(&["nyanpasu-service", "rpc", "stop-core"]));
        assert!(unprivileged(&["nyanpasu-service", "completions", "bash"]));
        assert!(unprivileged(&["nyanpasu-service", "update", "--check"]));
        assert!(unprivileged(&["nyanpasu-service", "-V"]));

        assert!(!unprivileged(&["nyanpasu-service", "update"]));
        assert!(!unprivileged(&["nyanpasu-service", "start"]));
        assert!(!unprivileged(&["nyanpasu-service", "stop"]));
        assert!(!unprivileged(&["nyanpasu-service", "restart"]));
        assert!(!unprivileged(&["nyanpasu-service", "uninstall"]));
        assert!(!unprivileged(&[
            "nyanpasu-service",
            "install",
            "--user",
            "alice",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-app-dir",
            "app",
        ]));
        assert!(!unprivileged(&[
            "nyanpasu-service",
            "server",
            "--nyanpasu-config-dir",
            "config",
            "--nyanpasu-data-dir",
            "data",
            "--nyanpasu-app-dir",
            "app",
        ]));
    }

    fn server_policy(argv: &[&str]) -> LocalIpcPolicyArg {
        let cli = Cli::try_parse_from(argv)
            .unwrap_or_else(|err| panic!("{argv:?} does not parse:\n{err}"));
        let Some(Commands::Server(ctx)) = cli.command else {
            panic!("{argv:?} is not a server invocation")
        };
        ctx.local_ipc_policy
    }

    /// The transition default (report §4 P2). Two things ride on it: a service
    /// definition written before the argument existed keeps starting, and the
    /// resulting behaviour matches the removed passthrough behavior — the
    /// config's HTTP controller, never rewritten.
    #[test]
    fn the_server_local_ipc_policy_defaults_to_disable() {
        assert_eq!(
            server_policy(&[
                "nyanpasu-service",
                "server",
                "--nyanpasu-data-dir",
                "data",
                "--nyanpasu-config-dir",
                "config",
                "--nyanpasu-app-dir",
                "app",
                "--service",
            ]),
            LocalIpcPolicyArg::Disable
        );
        assert_eq!(
            nyanpasu_core_manager::LocalIpcPolicy::from(LocalIpcPolicyArg::Disable),
            nyanpasu_core_manager::LocalIpcPolicy::Disable
        );
    }

    /// The CLI value names are kebab-case per the S5 conventions, and each maps
    /// onto the manager variant of the same name.
    #[test]
    fn every_local_ipc_policy_value_parses_and_maps() {
        use nyanpasu_core_manager::LocalIpcPolicy;

        let cases = [
            ("force", LocalIpcPolicyArg::Force, LocalIpcPolicy::Force),
            ("prefer", LocalIpcPolicyArg::Prefer, LocalIpcPolicy::Prefer),
            (
                "disable",
                LocalIpcPolicyArg::Disable,
                LocalIpcPolicy::Disable,
            ),
        ];
        for (value, arg, manager) in cases {
            let argv = [
                "nyanpasu-service",
                "server",
                "--nyanpasu-data-dir",
                "data",
                "--nyanpasu-config-dir",
                "config",
                "--nyanpasu-app-dir",
                "app",
                "--local-ipc-policy",
                value,
            ];
            assert_eq!(server_policy(&argv), arg, "{value}");
            assert_eq!(LocalIpcPolicy::from(arg), manager, "{value}");
        }
        assert!(
            Cli::try_parse_from([
                "nyanpasu-service",
                "server",
                "--nyanpasu-data-dir",
                "data",
                "--nyanpasu-config-dir",
                "config",
                "--nyanpasu-app-dir",
                "app",
                "--local-ipc-policy",
                "passthrough",
            ])
            .is_err(),
            "an unknown policy must not parse"
        );
    }

    /// The string `install` persists into the service definition must be the
    /// one the server parses back — the two are written out independently on
    /// purpose (`install::policy_value`).
    #[test]
    fn the_persisted_policy_value_is_what_the_server_parses() {
        for arg in [
            LocalIpcPolicyArg::Force,
            LocalIpcPolicyArg::Prefer,
            LocalIpcPolicyArg::Disable,
        ] {
            let value = super::install::policy_value(arg);
            let argv = [
                "nyanpasu-service",
                "server",
                "--nyanpasu-data-dir",
                "data",
                "--nyanpasu-config-dir",
                "config",
                "--nyanpasu-app-dir",
                "app",
                "--local-ipc-policy",
                value,
                "--service",
            ];
            assert_eq!(server_policy(&argv), arg, "{value}");
        }
    }
}
