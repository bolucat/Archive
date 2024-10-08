use serde_yaml::{Mapping, Value};
use std::collections::HashSet;

pub const HANDLE_FIELDS: [&str; 9] = [
    "mode",
    "port",
    "socks-port",
    "mixed-port",
    "allow-lan",
    "log-level",
    "ipv6",
    "secret",
    "external-controller",
];

pub const DEFAULT_FIELDS: [&str; 5] = [
    "proxies",
    "proxy-groups",
    "proxy-providers",
    "rules",
    "rule-providers",
];

pub const OTHERS_FIELDS: [&str; 31] = [
    "dns",
    "tun",
    "ebpf",
    "hosts",
    "script",
    "profile",
    "payload",
    "tunnels",
    "auto-redir",
    "experimental",
    "interface-name",
    "routing-mark",
    "redir-port",
    "tproxy-port",
    "iptables",
    "external-ui",
    "bind-address",
    "authentication",
    "tls",                       // meta
    "sniffer",                   // meta
    "geox-url",                  // meta
    "listeners",                 // meta
    "sub-rules",                 // meta
    "geodata-mode",              // meta
    "unified-delay",             // meta
    "tcp-concurrent",            // meta
    "enable-process",            // meta
    "find-process-mode",         // meta
    "skip-auth-prefixes",        // meta
    "external-controller-tls",   // meta
    "global-client-fingerprint", // meta
];

pub fn use_clash_fields() -> Vec<String> {
    DEFAULT_FIELDS
        .into_iter()
        .chain(HANDLE_FIELDS)
        .chain(OTHERS_FIELDS)
        .map(|s| s.to_string())
        .collect()
}

pub fn use_valid_fields(valid: &[String]) -> Vec<String> {
    let others = Vec::from(OTHERS_FIELDS);

    valid
        .iter()
        .cloned()
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| others.contains(&s.as_str()))
        .chain(DEFAULT_FIELDS.iter().map(|s| s.to_string()))
        .collect()
}

/// 使用白名单过滤配置字段
pub fn use_whitelist_fields_filter(config: Mapping, filter: &[String], enable: bool) -> Mapping {
    if !enable {
        return config;
    }

    let mut ret = Mapping::new();

    for (key, value) in config.into_iter() {
        if let Some(key) = key.as_str() {
            if filter.contains(&key.to_string()) {
                ret.insert(Value::from(key), value);
            }
        }
    }
    ret
}

pub fn use_lowercase(config: Mapping) -> Mapping {
    let mut ret = Mapping::new();
    for (key, value) in config.into_iter() {
        if let Some(key_str) = key.as_str() {
            let mut key_str = String::from(key_str);
            key_str.make_ascii_lowercase();
            // recursive transform the key of the nested mapping
            let value = if let Value::Mapping(value) = value {
                Value::Mapping(use_lowercase(value))
            } else {
                value // TODO: maybe should handle other types, Tagged, Sequence, etc.
            };
            ret.insert(Value::from(key_str), value);
        }
    }
    ret
}

pub fn use_sort(config: Mapping, enable_filter: bool) -> Mapping {
    let mut ret = Mapping::new();

    HANDLE_FIELDS
        .into_iter()
        .chain(OTHERS_FIELDS)
        .chain(DEFAULT_FIELDS)
        .for_each(|key| {
            let key = Value::from(key);
            if let Some(value) = config.get(&key) {
                ret.insert(key, value.clone());
            }
        });

    if !enable_filter {
        let supported_keys: HashSet<&str> = HANDLE_FIELDS
            .into_iter()
            .chain(OTHERS_FIELDS)
            .chain(DEFAULT_FIELDS)
            .collect();

        let config_keys: HashSet<&str> = config.keys().filter_map(|e| e.as_str()).collect();

        config_keys.difference(&supported_keys).for_each(|&key| {
            let key = Value::from(key);
            if let Some(value) = config.get(&key) {
                ret.insert(key, value.clone());
            }
        });
    }

    ret
}

pub fn use_keys(config: &Mapping) -> Vec<String> {
    config
        .iter()
        .filter_map(|(key, _)| key.as_str())
        .map(|s| {
            let mut s = s.to_string();
            s.make_ascii_lowercase();
            s
        })
        .collect()
}
