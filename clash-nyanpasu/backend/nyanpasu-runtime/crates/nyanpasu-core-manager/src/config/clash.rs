//! Clash-family controller vocabulary: the config keys every supported core
//! uses to expose its RESTful control plane, plus the managed-mode rewrite.
//!
//! Unlike [`super::mihomo`], nothing here is gated on a kind — `prepare_full`
//! runs this for every [`crate::CoreKind`], because `external-controller`,
//! `external-controller-pipe` and `external-controller-unix` are the Clash API
//! transports modelled by [`clash_api::Host`]. A core that does not speak the
//! Clash API (sing-box, …) needs its own module rather than a branch here.

use serde_yaml_ng::{Mapping, Value};

use crate::Feature;

use super::{ConfigInfo, RawController, str_value};

pub(super) const EXTERNAL_CONTROLLER: &str = "external-controller";
pub(super) const EXTERNAL_CONTROLLER_PIPE: &str = "external-controller-pipe";
pub(super) const EXTERNAL_CONTROLLER_UNIX: &str = "external-controller-unix";
pub(super) const SECRET: &str = "secret";

// Keep these cfgs strictly dual to the local key selected below.
#[cfg(windows)]
pub(crate) const LOCAL_TRANSPORT_FEATURE: Feature = Feature::NamedPipeIpc;
#[cfg(not(windows))]
pub(crate) const LOCAL_TRANSPORT_FEATURE: Feature = Feature::UnixSocketIpc;

/// Keys that decide how the manager reaches the core. A change to any of them
/// invalidates the live control channel, so it can never be reconciled in
/// place.
pub(super) const CONTROLLER_FIELDS: &[&str] = &[
    EXTERNAL_CONTROLLER,
    EXTERNAL_CONTROLLER_PIPE,
    EXTERNAL_CONTROLLER_UNIX,
    SECRET,
];

/// Local transports win over HTTP, matching the priority documented in the
/// crate README.
pub(super) fn inspect(document: &Mapping) -> ConfigInfo {
    #[cfg(windows)]
    let local = str_value(document, EXTERNAL_CONTROLLER_PIPE).map(RawController::Pipe);
    #[cfg(not(windows))]
    let local = str_value(document, EXTERNAL_CONTROLLER_UNIX).map(RawController::Unix);

    let controller =
        local.or_else(|| str_value(document, EXTERNAL_CONTROLLER).map(RawController::Http));
    ConfigInfo {
        controller,
        secret: str_value(document, SECRET),
    }
}

pub(super) fn inspect_http(document: &Mapping) -> ConfigInfo {
    ConfigInfo {
        controller: str_value(document, EXTERNAL_CONTROLLER).map(RawController::Http),
        secret: str_value(document, SECRET),
    }
}

/// Repoints the controller at the manager-owned local endpoint.
pub(super) fn rewrite_managed_controller(document: &mut Mapping, endpoint: String) {
    // Unconditional removal isolates overlapping epochs: each must expose only
    // its own epoch-templated local control channel during a graceful switch.
    for field in [
        EXTERNAL_CONTROLLER,
        EXTERNAL_CONTROLLER_PIPE,
        EXTERNAL_CONTROLLER_UNIX,
    ] {
        document.remove(Value::String(field.to_owned()));
    }
    #[cfg(windows)]
    let field = EXTERNAL_CONTROLLER_PIPE;
    #[cfg(not(windows))]
    let field = EXTERNAL_CONTROLLER_UNIX;
    document.insert(Value::String(field.to_owned()), Value::String(endpoint));
}
