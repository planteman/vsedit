//! Ext API: Timeline.
//!
//! RPC bridge between the extension host and the main thread for the timeline API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_timeline";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TimelineMessage {
    RegisterProvider {
        id: String,
        label: String,
        scheme: String,
    },
    UnregisterProvider {
        id: String,
    },
    GetItems {
        provider_id: String,
        uri: String,
    },
    ItemsChanged {
        provider_id: String,
        uri: Option<String>,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub timestamp: u64,
    pub icon_id: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineProvider {
    pub id: String,
    pub label: String,
    pub scheme: String,
}

// ── Bridge ──

pub struct TimelineBridge {
    providers: Vec<TimelineProvider>,
}

impl TimelineBridge {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register_provider(&mut self, provider: TimelineProvider) {
        if !self.providers.iter().any(|p| p.id == provider.id) {
            self.providers.push(provider);
        }
    }

    pub fn unregister_provider(&mut self, id: &str) {
        self.providers.retain(|p| p.id != id);
    }

    pub fn get_provider(&self, id: &str) -> Option<&TimelineProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    pub fn handle_message(&mut self, msg: &TimelineMessage) -> serde_json::Value {
        match msg {
            TimelineMessage::RegisterProvider { id, label, scheme } => {
                self.register_provider(TimelineProvider {
                    id: id.clone(),
                    label: label.clone(),
                    scheme: scheme.clone(),
                });
                serde_json::json!({"registered": true})
            }
            TimelineMessage::UnregisterProvider { id } => {
                self.unregister_provider(id);
                serde_json::json!({"unregistered": true})
            }
            TimelineMessage::GetItems { provider_id, uri } => {
                let found = self.get_provider(provider_id).is_some();
                serde_json::json!({"found": found, "uri": uri, "items": []})
            }
            TimelineMessage::ItemsChanged { provider_id, uri } => {
                serde_json::json!({"provider": provider_id, "uri": uri, "changed": true})
            }
        }
    }
}

impl Default for TimelineBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the timeline extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── Timeline Change Event ──

/// Describes a change in timeline items for a provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineChangeEvent {
    pub provider_id: String,
    pub uri: Option<String>,
    pub reset: bool,
}

// ── Timeline Item Store ──

/// In-memory store keyed by provider ID, holding timeline items.
pub struct TimelineItemStore {
    items: HashMap<String, Vec<TimelineItem>>,
}

impl TimelineItemStore {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Add items for a given provider.
    pub fn add_items(&mut self, provider_id: &str, new_items: Vec<TimelineItem>) {
        self.items
            .entry(provider_id.to_string())
            .or_default()
            .extend(new_items);
    }

    /// Get items for a provider.
    pub fn get_items(&self, provider_id: &str) -> Vec<TimelineItem> {
        self.items.get(provider_id).cloned().unwrap_or_default()
    }

    /// Clear all items for a provider.
    pub fn clear_items(&mut self, provider_id: &str) {
        self.items.remove(provider_id);
    }

    /// Get items within a timestamp range `[start, end]`.
    pub fn get_items_in_range(
        &self,
        provider_id: &str,
        start: u64,
        end: u64,
    ) -> Vec<TimelineItem> {
        self.get_items(provider_id)
            .into_iter()
            .filter(|item| item.timestamp >= start && item.timestamp <= end)
            .collect()
    }

    /// Sort items for a provider by timestamp (ascending).
    pub fn sort_items_by_timestamp(&mut self, provider_id: &str) {
        if let Some(items) = self.items.get_mut(provider_id) {
            items.sort_by_key(|item| item.timestamp);
        }
    }

    /// Merge new items into the store, deduplicating by id and keeping
    /// the item with the newer timestamp.
    pub fn merge_items(&mut self, provider_id: &str, new_items: Vec<TimelineItem>) {
        let entry = self.items.entry(provider_id.to_string()).or_default();
        for new_item in new_items {
            if let Some(existing) = entry.iter_mut().find(|i| i.id == new_item.id) {
                if new_item.timestamp > existing.timestamp {
                    *existing = new_item;
                }
            } else {
                entry.push(new_item);
            }
        }
    }
}

impl Default for TimelineItemStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineBridge {
    /// Return the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Return all providers that match the given scheme.
    pub fn get_providers_for_scheme(&self, scheme: &str) -> Vec<&TimelineProvider> {
        self.providers.iter().filter(|p| p.scheme == scheme).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TimelineMessage::GetItems {
            provider_id: "git".into(),
            uri: "file:///a.rs".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TimelineMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn item_serialization() {
        let item = TimelineItem {
            id: "commit-abc".into(),
            label: "Fix bug".into(),
            description: Some("abc1234".into()),
            timestamp: 1700000000,
            icon_id: Some("git-commit".into()),
            command: Some("git.showCommit".into()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: TimelineItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_provider_lifecycle() {
        let mut bridge = TimelineBridge::new();
        bridge.register_provider(TimelineProvider {
            id: "git".into(),
            label: "Git History".into(),
            scheme: "file".into(),
        });
        assert!(bridge.get_provider("git").is_some());
        bridge.unregister_provider("git");
        assert!(bridge.get_provider("git").is_none());
    }

    #[test]
    fn bridge_handle_get_unknown() {
        let mut bridge = TimelineBridge::new();
        let result = bridge.handle_message(&TimelineMessage::GetItems {
            provider_id: "nope".into(),
            uri: "file:///a".into(),
        });
        assert_eq!(result["found"], false);
    }

    #[test]
    fn bridge_duplicate_register() {
        let mut bridge = TimelineBridge::new();
        let p = TimelineProvider {
            id: "git".into(),
            label: "Git".into(),
            scheme: "file".into(),
        };
        bridge.register_provider(p.clone());
        bridge.register_provider(p);
        assert_eq!(bridge.providers.len(), 1);
    }

    #[test]
    fn store_add_and_get() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem {
                id: "c1".into(),
                label: "Commit 1".into(),
                description: None,
                timestamp: 100,
                icon_id: None,
                command: None,
            },
        ]);
        assert_eq!(store.get_items("git").len(), 1);
        assert!(store.get_items("unknown").is_empty());
    }

    #[test]
    fn store_clear() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem {
                id: "c1".into(),
                label: "Commit 1".into(),
                description: None,
                timestamp: 100,
                icon_id: None,
                command: None,
            },
        ]);
        store.clear_items("git");
        assert!(store.get_items("git").is_empty());
    }

    #[test]
    fn store_items_in_range() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "c1".into(), label: "A".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "c2".into(), label: "B".into(), description: None, timestamp: 200, icon_id: None, command: None },
            TimelineItem { id: "c3".into(), label: "C".into(), description: None, timestamp: 300, icon_id: None, command: None },
        ]);
        let range = store.get_items_in_range("git", 150, 250);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].id, "c2");
    }

    #[test]
    fn store_sort_by_timestamp() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "c2".into(), label: "B".into(), description: None, timestamp: 200, icon_id: None, command: None },
            TimelineItem { id: "c1".into(), label: "A".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        store.sort_items_by_timestamp("git");
        let items = store.get_items("git");
        assert_eq!(items[0].id, "c1");
        assert_eq!(items[1].id, "c2");
    }

    #[test]
    fn store_merge_dedup() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "c1".into(), label: "Old".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        store.merge_items("git", vec![
            TimelineItem { id: "c1".into(), label: "New".into(), description: None, timestamp: 200, icon_id: None, command: None },
            TimelineItem { id: "c2".into(), label: "Extra".into(), description: None, timestamp: 150, icon_id: None, command: None },
        ]);
        let items = store.get_items("git");
        assert_eq!(items.len(), 2);
        assert_eq!(items.iter().find(|i| i.id == "c1").unwrap().label, "New");
    }

    #[test]
    fn bridge_provider_count() {
        let mut bridge = TimelineBridge::new();
        assert_eq!(bridge.provider_count(), 0);
        bridge.register_provider(TimelineProvider {
            id: "git".into(),
            label: "Git".into(),
            scheme: "file".into(),
        });
        assert_eq!(bridge.provider_count(), 1);
    }

    #[test]
    fn bridge_providers_for_scheme() {
        let mut bridge = TimelineBridge::new();
        bridge.register_provider(TimelineProvider { id: "git".into(), label: "Git".into(), scheme: "file".into() });
        bridge.register_provider(TimelineProvider { id: "hg".into(), label: "Hg".into(), scheme: "file".into() });
        bridge.register_provider(TimelineProvider { id: "remote".into(), label: "Remote".into(), scheme: "vscode-remote".into() });
        assert_eq!(bridge.get_providers_for_scheme("file").len(), 2);
        assert_eq!(bridge.get_providers_for_scheme("vscode-remote").len(), 1);
        assert_eq!(bridge.get_providers_for_scheme("unknown").len(), 0);
    }

    #[test]
    fn change_event_serialization() {
        let evt = TimelineChangeEvent {
            provider_id: "git".into(),
            uri: Some("file:///a.rs".into()),
            reset: false,
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: TimelineChangeEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, back);
    }

    #[test]
    fn store_merge_keeps_older_when_newer_missing() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "c1".into(), label: "Original".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        store.merge_items("git", vec![
            TimelineItem { id: "c1".into(), label: "Older".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        let items = store.get_items("git");
        assert_eq!(items[0].label, "Original");
    }
}
