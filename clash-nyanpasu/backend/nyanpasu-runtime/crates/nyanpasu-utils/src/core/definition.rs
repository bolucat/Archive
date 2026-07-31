//! Core types and command definitions.

use camino::Utf8Path;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{borrow::Cow, ffi::OsStr};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ClashCoreType {
    #[cfg_attr(feature = "serde", serde(rename = "mihomo"))]
    Mihomo,
    #[cfg_attr(feature = "serde", serde(rename = "mihomo-alpha"))]
    MihomoAlpha,
    #[cfg_attr(feature = "serde", serde(rename = "clash-rs"))]
    ClashRust,
    #[cfg_attr(feature = "serde", serde(rename = "clash-rs-alpha"))]
    ClashRustAlpha,
    #[cfg_attr(feature = "serde", serde(rename = "clash"))]
    ClashPremium,
    #[serde(rename = "meow")]
    Meow,
}

impl AsRef<str> for ClashCoreType {
    fn as_ref(&self) -> &str {
        match self {
            ClashCoreType::Mihomo => "mihomo",
            ClashCoreType::MihomoAlpha => "mihomo-alpha",
            ClashCoreType::ClashRust => "clash-rs",
            ClashCoreType::ClashRustAlpha => "clash-rs-alpha",
            ClashCoreType::ClashPremium => "clash",
            ClashCoreType::Meow => "meow",
        }
    }
}

impl std::fmt::Display for ClashCoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl ClashCoreType {
    pub(super) fn get_run_args<'a, P: Into<Cow<'a, Utf8Path>>>(
        &self,
        app_dir: P,
        config_path: P,
    ) -> Vec<Cow<'a, OsStr>> {
        let app_dir: Cow<'a, Utf8Path> = app_dir.into();
        let config_path: Cow<'a, Utf8Path> = config_path.into();
        let app_dir: Cow<'a, OsStr> = Cow::Owned(app_dir.as_ref().as_os_str().to_owned());
        let config_path: Cow<'a, OsStr> = Cow::Owned(config_path.as_ref().as_os_str().to_owned());
        match self {
            ClashCoreType::Mihomo | ClashCoreType::MihomoAlpha | ClashCoreType::Meow => vec![
                Cow::Borrowed(OsStr::new("-m")),
                Cow::Borrowed(OsStr::new("-d")),
                app_dir,
                Cow::Borrowed(OsStr::new("-f")),
                config_path,
            ],
            ClashCoreType::ClashRust | ClashCoreType::ClashRustAlpha => {
                vec![
                    Cow::Borrowed(OsStr::new("-d")),
                    app_dir,
                    Cow::Borrowed(OsStr::new("-c")),
                    config_path,
                ]
            }
            ClashCoreType::ClashPremium => {
                vec![
                    Cow::Borrowed(OsStr::new("-d")),
                    app_dir,
                    Cow::Borrowed(OsStr::new("-f")),
                    config_path,
                ]
            }
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CoreType {
    #[cfg_attr(feature = "serde", serde(rename = "clash"))]
    Clash(ClashCoreType),
    #[cfg_attr(feature = "serde", serde(rename = "singbox"))]
    SingBox, // Maybe we would support this in the 2.x?
}

/// TODO: give a idea to show the meta tags of a core
/// such as the build info, gccgo, llvm-go, amdv3, amdv4 etc.
impl CoreType {
    pub fn get_executable_name(&self) -> &'static str {
        match self {
            CoreType::Clash(ClashCoreType::Mihomo) => {
                constcat::concat!("mihomo", std::env::consts::EXE_SUFFIX)
            }
            CoreType::Clash(ClashCoreType::MihomoAlpha) => {
                constcat::concat!("mihomo-alpha", std::env::consts::EXE_SUFFIX)
            }
            CoreType::Clash(ClashCoreType::ClashRust) => {
                constcat::concat!("clash-rs", std::env::consts::EXE_SUFFIX)
            }
            CoreType::Clash(ClashCoreType::ClashRustAlpha) => {
                constcat::concat!("clash-rs-alpha", std::env::consts::EXE_SUFFIX)
            }
            CoreType::Clash(ClashCoreType::ClashPremium) => {
                constcat::concat!("clash", std::env::consts::EXE_SUFFIX)
            }
            CoreType::Clash(ClashCoreType::Meow) => {
                constcat::concat!("meow", std::env::consts::EXE_SUFFIX)
            }
            CoreType::SingBox => {
                constcat::concat!("singbox", std::env::consts::EXE_SUFFIX)
            }
        }
    }

    pub fn get_supported_cores() -> &'static [CoreType] {
        &[
            CoreType::Clash(ClashCoreType::Mihomo),
            CoreType::Clash(ClashCoreType::MihomoAlpha),
            CoreType::Clash(ClashCoreType::ClashRust),
            CoreType::Clash(ClashCoreType::ClashRustAlpha),
            CoreType::Clash(ClashCoreType::ClashPremium),
            CoreType::Clash(ClashCoreType::Meow),
            // CoreType::SingBox,
        ]
    }

    pub fn get_supported_cores_executables() -> Vec<&'static str> {
        Self::get_supported_cores()
            .iter()
            .map(|core| core.get_executable_name())
            .collect()
    }
}

impl AsRef<str> for CoreType {
    fn as_ref(&self) -> &str {
        match self {
            CoreType::Clash(clash) => clash.as_ref(),
            CoreType::SingBox => "singbox",
        }
    }
}

impl std::fmt::Display for CoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

// TODO: impl downloadable core and core with different tags
pub struct CoreMetaData {
    #[allow(dead_code)] // Reserved for the planned downloadable-core metadata API.
    downloaded: bool,
}

pub type CoresMetaMap = HashMap<CoreType, CoreMetaData>;

#[derive(Debug, Clone)]
pub struct TerminatedPayload {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

pub enum CommandEvent {
    Stdout(String),
    Stderr(String),
    Error(String),
    Terminated(TerminatedPayload),
    DelayCheckpointPass, // Custom event for a delay health check
}
