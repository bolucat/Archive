//! Execute CLI commands and preserve legacy exit codes without initializing WinAppSDK (§3.8).

use crate::{
    args::Command,
    model,
    netiso::{self, AppContainer, Sid},
};
use snafu::Snafu;

pub const SUCCESS: i32 = 0;
pub const FAILURE: i32 = -1;
pub const SYNTAX: i32 = -10;

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Unable to enumerate AppContainers. Is Windows Firewall running? {source}"))]
    Enumerate { source: netiso::Error },
    #[snafu(display("Enumeration returned no AppContainers (0x0)"))]
    EnumerateEmpty {},
    #[snafu(display("Reading existing CLI exemptions failed: {source}"))]
    ReadExemptions { source: netiso::Error },
    #[snafu(display("Writing CLI exemptions failed: {source}"))]
    SetExemptions { source: netiso::Error },
}

impl Error {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Enumerate { .. } | Self::EnumerateEmpty { .. } => SYNTAX,
            _ => FAILURE,
        }
    }
}

/// The injected operations make decisions testable without writing the firewall table.
fn execute_with(
    command: &Command,
    enumerate: impl FnOnce() -> Result<Vec<AppContainer>, netiso::Error>,
    read: impl FnOnce() -> Result<Vec<Sid>, netiso::Error>,
    write: impl FnOnce(&[Sid]) -> Result<(), netiso::Error>,
) -> Result<(), Error> {
    let sids = if model::clears_without_reading(command) {
        Vec::new()
    } else {
        let apps = enumerate().map_err(|source| Error::Enumerate { source })?;
        // Original GetAppContainers returns null for a successful zero-record result.
        if apps.is_empty() {
            return Err(Error::EnumerateEmpty {});
        }
        let exempted = if model::needs_exemptions(command) {
            read().map_err(|source| Error::ReadExemptions { source })?
        } else {
            Vec::new()
        };
        model::filter_exemptions(command, &apps, &exempted)
    };
    write(&sids).map_err(|source| Error::SetExemptions { source })
}

pub fn run(command: Command) -> i32 {
    if command == Command::Syntax {
        crate::show_message(
            &rust_i18n::t!("cli.syntax"),
            windows::Win32::winuser::MB_ICONINFORMATION as u32,
        );
        return SYNTAX;
    }
    match execute_with(
        &command,
        || netiso::enumerate(false),
        netiso::exempted_sids,
        netiso::set_exempted,
    ) {
        Ok(()) => SUCCESS,
        Err(error) => {
            crate::debug(&format!("EnableLoopback.exe failed: {error}"));
            error.exit_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use windows::Win32::winerror::{EPT_S_NOT_REGISTERED, ERROR_ACCESS_DENIED, ERROR_INVALID_NAME};

    fn app() -> AppContainer {
        AppContainer {
            sid: "S-1-15-2-1".parse().unwrap(),
            users: Vec::new(),
            name: Some("Store".into()),
            display_name: None,
            description: None,
            package_full_name: None,
            binaries: Vec::new(),
            exempt_at_last_check: false,
        }
    }

    #[test]
    fn clear_commands_write_empty_without_enumeration_or_config_read() {
        for command in [Command::None, Command::Deny { name: "*".into() }] {
            let written = Cell::new(false);
            execute_with(
                &command,
                || panic!("must not enumerate"),
                || panic!("must not read"),
                |sids| {
                    assert!(sids.is_empty());
                    written.set(true);
                    Ok(())
                },
            )
            .unwrap();
            assert!(written.get());
        }
    }

    #[test]
    fn allow_all_bypasses_baseline_and_named_filters_read_it() {
        for command in [Command::All, Command::Allow { name: "*".into() }] {
            execute_with(
                &command,
                || Ok(vec![app()]),
                || panic!("must not read"),
                |sids| {
                    assert_eq!(sids, &[app().sid]);
                    Ok(())
                },
            )
            .unwrap();
        }
        for (command, count) in [
            (
                Command::Allow {
                    name: "no-match".into(),
                },
                1,
            ),
            (
                Command::Deny {
                    name: "STORE".into(),
                },
                0,
            ),
        ] {
            let read = Cell::new(false);
            execute_with(
                &command,
                || Ok(vec![app()]),
                || {
                    read.set(true);
                    Ok(vec![app().sid])
                },
                |sids| {
                    assert_eq!(sids.len(), count);
                    Ok(())
                },
            )
            .unwrap();
            assert!(read.get());
        }
    }

    #[test]
    fn enumeration_failures_and_empty_results_exit_minus_ten_without_writing() {
        for enumerated in [
            Ok(Vec::new()),
            Err(netiso::Error::Enumerate {
                force_binaries: false,
                code: EPT_S_NOT_REGISTERED as u32,
            }),
        ] {
            let error = execute_with(
                &Command::All,
                || enumerated,
                || panic!("must not read"),
                |_| panic!("must not write"),
            )
            .unwrap_err();
            assert_eq!(error.exit_code(), -10);
        }
    }

    #[test]
    fn read_and_write_failures_exit_minus_one_with_codes_preserved() {
        let error = execute_with(
            &Command::Allow {
                name: "Store".into(),
            },
            || Ok(vec![app()]),
            || {
                Err(netiso::Error::ReadExemptions {
                    code: ERROR_INVALID_NAME as u32,
                })
            },
            |_| panic!("must not write"),
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), -1);
        assert!(error.to_string().contains("0x7b"));
        let error = execute_with(
            &Command::None,
            || panic!("must not enumerate"),
            || panic!("must not read"),
            |_| {
                Err(netiso::Error::SetExemptionsAccessDenied {
                    sid_count: 0,
                    code: ERROR_ACCESS_DENIED as u32,
                })
            },
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), -1);
        assert!(error.to_string().contains("0x5"));
        assert!(matches!(
            error,
            Error::SetExemptions {
                source: netiso::Error::SetExemptionsAccessDenied { code, .. }
            } if code == ERROR_ACCESS_DENIED as u32
        ));
    }
}
