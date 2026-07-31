//! Which Clash-family cores speak which optional protocol features, and from
//! which release onwards.
//!
//! The version floors here are the whole point of the module: a core kind alone
//! cannot answer "can I use this", because every capability below arrived in a
//! specific upstream release. Each floor is pinned to the commit that
//! introduced the feature and verified against the first tag containing it.

use std::sync::LazyLock;

use super::{CoreVersion, Support};
use enumset::{EnumSet, EnumSetType};
use schemars::JsonSchema;
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, EnumSetType, Type, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    /// Supports a Windows named pipe for IPC.
    NamedPipeIpc,
    /// Supports a Unix domain socket for IPC.
    UnixSocketIpc,
    /// Supports running without a TCP external controller.
    DisableTcpController,
}

pub trait FeatureSupport {
    /// Whether `self` speaks `feature`.
    ///
    /// A release version is compared after discarding its prerelease label.
    /// Nightly versions are treated as newer than every known floor, while an
    /// unknown version is denied. Pass `None` to retrieve an unresolved
    /// [`Support::Since`] requirement.
    fn supports(&self, feature: Feature, version: Option<&CoreVersion>) -> Support;

    /// Returns only features whose support is decided as [`Support::Yes`].
    fn features(&self, version: Option<&CoreVersion>) -> EnumSet<Feature> {
        EnumSet::all()
            .iter()
            .filter(|feature| matches!(self.supports(*feature, version), Support::Yes))
            .collect()
    }

    /// Returns features that may be supported by a sufficiently recent build.
    ///
    /// This is for display and probe short-circuiting only, never for enabling
    /// a capability.
    fn potential_features(&self) -> EnumSet<Feature> {
        EnumSet::all()
            .iter()
            .filter(|feature| !matches!(self.supports(*feature, None), Support::No))
            .collect()
    }
}

/// Mihomo grew the two controller transports a year apart:
/// `external-controller-unix` in v1.18.4 (commit `ca84ab1a`, absent in
/// v1.18.3), `external-controller-pipe` in v1.18.9 (commit `88bfe7cf`, absent
/// in v1.18.8).
///
/// Unlike clash-rs below, neither needs a later floor for WebSocket: both
/// listeners serve the same `router()` as the TCP controller over plain
/// HTTP/1.1, so `http.Hijacker` — and with it the `/traffic`, `/memory` and
/// `/logs` upgrades — works from the release that added each key.
static MIHOMO_UNIX: LazyLock<VersionReq> =
    LazyLock::new(|| VersionReq::parse(">=1.18.4").expect("valid Mihomo unix feature floor"));
static MIHOMO_PIPE: LazyLock<VersionReq> =
    LazyLock::new(|| VersionReq::parse(">=1.18.9").expect("valid Mihomo pipe feature floor"));

/// clash-rs added both keys at once in v0.9.1 (PR #867), but only the unix
/// listener was usable: it went through `axum::serve`, which always enables
/// upgrades, while the Windows named-pipe listener called
/// `hyper::server::conn::http1::Builder::serve_connection` *without*
/// `with_upgrades`. WebSocket endpoints could therefore never complete a
/// handshake over a named pipe until PR #1068 moved Windows onto `axum::serve`
/// too, first tagged in v0.9.7. That matters here because the `clash-api`
/// client drives `/traffic`, `/memory` and friends over the very same transport
/// it uses for REST, and clash-rs offers no newline-delimited-JSON fallback to
/// degrade to.
static CLASH_RS_UNIX: LazyLock<VersionReq> =
    LazyLock::new(|| VersionReq::parse(">=0.9.1").expect("valid clash-rs unix feature floor"));
static CLASH_RS_PIPE: LazyLock<VersionReq> =
    LazyLock::new(|| VersionReq::parse(">=0.9.7").expect("valid clash-rs pipe feature floor"));

/// `DisableTcpController` is exported capability metadata only and currently
/// gates no core-manager behavior; every core kind reports `Support::No`.
impl FeatureSupport for crate::kind::ClashCoreKind {
    fn supports(&self, feature: Feature, version: Option<&CoreVersion>) -> Support {
        match self {
            crate::kind::ClashCoreKind::Mihomo => match feature {
                Feature::NamedPipeIpc => since(&MIHOMO_PIPE, version),
                Feature::UnixSocketIpc => since(&MIHOMO_UNIX, version),
                Feature::DisableTcpController => Support::No,
            },
            crate::kind::ClashCoreKind::ClashRust => match feature {
                Feature::NamedPipeIpc => since(&CLASH_RS_PIPE, version),
                Feature::UnixSocketIpc => since(&CLASH_RS_UNIX, version),
                Feature::DisableTcpController => Support::No,
            },
            // Clash Premium only ever exposed `external-controller` over TCP.
            crate::kind::ClashCoreKind::ClashPremium => match feature {
                Feature::NamedPipeIpc | Feature::UnixSocketIpc | Feature::DisableTcpController => {
                    Support::No
                }
            },
            // meow-rs advertises `--ext-ctl-unix` and `--ext-ctl-pipe` in
            // `--help`, but only for mihomo CLI compatibility: both `bail!`
            // with "not yet supported" before startup (meow-app/src/main.rs,
            // v0.18.0 and current `main`). Passing either is therefore *worse*
            // than unsupported — it aborts the core instead of degrading, so
            // this must never resolve to `Yes` on the strength of the flag
            // existing. There is no YAML counterpart either: `raw.rs` has only
            // `external-controller`, parsed into a `SocketAddr`, and the repo
            // contains no `UnixListener` at all.
            crate::kind::ClashCoreKind::Meow => match feature {
                Feature::NamedPipeIpc | Feature::UnixSocketIpc | Feature::DisableTcpController => {
                    Support::No
                }
            },
        }
    }
}

/// Resolves a floor against a known version, or hands the requirement back when
/// the version is unknown.
fn since(req: &LazyLock<VersionReq>, version: Option<&CoreVersion>) -> Support {
    match version {
        Some(CoreVersion::Release(version)) if req.matches(version) => Support::Yes,
        Some(CoreVersion::Nightly) => Support::Yes,
        Some(CoreVersion::Release(_) | CoreVersion::Unknown) => Support::No,
        None => Support::Since((**req).clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::ClashCoreKind;

    fn version(raw: &str) -> Option<CoreVersion> {
        Some(CoreVersion::parse(raw))
    }

    #[test]
    fn parses_bare_versions_banners_and_nightly_builds() {
        assert_eq!(
            CoreVersion::parse("v1.18.9"),
            CoreVersion::Release(semver::Version::new(1, 18, 9))
        );
        assert_eq!(
            CoreVersion::parse("Mihomo Meta v1.18.9 linux amd64 with go1.22"),
            CoreVersion::Release(semver::Version::new(1, 18, 9))
        );
        assert_eq!(CoreVersion::parse("alpha-deadbeef"), CoreVersion::Nightly);
        assert_eq!(
            CoreVersion::parse("unrecognized build"),
            CoreVersion::Unknown
        );
        assert!(matches!(
            ClashCoreKind::Mihomo.supports(
                Feature::NamedPipeIpc,
                Some(&CoreVersion::parse(
                    "Mihomo Meta v1.18.9 linux amd64 with go1.22"
                ))
            ),
            Support::Yes
        ));
    }

    #[test]
    fn an_unknown_version_yields_the_requirement() {
        for kind in [ClashCoreKind::Mihomo, ClashCoreKind::ClashRust] {
            for feature in [Feature::NamedPipeIpc, Feature::UnixSocketIpc] {
                assert!(matches!(kind.supports(feature, None), Support::Since(_)));
            }
        }
    }

    #[test]
    fn every_floor_brackets_the_release_that_added_its_transport() {
        for (floor, last_without, first_with) in [
            (&MIHOMO_UNIX, "1.18.3", "1.18.4"),
            (&MIHOMO_PIPE, "1.18.8", "1.18.9"),
            (&CLASH_RS_UNIX, "0.9.0", "0.9.1"),
            (&CLASH_RS_PIPE, "0.9.6", "0.9.7"),
        ] {
            assert!(
                matches!(since(floor, version(last_without).as_ref()), Support::No),
                "`{}` must reject {last_without}",
                **floor
            );
            assert!(
                matches!(since(floor, version(first_with).as_ref()), Support::Yes),
                "`{}` must accept {first_with}",
                **floor
            );
        }
    }

    #[test]
    fn each_core_gates_each_local_ipc_transport() {
        for (kind, feature, last_without, first_with) in [
            (
                ClashCoreKind::Mihomo,
                Feature::NamedPipeIpc,
                "1.18.8",
                "1.18.9",
            ),
            (
                ClashCoreKind::Mihomo,
                Feature::UnixSocketIpc,
                "1.18.3",
                "1.18.4",
            ),
            (
                ClashCoreKind::ClashRust,
                Feature::NamedPipeIpc,
                "0.9.6",
                "0.9.7",
            ),
            (
                ClashCoreKind::ClashRust,
                Feature::UnixSocketIpc,
                "0.9.0",
                "0.9.1",
            ),
        ] {
            assert!(
                matches!(
                    kind.supports(feature, version(last_without).as_ref()),
                    Support::No
                ),
                "{kind} must reject {feature:?} at {last_without}"
            );
            assert!(
                matches!(
                    kind.supports(feature, version(first_with).as_ref()),
                    Support::Yes
                ),
                "{kind} must accept {feature:?} at {first_with}"
            );
        }
    }

    #[test]
    fn nightly_and_prerelease_versions_follow_core_version_policy() {
        assert!(matches!(
            ClashCoreKind::Mihomo.supports(Feature::NamedPipeIpc, Some(&CoreVersion::Nightly)),
            Support::Yes
        ));
        assert!(matches!(
            ClashCoreKind::ClashRust
                .supports(Feature::NamedPipeIpc, version("0.9.7-alpha.1").as_ref()),
            Support::Yes
        ));
        assert!(matches!(
            ClashCoreKind::ClashRust
                .supports(Feature::NamedPipeIpc, version("0.9.6-alpha.1").as_ref()),
            Support::No
        ));
        assert!(matches!(
            ClashCoreKind::Mihomo.supports(Feature::NamedPipeIpc, Some(&CoreVersion::Unknown)),
            Support::No
        ));
    }

    #[test]
    fn unknown_versions_are_not_enabled_but_potential_features_remain_visible() {
        assert!(ClashCoreKind::Mihomo.features(None).is_empty());
        assert!(
            ClashCoreKind::Mihomo
                .potential_features()
                .contains(Feature::NamedPipeIpc)
        );
    }

    #[test]
    fn tcp_only_cores_never_support_local_ipc() {
        for kind in [ClashCoreKind::ClashPremium, ClashCoreKind::Meow] {
            for feature in [
                Feature::NamedPipeIpc,
                Feature::UnixSocketIpc,
                Feature::DisableTcpController,
            ] {
                assert!(matches!(kind.supports(feature, None), Support::No));
                assert!(matches!(
                    kind.supports(feature, version("99.0.0").as_ref()),
                    Support::No
                ));
                assert!(matches!(
                    kind.supports(feature, Some(&CoreVersion::Nightly)),
                    Support::No
                ));
            }
        }
    }

    #[test]
    fn no_core_can_disable_its_tcp_controller_today() {
        for kind in [
            ClashCoreKind::Mihomo,
            ClashCoreKind::ClashRust,
            ClashCoreKind::ClashPremium,
            ClashCoreKind::Meow,
        ] {
            assert!(matches!(
                kind.supports(Feature::DisableTcpController, Some(&CoreVersion::Nightly)),
                Support::No
            ));
        }
    }
}
