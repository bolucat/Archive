use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;

use crate::enhance::Logs;

pub const RUNTIME_PATCHABLE_KEYS: [&str; 4] = ["allow-lan", "ipv6", "log-level", "mode"];

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct IRuntime {
    pub config: Option<Mapping>,
    // 记录在配置中（包括merge和script生成的）出现过的keys
    // 这些keys不一定都生效
    pub exists_keys: Vec<String>,
    pub chain_logs: IndexMap<String, Logs>,
}

impl IRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    // 这里只更改 allow-lan | ipv6 | log-level | mode
    pub fn patch_config(&mut self, patch: Mapping) {
        tracing::debug!("patching runtime config: {:?}", patch);
        if let Some(config) = self.config.as_mut() {
            RUNTIME_PATCHABLE_KEYS.iter().for_each(|key| {
                if let Some(value) = patch.get(*key).to_owned() {
                    config.insert(key.to_string().into(), value.clone());
                }
            });
        }
    }
}
