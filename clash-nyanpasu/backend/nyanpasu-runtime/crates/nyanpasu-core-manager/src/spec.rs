//! Immutable launch specifications and manager options.

use std::time::Duration;

use camino::Utf8PathBuf;
use nyanpasu_utils::process::{Backoff, RestartPolicy};
use tokio_util::sync::CancellationToken;

use crate::{health::HealthPolicy, kind::CoreKind};

#[derive(Debug, Clone)]
pub struct CoreSpec {
    pub kind: CoreKind,
    /// Resolved by the caller (the service keeps `find_binary_path`).
    pub binary_path: Utf8PathBuf,
    /// Authoritative capability version. The manager probes `-v` when absent.
    pub version: Option<String>,
    pub features: Vec<String>,
}

/// Immutable per-epoch launch spec. Changing the config means a new epoch.
#[derive(Debug, Clone)]
pub struct InstanceSpec {
    pub core: CoreSpec,
    pub config_path: Utf8PathBuf,
    pub working_dir: Utf8PathBuf,
    pub pid_file: Option<Utf8PathBuf>,
    pub options: InstanceOptions,
}

#[derive(Debug, Clone)]
pub struct InstanceOptions {
    /// Total limit for the initial start (spawn → readiness threshold).
    pub startup_timeout: Duration,
    pub health: HealthPolicy,
    pub restart_policy: RestartPolicy,
    pub backoff: Backoff,
}

impl Default for InstanceOptions {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(30),
            health: HealthPolicy::default(),
            restart_policy: RestartPolicy::OnFailure { max_restarts: 5 },
            backoff: Backoff::exponential(Duration::from_secs(1), Duration::from_secs(30))
                .with_jitter(),
        }
    }
}

/// The probe/control endpoint an instance actually uses.
#[derive(Debug, Clone)]
pub struct ResolvedController {
    pub host: clash_api::Host,
    pub secret: Option<String>,
}

/// How the manager selects the core's primary controller transport.
///
/// This is the manager's only controller knob. Local IPC means a manager-owned,
/// epoch-parameterized named pipe (Windows) or Unix socket (elsewhere) written
/// into the effective config; the HTTP paths keep the `external-controller`
/// address the source config declares, untouched.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalIpcPolicy {
    /// Require local IPC and fail startup or switching when the core does not
    /// support the platform transport.
    Force,
    /// Use local IPC when supported; otherwise use the HTTP controller from
    /// the source config.
    Prefer,
    /// Always use the HTTP controller from the source config and never rewrite
    /// it to a local transport.
    Disable,
}

#[derive(Debug, Clone)]
pub struct ManagerOptions {
    /// Manager-owned runtime artifact directory: effective configs, pid files,
    /// and by default the epoch-scoped Unix sockets. Required.
    pub runtime_dir: Option<Utf8PathBuf>,
    /// Whether local IPC is required, preferred, or disabled.
    pub local_ipc_policy: LocalIpcPolicy,
    /// Endpoint template containing `{epoch}`; platform default when `None`.
    /// Only consulted when the policy selects local IPC, but validated at
    /// construction under every policy.
    pub controller_template: Option<String>,
    pub control_timeout: Duration,
    pub reconcile_timeout: Duration,
    pub stop_timeout: Duration,
    /// Bound on one injected [`DnsController`](crate::dns::DnsController) call.
    /// The converge tail runs under the control lock, so an unbounded platform
    /// command would freeze every other transaction; a timeout is treated as
    /// "the side effect is uncertain", never as "it did not happen".
    pub dns_timeout: Duration,
    pub cancel_token: CancellationToken,
    /// Write core logs as JSONL under `{runtime_dir}/logs/`. Off means the
    /// directory is never created and nothing is written; an embedder with its
    /// own log stack subscribes to the frames directly instead.
    pub log_sink_enabled: bool,
    /// Rotate the active core-log file once it reaches this size. Soft: a file
    /// overshoots by at most the one record that crossed the line.
    pub log_max_bytes: u64,
    /// How many core-log files to keep, the active one included. With
    /// `log_max_bytes` this is the directory's hard disk budget.
    pub log_max_files: usize,
}

impl Default for ManagerOptions {
    fn default() -> Self {
        Self {
            runtime_dir: None,
            // The behavioural successor of the removed passthrough default:
            // the source config's HTTP controller is authoritative and is
            // never rewritten. Callers that want the epoch-scoped local
            // transport ask for it.
            local_ipc_policy: LocalIpcPolicy::Disable,
            controller_template: None,
            control_timeout: Duration::from_secs(10),
            reconcile_timeout: Duration::from_secs(30),
            stop_timeout: Duration::from_secs(10),
            dns_timeout: Duration::from_secs(10),
            cancel_token: CancellationToken::new(),
            log_sink_enabled: true,
            // 4 MiB x 5 = a 20 MiB ceiling — tighter than the service's own
            // tracing-appender policy, which keeps 7 daily files of unbounded
            // size. At roughly 300-600 B per record a file covers 7k-14k lines.
            log_max_bytes: 4 * 1024 * 1024,
            log_max_files: 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_options_defaults_match_spec() {
        let o = InstanceOptions::default();
        assert_eq!(o.startup_timeout, Duration::from_secs(30));
        assert_eq!(o.health.interval(), Duration::from_millis(250));
        assert_eq!(o.health.timeout(), Duration::from_secs(1));
        assert_eq!(o.health.failure_threshold().get(), 3);
        assert_eq!(o.health.success_threshold().get(), 1);
        assert_eq!(o.health.start_period(), Duration::ZERO);
        assert_eq!(
            o.restart_policy,
            RestartPolicy::OnFailure { max_restarts: 5 }
        );
    }

    /// The compatibility pin for S10: `ManagerOptions::default()` must keep the
    /// removed passthrough behavior — the source config's HTTP controller,
    /// never rewritten. The log-sink defaults are pinned beside it because they
    /// set a disk budget every embedder inherits without asking.
    #[test]
    fn manager_options_default_to_the_non_rewriting_policy() {
        let o = ManagerOptions::default();
        assert_eq!(o.local_ipc_policy, LocalIpcPolicy::Disable);
        assert!(o.controller_template.is_none());
        assert!(o.runtime_dir.is_none());
        assert!(o.log_sink_enabled);
        assert_eq!(o.log_max_bytes, 4 * 1024 * 1024);
        assert_eq!(o.log_max_files, 5);
    }
}
