use constcat::concat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Type, JsonSchema, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CoreKind {
    Clash(ClashCoreKind),
    SingBox,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Type, JsonSchema, Serialize, Deserialize)]
#[repr(u8)]
/// Supported Clash core kinds. This is used to gate the launch arguments and api favors.
///
/// For example:
/// for modern core, such as Mihomo, we prefer the unix socket or named pipe for ipc,
/// while for legacy core, such as Clash Premium, we prefer the http api.
pub enum ClashCoreKind {
    #[serde(rename = "mihomo")]
    Mihomo,
    #[serde(rename = "clash-rs")]
    ClashRust,
    #[serde(rename = "clash")]
    ClashPremium,
    #[serde(rename = "meow")]
    Meow,
}

impl AsRef<str> for ClashCoreKind {
    fn as_ref(&self) -> &str {
        match self {
            ClashCoreKind::Mihomo => "mihomo",
            ClashCoreKind::ClashRust => "clash-rs",
            ClashCoreKind::ClashPremium => "clash",
            ClashCoreKind::Meow => "meow",
        }
    }
}

impl std::fmt::Display for ClashCoreKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_ref())
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Type, Serialize, Deserialize)]
#[repr(u8)]
/// The resource variant of a Clash core. This is used to determine which resource to download for a given core kind.
pub enum ClashCoreResourceVariant {
    #[serde(rename = "mihomo")]
    Mihomo,
    #[serde(rename = "mihomo-alpha")]
    MihomoAlpha,
    #[serde(rename = "clash-rs")]
    ClashRust,
    #[serde(rename = "clash-rs-alpha")]
    ClashRustAlpha,
    #[serde(rename = "clash")]
    ClashPremium,
    #[serde(rename = "meow")]
    Meow,
}

impl ClashCoreResourceVariant {
    #[inline]
    pub fn binary_name(&self) -> &'static str {
        use std::env::consts::*;
        match self {
            ClashCoreResourceVariant::Mihomo => concat!("mihomo", EXE_SUFFIX),
            ClashCoreResourceVariant::MihomoAlpha => {
                concat!("mihomo-alpha", EXE_SUFFIX)
            }
            ClashCoreResourceVariant::ClashRust => {
                concat!("clash-rs", EXE_SUFFIX)
            }
            ClashCoreResourceVariant::ClashRustAlpha => {
                concat!("clash-rs-alpha", EXE_SUFFIX)
            }
            ClashCoreResourceVariant::ClashPremium => {
                concat!("clash", EXE_SUFFIX)
            }
            ClashCoreResourceVariant::Meow => concat!("meow", EXE_SUFFIX),
        }
    }
}
