//! Core kinds, launch profiles, and config checking.

use std::{ffi::OsString, time::Duration};

use camino::Utf8Path;
use nyanpasu_utils::process::ProcessError;

use crate::{
    error::Error,
    log::{CapturedOutput, summarize_output},
};

pub use nyanpasu_core_metadata::ClashCoreKind as CoreKind;

/// The environment variable Mihomo consults for permitted file-system roots.
pub const MIHOMO_SAFE_PATHS_ENV_NAME: &str = "SAFE_PATHS";

/// Logrus honours this when `EnvironmentOverrideColors` is set, which Mihomo
/// does (`log/log.go`). Pinning it to `0` keeps the logfmt layout the log parser
/// expects even when the service inherits a colour-forcing environment.
pub(crate) const CLICOLOR_FORCE_ENV_NAME: &str = "CLICOLOR_FORCE";

#[cfg(windows)]
const SAFE_PATHS_SEPARATOR: &str = ";";
#[cfg(not(windows))]
const SAFE_PATHS_SEPARATOR: &str = ":";

/// The two paths every core is launched with. They are both `Utf8Path` and
/// were adjacent parameters, so a transposed call produces a well-formed
/// command line that names the config as the working directory.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CorePaths<'a> {
    pub working_dir: &'a Utf8Path,
    pub config_path: &'a Utf8Path,
}

/// Launch arguments for this kind.
pub(crate) fn run_args(kind: CoreKind, paths: CorePaths<'_>) -> Result<Vec<OsString>, Error> {
    let dir = OsString::from(paths.working_dir.as_str());
    let cfg = OsString::from(paths.config_path.as_str());
    Ok(match kind {
        // Meow accepts the mihomo CLI for compatibility.
        CoreKind::Mihomo | CoreKind::Meow => {
            vec!["-m".into(), "-d".into(), dir, "-f".into(), cfg]
        }
        CoreKind::ClashRust => vec!["-d".into(), dir, "-c".into(), cfg],
        CoreKind::ClashPremium => vec!["-d".into(), dir, "-f".into(), cfg],
    })
}

/// Extra launch flags to enable the controller for kinds that cannot take it
/// from the config file.
///
/// clash-bin unconditionally overwrites the config's `external_controller_ipc`
/// with its CLI flag value (clash-bin/src/main.rs), so for clash-rs a system
/// IPC endpoint only takes effect when passed as `--controller-ipc`.
pub(crate) fn controller_args(kind: CoreKind, host: &clash_api::Host) -> Vec<OsString> {
    if !matches!(kind, CoreKind::ClashRust) {
        return Vec::new();
    }
    match host {
        clash_api::Host::NamedPipe(path) | clash_api::Host::UnixSocket(path) => {
            vec!["--controller-ipc".into(), path.as_os_str().to_owned()]
        }
        _ => Vec::new(),
    }
}

/// Arguments for a one-shot `-t` config validation run (same for all kinds,
/// matching the legacy `check_config_`).
pub(crate) fn check_args(paths: CorePaths<'_>) -> Vec<OsString> {
    vec![
        "-t".into(),
        "-d".into(),
        paths.working_dir.as_str().into(),
        "-f".into(),
        paths.config_path.as_str().into(),
    ]
}

/// Joins the directories Mihomo may touch into its `SAFE_PATHS` format.
pub fn mihomo_safe_paths(working_dir: &Utf8Path, config_dir: &Utf8Path) -> String {
    [working_dir.as_str(), config_dir.as_str()].join(SAFE_PATHS_SEPARATOR)
}

/// Upper bound on a single `-t` validation run.
///
/// A `-t` run parses a config and exits; it never serves traffic, so a core that
/// has not answered in this long is wedged, not slow. The bound covers every
/// caller — `/core/check`, the staged check inside `apply_config`
/// (`manager/apply.rs:174`) and the pre-flight checks on the start and switch
/// paths (`manager/switching.rs:431,492,496`, which run while the control lock
/// is held) — and sits far enough under the service's 120s request bound
/// (`routing/middleware.rs:31`) that the caller receives this error rather than
/// a dropped future.
pub const CHECK_CONFIG_TIMEOUT: Duration = Duration::from_secs(30);

/// One-shot `-t` config validation, replacing the legacy `check_config_`.
/// A non-zero exit becomes [`Error::ConfigCheckFailed`] with a condensed message,
/// and so does a run that exceeds [`CHECK_CONFIG_TIMEOUT`].
pub async fn check_config(spec: &crate::spec::InstanceSpec) -> Result<(), Error> {
    run_check(spec, CHECK_CONFIG_TIMEOUT).await
}

/// [`check_config`] with an explicit bound. Public only under `test-hooks`:
/// production has exactly one bound, and a test that had to wait
/// [`CHECK_CONFIG_TIMEOUT`] to observe the timeout would not be worth running.
#[cfg(feature = "test-hooks")]
pub async fn check_config_within(
    spec: &crate::spec::InstanceSpec,
    timeout: Duration,
) -> Result<(), Error> {
    run_check(spec, timeout).await
}

async fn run_check(spec: &crate::spec::InstanceSpec, timeout: Duration) -> Result<(), Error> {
    let config_dir = spec
        .config_path
        .parent()
        .ok_or_else(|| Error::ConfigNotFound(spec.config_path.clone()))?;
    let output = nyanpasu_utils::process::Command::new(spec.core.binary_path.as_str())
        .args(check_args(spec.core_paths()))
        .env(
            MIHOMO_SAFE_PATHS_ENV_NAME,
            mihomo_safe_paths(&spec.working_dir, config_dir),
        )
        .env(CLICOLOR_FORCE_ENV_NAME, "0")
        .timeout(timeout)
        .output()
        .await
        .map_err(|error| match error {
            // The process tree is already killed by the time this arrives
            // (`Command::timeout`). Reported as a check failure rather than a
            // process error because that is what the caller asked about: the
            // config did not validate. Same wire kind (`config_check_failed`),
            // and the message names the bound so a slow core is diagnosable.
            ProcessError::Timeout { after } => {
                Error::ConfigCheckFailed(format!("config check timed out after {after:?}"))
            }
            other => Error::Process(other),
        })?;
    if output.success() {
        return Ok(());
    }
    Err(Error::ConfigCheckFailed(summarize_output(
        spec.core.kind,
        CapturedOutput {
            stdout: &output.stdout,
            stderr: &output.stderr,
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

    #[test]
    fn run_args_match_legacy_profiles() {
        let dir = Utf8PathBuf::from("C:/data");
        let cfg = Utf8PathBuf::from("C:/data/config.yaml");
        let paths = CorePaths {
            working_dir: &dir,
            config_path: &cfg,
        };
        let args = run_args(CoreKind::Mihomo, paths).unwrap();
        assert_eq!(
            args,
            ["-m", "-d", "C:/data", "-f", "C:/data/config.yaml"].map(OsString::from)
        );
        let args = run_args(CoreKind::ClashRust, paths).unwrap();
        assert_eq!(
            args,
            ["-d", "C:/data", "-c", "C:/data/config.yaml"].map(OsString::from)
        );
        let args = run_args(CoreKind::ClashPremium, paths).unwrap();
        assert_eq!(
            args,
            ["-d", "C:/data", "-f", "C:/data/config.yaml"].map(OsString::from)
        );
    }

    #[test]
    fn meow_shares_the_mihomo_launch_profile() {
        let dir = Utf8PathBuf::from("/d");
        let cfg = Utf8PathBuf::from("/d/config.yaml");
        let paths = CorePaths {
            working_dir: &dir,
            config_path: &cfg,
        };
        assert_eq!(
            run_args(CoreKind::Meow, paths).unwrap(),
            run_args(CoreKind::Mihomo, paths).unwrap()
        );
    }

    #[test]
    fn safe_paths_joins_with_platform_separator() {
        let joined = mihomo_safe_paths(Utf8Path::new("/a"), Utf8Path::new("/b"));
        #[cfg(windows)]
        assert_eq!(joined, "/a;/b");
        #[cfg(not(windows))]
        assert_eq!(joined, "/a:/b");
    }

    #[test]
    fn check_output_condenses_the_last_error_record() {
        let log = "time=\"2026-07-18T10:00:00Z\" level=info msg=\"start\"\n\
                   time=\"2026-07-18T10:00:01Z\" level=error msg=\"configuration file /x.yaml test failed\"";
        assert_eq!(
            summarize_output(
                CoreKind::Mihomo,
                CapturedOutput {
                    stdout: log,
                    stderr: "",
                },
            ),
            "configuration file /x.yaml test failed"
        );
    }

    #[test]
    fn check_output_keeps_unrecognized_text() {
        assert_eq!(
            summarize_output(
                CoreKind::Mihomo,
                CapturedOutput {
                    stdout: "plain failure",
                    stderr: "",
                },
            ),
            "plain failure"
        );
    }

    #[test]
    fn check_output_no_longer_special_cases_clash_rs() {
        assert_eq!(
            summarize_output(
                CoreKind::ClashRust,
                CapturedOutput {
                    stdout: "",
                    stderr: "Error: invalid config",
                },
            ),
            "Error: invalid config"
        );
    }
}
