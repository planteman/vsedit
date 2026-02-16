//! Three-way merge algorithm for JSON settings.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Result of a three-way merge.
#[derive(Debug, Clone, PartialEq)]
pub enum MergeResult {
    Clean(Value),
    Conflict(Vec<ConflictEntry>),
}

/// A single conflicting key from a merge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictEntry {
    pub key: String,
    pub local_value: Option<Value>,
    pub remote_value: Option<Value>,
    pub base_value: Option<Value>,
}

/// How to resolve a single conflict.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    AcceptLocal,
    AcceptRemote,
    AcceptBoth,
    Custom(Value),
}

/// Three-way merge of JSON object settings (key-by-key).
///
/// Compares `local` and `remote` against `base` to detect concurrent
/// modifications. Keys changed on only one side are accepted; keys changed
/// on both sides to different values produce conflicts.
pub fn merge_settings(local: &Value, remote: &Value, base: &Value) -> MergeResult {
    let local_obj = local.as_object();
    let remote_obj = remote.as_object();
    let base_obj = base.as_object();

    let (Some(local_obj), Some(remote_obj), Some(base_obj)) =
        (local_obj, remote_obj, base_obj)
    else {
        // Non-object values: if both changed to different values, conflict.
        if local == remote {
            return MergeResult::Clean(local.clone());
        }
        if local == base {
            return MergeResult::Clean(remote.clone());
        }
        if remote == base {
            return MergeResult::Clean(local.clone());
        }
        return MergeResult::Conflict(vec![ConflictEntry {
            key: String::new(),
            local_value: Some(local.clone()),
            remote_value: Some(remote.clone()),
            base_value: Some(base.clone()),
        }]);
    };

    let mut result = base_obj.clone();
    let mut conflicts = Vec::new();

    // Collect all keys from all three objects.
    let mut all_keys: Vec<String> = Vec::new();
    for key in local_obj
        .keys()
        .chain(remote_obj.keys())
        .chain(base_obj.keys())
    {
        if !all_keys.contains(key) {
            all_keys.push(key.clone());
        }
    }
    all_keys.sort();

    for key in &all_keys {
        let base_val = base_obj.get(key);
        let local_val = local_obj.get(key);
        let remote_val = remote_obj.get(key);

        let local_changed = local_val != base_val;
        let remote_changed = remote_val != base_val;

        match (local_changed, remote_changed) {
            (false, false) => {
                // No change — keep base.
                if let Some(v) = base_val {
                    result.insert(key.clone(), v.clone());
                }
            }
            (true, false) => {
                // Only local changed.
                match local_val {
                    Some(v) => result.insert(key.clone(), v.clone()),
                    None => result.remove(key),
                };
            }
            (false, true) => {
                // Only remote changed.
                match remote_val {
                    Some(v) => result.insert(key.clone(), v.clone()),
                    None => result.remove(key),
                };
            }
            (true, true) => {
                if local_val == remote_val {
                    // Both changed to the same value.
                    match local_val {
                        Some(v) => result.insert(key.clone(), v.clone()),
                        None => result.remove(key),
                    };
                } else {
                    // True conflict.
                    conflicts.push(ConflictEntry {
                        key: key.clone(),
                        local_value: local_val.cloned(),
                        remote_value: remote_val.cloned(),
                        base_value: base_val.cloned(),
                    });
                }
            }
        }
    }

    if conflicts.is_empty() {
        MergeResult::Clean(Value::Object(result))
    } else {
        MergeResult::Conflict(conflicts)
    }
}

/// Apply a conflict resolution to produce a final value for one key.
pub fn resolve_conflict(entry: &ConflictEntry, resolution: &ConflictResolution) -> Option<Value> {
    match resolution {
        ConflictResolution::AcceptLocal => entry.local_value.clone(),
        ConflictResolution::AcceptRemote => entry.remote_value.clone(),
        ConflictResolution::AcceptBoth => {
            // Merge both: if both are objects, merge keys; otherwise prefer local.
            match (&entry.local_value, &entry.remote_value) {
                (Some(Value::Object(l)), Some(Value::Object(r))) => {
                    let mut merged = l.clone();
                    for (k, v) in r {
                        merged.entry(k.clone()).or_insert(v.clone());
                    }
                    Some(Value::Object(merged))
                }
                (Some(local), _) => Some(local.clone()),
                (None, remote) => remote.clone(),
            }
        }
        ConflictResolution::Custom(v) => Some(v.clone()),
    }
}
