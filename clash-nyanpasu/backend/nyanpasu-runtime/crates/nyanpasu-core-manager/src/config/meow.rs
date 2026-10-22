//! Meow hot-patches only `tun.enable`; every other change restarts.

use clash_api::{ConfigPatch, TunPatch};
use serde_yaml_ng::Value;

use super::{
    diff::{DiffEntry, diff},
    mihomo::{ConfigChange, ConfigClassificationView, RuntimeProjection},
};

pub(super) fn classify(
    current: ConfigClassificationView<'_>,
    desired: ConfigClassificationView<'_>,
) -> ConfigChange {
    let source = diff(current.source, desired.source);
    let effective = diff(current.effective, desired.effective);
    if source.is_empty() && effective.is_empty() {
        return ConfigChange::Noop;
    }
    let toggles = |entry: &DiffEntry| {
        entry.path == ["tun", "enable"] && entry.new.as_ref().is_some_and(Value::is_bool)
    };
    let enable = match effective.as_slice() {
        [entry] if toggles(entry) && source.iter().all(toggles) => {
            entry.new.as_ref().and_then(Value::as_bool)
        }
        _ => None,
    };
    let Some(enable) = enable else {
        return ConfigChange::Switch;
    };
    // Never send Meow's other TUN fields through the Mihomo DTO. The readback
    // after PATCH, not the core version, decides whether Meow supports it.
    ConfigChange::Patch {
        patch: Box::new(ConfigPatch {
            tun: Some(TunPatch {
                enable,
                ..TunPatch::default()
            }),
            ..ConfigPatch::default()
        }),
        projection: RuntimeProjection {
            expected: vec![(vec!["tun".into(), "enable".into()], Value::Bool(enable))],
        },
    }
}
