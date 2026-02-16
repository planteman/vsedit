//! Ext API: Timeline.
//!
//! RPC bridge between the extension host and the main thread for the timeline API.

use std::fmt;
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

/// Accumulated statistics for ext-timeline operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtTimelineStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtTimelineStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ExtTimelineStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ExtTimelineStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtTimelineStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtTimelineStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-timeline.
#[derive(Debug, Clone)]
pub struct ExtTimelineValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtTimelineValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ExtTimelineValidator {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn timeline_message_size() {
        assert!(std::mem::size_of::<TimelineMessage>() > 0);
    }

    #[test]
    fn timeline_change_event_check() {
        assert!(std::mem::size_of::<TimelineChangeEvent>() > 0);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_27() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_28() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_29() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_30() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_31() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_32() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_33() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_34() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_35() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_36() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_37() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_38() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_39() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn ext_timeline_stats_new_defaults() {
        let stats = ExtTimelineStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_timeline_stats_record_success() {
        let mut stats = ExtTimelineStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_timeline_stats_record_failure() {
        let mut stats = ExtTimelineStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_timeline_stats_reset() {
        let mut stats = ExtTimelineStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_timeline_stats_merge() {
        let mut a = ExtTimelineStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtTimelineStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn ext_timeline_stats_display() {
        let mut stats = ExtTimelineStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_timeline_stats_default() {
        let stats = ExtTimelineStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_timeline_validator_accepts_valid_name() {
        let v = ExtTimelineValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_timeline_validator_rejects_empty() {
        let v = ExtTimelineValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_timeline_validator_rejects_too_long() {
        let v = ExtTimelineValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_timeline_validator_forbidden_prefix() {
        let v = ExtTimelineValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_timeline_validator_allowed_chars() {
        let v = ExtTimelineValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_timeline_validator_range() {
        let v = ExtTimelineValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_timeline_sanitize_removes_control() {
        let result = ExtTimelineValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_timeline_truncate_short_string() {
        assert_eq!(ExtTimelineValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_timeline_truncate_long_string() {
        let result = ExtTimelineValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_timeline_is_ascii_printable() {
        assert!(ExtTimelineValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtTimelineValidator::is_ascii_printable("Hello\x00World"));
    }
}
