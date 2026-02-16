//! Extension sync data and diffing.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Sync data for a single extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionSyncData {
    pub id: String,
    pub version: String,
    pub enabled: bool,
    pub metadata: HashMap<String, Value>,
}

impl ExtensionSyncData {
    pub fn new(id: impl Into<String>, version: impl Into<String>, enabled: bool) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            enabled,
            metadata: HashMap::new(),
        }
    }
}

/// Diff between local and remote extension sets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExtensionSyncDiff {
    pub to_install: Vec<ExtensionSyncData>,
    pub to_uninstall: Vec<String>,
    pub to_enable: Vec<String>,
    pub to_disable: Vec<String>,
}

impl ExtensionSyncDiff {
    pub fn is_empty(&self) -> bool {
        self.to_install.is_empty()
            && self.to_uninstall.is_empty()
            && self.to_enable.is_empty()
            && self.to_disable.is_empty()
    }
}

/// Compute the diff needed to bring local extensions in line with remote.
pub fn compute_extension_diff(
    local: &[ExtensionSyncData],
    remote: &[ExtensionSyncData],
) -> ExtensionSyncDiff {
    let local_map: HashMap<&str, &ExtensionSyncData> =
        local.iter().map(|e| (e.id.as_str(), e)).collect();
    let remote_map: HashMap<&str, &ExtensionSyncData> =
        remote.iter().map(|e| (e.id.as_str(), e)).collect();

    let mut diff = ExtensionSyncDiff::default();

    // Extensions in remote but not in local → install.
    for (id, remote_ext) in &remote_map {
        if !local_map.contains_key(id) {
            diff.to_install.push((*remote_ext).clone());
        }
    }

    // Extensions in local but not in remote → uninstall.
    for id in local_map.keys() {
        if !remote_map.contains_key(id) {
            diff.to_uninstall.push((*id).to_string());
        }
    }

    // Extensions in both — check enabled state.
    for (id, local_ext) in &local_map {
        if let Some(remote_ext) = remote_map.get(id) {
            if local_ext.enabled && !remote_ext.enabled {
                diff.to_disable.push((*id).to_string());
            } else if !local_ext.enabled && remote_ext.enabled {
                diff.to_enable.push((*id).to_string());
            }
        }
    }

    diff.to_install.sort_by(|a, b| a.id.cmp(&b.id));
    diff.to_uninstall.sort();
    diff.to_enable.sort();
    diff.to_disable.sort();

    diff
}
