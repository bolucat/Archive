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
    if current != desired {
        diff_mapping(Some(current), Some(desired), &mut Vec::new(), &mut changes);
    }
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
        diff_mapping(current_mapping, desired_mapping, path, changes);
        return;
    }
    changes.push(DiffEntry {
        path: path.clone(),
        new: desired.cloned(),
    });
}

fn diff_mapping(
    current: Option<&Mapping>,
    desired: Option<&Mapping>,
    path: &mut Vec<String>,
    changes: &mut Vec<DiffEntry>,
) {
    let mut keys = BTreeSet::new();
    if let Some(mapping) = current {
        keys.extend(mapping.keys().filter_map(Value::as_str));
    }
    if let Some(mapping) = desired {
        keys.extend(mapping.keys().filter_map(Value::as_str));
    }
    if keys.is_empty() {
        changes.push(DiffEntry {
            path: path.clone(),
            new: desired.cloned().map(Value::Mapping),
        });
        return;
    }
    for key in keys {
        let yaml_key = Value::String(key.to_owned());
        path.push(key.to_owned());
        diff_value(
            current.and_then(|mapping| mapping.get(&yaml_key)),
            desired.and_then(|mapping| mapping.get(&yaml_key)),
            path,
            changes,
        );
        path.pop();
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(current: &str, desired: &str) -> Vec<(Vec<String>, Option<Value>)> {
        diff(
            &serde_yaml_ng::from_str(current).unwrap(),
            &serde_yaml_ng::from_str(desired).unwrap(),
        )
        .into_iter()
        .map(|entry| (entry.path, entry.new))
        .collect()
    }

    fn entry(path: &[&str], new: Option<&str>) -> (Vec<String>, Option<Value>) {
        (
            path.iter().map(|part| (*part).to_owned()).collect(),
            new.map(|yaml| serde_yaml_ng::from_str(yaml).unwrap()),
        )
    }

    #[test]
    fn equal_independent_documents_and_empty_roots_have_no_changes() {
        let current: Mapping = serde_yaml_ng::from_str("rules: [MATCH,DIRECT]\ndns: {}\n").unwrap();
        let desired = current.clone();
        assert!(diff(&current, &desired).is_empty());
        assert!(diff(&Mapping::new(), &Mapping::new()).is_empty());
    }

    #[test]
    fn nested_changes_are_sorted_and_keep_additions_and_deletions_distinct() {
        assert_eq!(
            changes(
                "z: old\ndns: {z: 1, a: old}\nremoved: null\n",
                "added: null\ndns: {b: 2, a: new}\nz: new\n",
            ),
            vec![
                entry(&["added"], Some("null")),
                entry(&["dns", "a"], Some("new")),
                entry(&["dns", "b"], Some("2")),
                entry(&["dns", "z"], None),
                entry(&["removed"], None),
                entry(&["z"], Some("new")),
            ]
        );
    }

    #[test]
    fn mapping_additions_and_deletions_report_their_leaves() {
        assert_eq!(
            changes("{}", "dns: {enable: true, nested: {value: 1}}"),
            vec![
                entry(&["dns", "enable"], Some("true")),
                entry(&["dns", "nested", "value"], Some("1")),
            ]
        );
        assert_eq!(
            changes("dns: {enable: true, nested: {value: 1}}", "{}"),
            vec![
                entry(&["dns", "enable"], None),
                entry(&["dns", "nested", "value"], None),
            ]
        );
    }

    #[test]
    fn empty_mappings_are_values_when_added_or_removed() {
        assert_eq!(changes("{}", "dns: {}"), vec![entry(&["dns"], Some("{}"))]);
        assert_eq!(changes("dns: {}", "{}"), vec![entry(&["dns"], None)]);
        assert_eq!(
            changes("dns: {enable: true}", "dns: {}"),
            vec![entry(&["dns", "enable"], None)]
        );
    }

    #[test]
    fn scalar_and_mapping_replacements_are_atomic() {
        assert_eq!(
            changes("dns: false", "dns: {enable: true}"),
            vec![entry(&["dns"], Some("{enable: true}"))]
        );
        assert_eq!(
            changes("dns: {enable: true}", "dns: false"),
            vec![entry(&["dns"], Some("false"))]
        );
    }

    #[test]
    fn large_sequences_only_copy_the_changed_output() {
        let mut current = Mapping::new();
        current.insert(
            Value::from("rules"),
            Value::Sequence(vec![Value::from("MATCH,DIRECT"); 10_000]),
        );
        let mut desired = current.clone();
        assert!(diff(&current, &desired).is_empty());
        desired.get_mut("rules").unwrap().as_sequence_mut().unwrap()[9_999] =
            Value::from("MATCH,REJECT");
        let entries = diff(&current, &desired);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, ["rules"]);
        assert_eq!(entries[0].new.as_ref(), desired.get("rules"));
    }

    #[test]
    fn mappings_without_string_keys_keep_the_atomic_fallback() {
        assert_eq!(
            changes("{1: old}", "{1: new}"),
            vec![entry(&[], Some("{1: new}"))]
        );
    }
}
