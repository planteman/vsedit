//! Keybinding sync and merge.

use serde::{Deserialize, Serialize};

/// A single keybinding entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeybindingEntry {
    pub key: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

impl KeybindingEntry {
    /// Returns `true` if this entry removes a keybinding (command prefixed with `-`).
    pub fn is_removal(&self) -> bool {
        self.command.starts_with('-')
    }

    /// The underlying command name, stripping any `-` removal prefix.
    pub fn command_name(&self) -> &str {
        self.command.strip_prefix('-').unwrap_or(&self.command)
    }
}

/// Merge local and remote keybindings relative to a base.
///
/// Additions from either side are included. Removals (`-command`) take effect.
/// When both sides modify the same `(key, command)` pair to different values,
/// the local version wins.
pub fn merge_keybindings(
    local: &[KeybindingEntry],
    remote: &[KeybindingEntry],
    base: &[KeybindingEntry],
) -> Vec<KeybindingEntry> {
    use std::collections::HashMap;

    type Key = (String, String);
    let make_key = |e: &KeybindingEntry| -> Key { (e.key.clone(), e.command_name().to_string()) };

    let base_map: HashMap<Key, &KeybindingEntry> = base.iter().map(|e| (make_key(e), e)).collect();
    let local_map: HashMap<Key, &KeybindingEntry> =
        local.iter().map(|e| (make_key(e), e)).collect();
    let remote_map: HashMap<Key, &KeybindingEntry> =
        remote.iter().map(|e| (make_key(e), e)).collect();

    let mut all_keys: Vec<Key> = Vec::new();
    for k in local_map
        .keys()
        .chain(remote_map.keys())
        .chain(base_map.keys())
    {
        if !all_keys.contains(k) {
            all_keys.push(k.clone());
        }
    }
    all_keys.sort();

    // Collect removals from both sides.
    let local_removals: Vec<String> = local
        .iter()
        .filter(|e| e.is_removal())
        .map(|e| e.command_name().to_string())
        .collect();
    let remote_removals: Vec<String> = remote
        .iter()
        .filter(|e| e.is_removal())
        .map(|e| e.command_name().to_string())
        .collect();

    let mut result: Vec<KeybindingEntry> = Vec::new();

    for key in &all_keys {
        let in_base = base_map.contains_key(key);
        let local_entry = local_map.get(key);
        let remote_entry = remote_map.get(key);

        // Skip if removed by either side.
        if local_removals.contains(&key.1) || remote_removals.contains(&key.1) {
            continue;
        }

        match (local_entry, remote_entry) {
            (Some(le), Some(_re)) => {
                // Both have it — prefer local for modifications.
                result.push((*le).clone());
            }
            (Some(le), None) => {
                // Only in local — addition or kept from base.
                result.push((*le).clone());
            }
            (None, Some(re)) => {
                if !in_base {
                    // New in remote.
                    result.push((*re).clone());
                }
                // If it was in base but removed from local, skip.
            }
            (None, None) => {}
        }
    }

    result
}
