//! Core-agnostic YAML document diffing primitives.
//!
//! Nothing here knows a specific core's schema; per-core change classification
//! lives in sibling modules such as [`super::mihomo`].

use std::collections::BTreeSet;

use serde_yaml_ng::{Mapping, Value};

#[derive(Debug)]
pub(super) struct DiffEntry {
    pub(super) path: Vec<String>,
    pub(super) new: Option<Value>,
}

pub(super) fn diff(current: &Mapping, desired: &Mapping) -> Vec<DiffEntry> {
    let mut changes = Vec::new();
    diff_value(
        Some(&Value::Mapping(current.clone())),
        Some(&Value::Mapping(desired.clone())),
        &mut Vec::new(),
        &mut changes,
    );
    changes
}

fn diff_value(
    current: Option<&Value>,
    desired: Option<&Value>,
    path: &mut Vec<String>,
    changes: &mut Vec<DiffEntry>,
) {
    if current == desired {
        return;
    }
    let current_mapping = current.and_then(Value::as_mapping);
    let desired_mapping = desired.and_then(Value::as_mapping);
    if (current_mapping.is_some() || desired_mapping.is_some())
        && (current.is_none()
            || current_mapping.is_some() && (desired.is_none() || desired_mapping.is_some()))
    {
        let mut keys = BTreeSet::new();
        if let Some(mapping) = current_mapping {
            keys.extend(mapping.keys().filter_map(Value::as_str));
        }
        if let Some(mapping) = desired_mapping {
            keys.extend(mapping.keys().filter_map(Value::as_str));
        }
        if !keys.is_empty() {
            for key in keys {
                let yaml_key = Value::String(key.to_owned());
                path.push(key.to_owned());
                diff_value(
                    current_mapping.and_then(|mapping| mapping.get(&yaml_key)),
                    desired_mapping.and_then(|mapping| mapping.get(&yaml_key)),
                    path,
                    changes,
                );
                path.pop();
            }
            return;
        }
    }
    changes.push(DiffEntry {
        path: path.clone(),
        new: desired.cloned(),
    });
}

pub(super) fn collect_leaves(
    value: &Value,
    path: &mut Vec<String>,
    output: &mut Vec<(Vec<String>, Value)>,
) {
    if let Some(mapping) = value.as_mapping()
        && !mapping.is_empty()
    {
        for (key, value) in mapping {
            if let Some(key) = key.as_str() {
                path.push(key.to_owned());
                collect_leaves(value, path, output);
                path.pop();
            }
        }
        return;
    }
    output.push((path.clone(), value.clone()));
}

pub(super) fn value_at<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(value, |value, key| {
        value.as_mapping()?.get(Value::String(key.to_owned()))
    })
}
