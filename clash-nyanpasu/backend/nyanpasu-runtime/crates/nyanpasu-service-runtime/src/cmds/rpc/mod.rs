use std::{borrow::Cow, ffi::OsStr, net::IpAddr};

/// This module is a shortcut for client rpc calls.
/// It is useful for testing and debugging service rpc calls.
use clap::{
    Subcommand, ValueEnum,
    builder::{PossibleValue, TypedValueParser},
};
use nyanpasu_ipc::{
    api::{network::set_dns::NetworkSetDnsReq, status::RevisionIdInfo},
    client::shortcuts::Client,
};
use nyanpasu_utils::core::{ClashCoreType, CoreType};

/// The core names `--core-type` accepts, spelled exactly as they are on the
/// wire (`ClashCoreType`'s serde renames) so a CLI value and an IPC payload can
/// never disagree. `CoreType::SingBox` is deliberately absent: it is not in
/// `CoreType::get_supported_cores`. The legacy JSON form still reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CoreTypeArg {
    #[value(name = "mihomo")]
    Mihomo,
    #[value(name = "mihomo-alpha")]
    MihomoAlpha,
    #[value(name = "clash-rs")]
    ClashRust,
    #[value(name = "clash-rs-alpha")]
    ClashRustAlpha,
    #[value(name = "clash")]
    ClashPremium,
    #[value(name = "meow")]
    Meow,
}

impl From<CoreTypeArg> for CoreType {
    fn from(value: CoreTypeArg) -> Self {
        CoreType::Clash(match value {
            CoreTypeArg::Mihomo => ClashCoreType::Mihomo,
            CoreTypeArg::MihomoAlpha => ClashCoreType::MihomoAlpha,
            CoreTypeArg::ClashRust => ClashCoreType::ClashRust,
            CoreTypeArg::ClashRustAlpha => ClashCoreType::ClashRustAlpha,
            CoreTypeArg::ClashPremium => ClashCoreType::ClashPremium,
            CoreTypeArg::Meow => ClashCoreType::Meow,
        })
    }
}

/// Accepts a core name, or the JSON form the CLI required before S5
/// (`{"clash":"mihomo"}`). The name is tried first; the JSON parse is the
/// fallback, so no old invocation changes meaning.
#[derive(Clone)]
struct CoreTypeParser;

impl TypedValueParser for CoreTypeParser {
    type Value = CoreType;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        match clap::builder::EnumValueParser::<CoreTypeArg>::new().parse_ref(cmd, arg, value) {
            Ok(core) => Ok(core.into()),
            // The enum parser's error already lists the accepted names, so it
            // is the right thing to report when the JSON fallback fails too.
            Err(err) => value.to_str().and_then(legacy_json_core_type).ok_or(err),
        }
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        Some(Box::new(
            CoreTypeArg::value_variants()
                .iter()
                .filter_map(ValueEnum::to_possible_value),
        ))
    }
}

fn legacy_json_core_type(value: &str) -> Option<CoreType> {
    let mut bytes = value.as_bytes().to_vec();
    simd_json::serde::from_slice(&mut bytes).ok()
}

/// `--expected-revision 3:7:fedcba9876543210`, i.e. the three fields the
/// manager's compare-and-swap actually compares, in the order `/status` prints
/// them.
fn parse_revision_id(value: &str) -> Result<RevisionIdInfo, String> {
    let parts: Vec<&str> = value.split(':').collect();
    let [epoch, generation, effective_hash] = parts.as_slice() else {
        return Err(format!(
            "`{value}` is not <epoch>:<generation>:<effective-hash>"
        ));
    };
    Ok(RevisionIdInfo {
        epoch: epoch
            .parse()
            .map_err(|_| format!("`{epoch}` is not an epoch"))?,
        generation: generation
            .parse()
            .map_err(|_| format!("`{generation}` is not a generation"))?,
        effective_hash: (*effective_hash).to_owned(),
    })
}

#[derive(Debug, Subcommand)]
pub enum RpcCommand {
    /// Start specific core with the given config file
    StartCore {
        /// The core type to start
        #[clap(long)]
        #[arg(value_parser = CoreTypeParser)]
        core_type: nyanpasu_utils::core::CoreType,

        /// The path to the core config fileW
        #[clap(long)]
        config_file: std::path::PathBuf,
    },
    /// Stop the running core
    StopCore,
    /// Restart the running core
    RestartCore,
    /// Apply a config to the running core (patch/reload/restart/switch, as the
    /// manager classifies it)
    ApplyConfig {
        /// The core type to run the config with. A different core than the
        /// running one switches cores.
        #[clap(long)]
        #[arg(value_parser = CoreTypeParser)]
        core_type: nyanpasu_utils::core::CoreType,

        /// The path to the core config file
        #[clap(long)]
        config_file: std::path::PathBuf,

        /// Only apply if the running revision still matches, as
        /// `<epoch>:<generation>:<effective-hash>`
        #[clap(long)]
        #[arg(value_parser = parse_revision_id)]
        expected_revision: Option<RevisionIdInfo>,
    },
    /// Dry-run a config against a core binary without touching the running core
    CheckConfig {
        /// The core type to check the config against
        #[clap(long)]
        #[arg(value_parser = CoreTypeParser)]
        core_type: nyanpasu_utils::core::CoreType,

        /// The path to the core config file
        #[clap(long)]
        config_file: std::path::PathBuf,
    },
    /// Clear the manager's quarantine latch
    RecoverCore,
    /// Get the logs of the service
    InspectLogs,
    /// Set the dns servers
    SetDns { dns_servers: Option<Vec<IpAddr>> },
}

pub async fn rpc(commands: RpcCommand) -> Result<(), crate::cmds::CommandError> {
    // let client = Client::new().await?;
    match commands {
        RpcCommand::StartCore {
            core_type,
            config_file,
        } => {
            let client = Client::service_default();

            let payload = nyanpasu_ipc::api::core::start::CoreStartReq {
                core_type: Cow::Borrowed(&core_type),
                config_file: Cow::Borrowed(&config_file),
            };
            client
                .start_core(&payload)
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
        }
        RpcCommand::StopCore => {
            let client = Client::service_default();
            client
                .stop_core()
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
        }
        RpcCommand::RestartCore => {
            let client = Client::service_default();
            client
                .restart_core()
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
        }
        RpcCommand::ApplyConfig {
            core_type,
            config_file,
            expected_revision,
        } => {
            let client = Client::service_default();
            let payload = nyanpasu_ipc::api::core::apply::CoreApplyReq {
                core_type: Cow::Borrowed(&core_type),
                config_file: Cow::Borrowed(&config_file),
                expected_revision,
            };
            let data = client
                .apply_config(&payload)
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
            // The response is the point of the operation — a rolled-back apply
            // is a success at the transport level and must be visible.
            println!(
                "{}",
                serde_json::to_string_pretty(&data)
                    .map_err(|e| crate::cmds::CommandError::Other(e.into()))?
            );
        }
        RpcCommand::CheckConfig {
            core_type,
            config_file,
        } => {
            let client = Client::service_default();
            let payload = nyanpasu_ipc::api::core::check::CoreCheckReq {
                core_type: Cow::Borrowed(&core_type),
                config_file: Cow::Borrowed(&config_file),
            };
            client
                .check_config(&payload)
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
        }
        RpcCommand::RecoverCore => {
            let client = Client::service_default();
            client
                .recover_core()
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
        }
        RpcCommand::InspectLogs => {
            let client = Client::service_default();
            let logs = client
                .inspect_logs()
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
            for log in logs.logs {
                println!("{}", log.trim_matches('\n'));
            }
        }
        RpcCommand::SetDns { dns_servers } => {
            let client = Client::service_default();
            client
                .set_dns(&NetworkSetDnsReq {
                    dns_servers: dns_servers
                        .as_ref()
                        .map(|v| v.iter().map(Cow::Borrowed).collect()),
                })
                .await
                .map_err(|e| crate::cmds::CommandError::Other(e.into()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: &str) -> Result<CoreType, clap::Error> {
        CoreTypeParser.parse_ref(&clap::Command::new("test"), None, OsStr::new(value))
    }

    #[test]
    fn the_core_type_parser_accepts_every_kebab_case_name() {
        let cases = [
            ("mihomo", ClashCoreType::Mihomo),
            ("mihomo-alpha", ClashCoreType::MihomoAlpha),
            ("clash-rs", ClashCoreType::ClashRust),
            ("clash-rs-alpha", ClashCoreType::ClashRustAlpha),
            ("clash", ClashCoreType::ClashPremium),
            ("meow", ClashCoreType::Meow),
        ];
        for (name, expected) in cases {
            assert_eq!(parse(name).unwrap(), CoreType::Clash(expected), "{name}");
        }
    }

    /// The CLI names are the wire names. If a core is ever added to
    /// `get_supported_cores` without a `CoreTypeArg` variant, this fails.
    #[test]
    fn every_supported_core_is_reachable_by_its_wire_name() {
        for core in CoreType::get_supported_cores() {
            let CoreType::Clash(clash) = core else {
                continue;
            };
            assert_eq!(parse(clash.as_ref()).unwrap(), *core, "{clash}");
        }
    }

    #[test]
    fn the_core_type_parser_still_accepts_the_legacy_json_form() {
        assert_eq!(
            parse(r#"{"clash":"mihomo"}"#).unwrap(),
            CoreType::Clash(ClashCoreType::Mihomo)
        );
        assert_eq!(
            parse(r#"{"clash":"clash-rs-alpha"}"#).unwrap(),
            CoreType::Clash(ClashCoreType::ClashRustAlpha)
        );
        assert_eq!(parse(r#""singbox""#).unwrap(), CoreType::SingBox);
    }

    #[test]
    fn the_core_type_parser_rejects_an_unknown_core() {
        let err = parse("not-a-core").unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn the_core_type_parser_advertises_the_kebab_case_names() {
        let names: Vec<String> = CoreTypeParser
            .possible_values()
            .unwrap()
            .map(|value| value.get_name().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "mihomo",
                "mihomo-alpha",
                "clash-rs",
                "clash-rs-alpha",
                "clash",
                "meow"
            ]
        );
    }

    #[test]
    fn the_revision_parser_accepts_the_status_triple() {
        assert_eq!(
            parse_revision_id("3:7:fedcba9876543210").unwrap(),
            RevisionIdInfo {
                epoch: 3,
                generation: 7,
                effective_hash: "fedcba9876543210".to_owned(),
            }
        );
    }

    #[test]
    fn the_revision_parser_rejects_anything_else() {
        for value in ["", "3", "3:7", "3:7:hash:extra", "x:7:hash", "3:y:hash"] {
            assert!(parse_revision_id(value).is_err(), "{value}");
        }
    }
}
