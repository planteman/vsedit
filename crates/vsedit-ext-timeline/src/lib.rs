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

impl TimelineItemStore {
    /// Return all provider IDs that have items.
    pub fn all_provider_ids(&self) -> Vec<&str> {
        self.items.keys().map(|s| s.as_str()).collect()
    }

    /// Total number of items across all providers.
    pub fn total_item_count(&self) -> usize {
        self.items.values().map(|v| v.len()).sum()
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

// ---------------------------------------------------------------------------
// TimelineEventFilter
// ---------------------------------------------------------------------------

/// Filter for querying timeline items by various criteria.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimelineEventFilter {
    /// Filter by minimum timestamp (inclusive).
    pub since: Option<u64>,
    /// Filter by maximum timestamp (inclusive).
    pub until: Option<u64>,
    /// Filter by provider IDs (if non-empty, only items from these providers match).
    pub provider_ids: Vec<String>,
    /// Filter by label substring (case-insensitive).
    pub label_contains: Option<String>,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl TimelineEventFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn since(mut self, ts: u64) -> Self {
        self.since = Some(ts);
        self
    }

    pub fn until(mut self, ts: u64) -> Self {
        self.until = Some(ts);
        self
    }

    pub fn provider(mut self, id: impl Into<String>) -> Self {
        self.provider_ids.push(id.into());
        self
    }

    pub fn label_contains(mut self, query: impl Into<String>) -> Self {
        self.label_contains = Some(query.into());
        self
    }

    pub fn limit(mut self, max: usize) -> Self {
        self.limit = Some(max);
        self
    }

    /// Test whether a single timeline item matches this filter.
    pub fn matches_item(&self, item: &TimelineItem) -> bool {
        if let Some(since) = self.since {
            if item.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until {
            if item.timestamp > until {
                return false;
            }
        }
        if let Some(ref query) = self.label_contains {
            if !item.label.to_lowercase().contains(&query.to_lowercase()) {
                return false;
            }
        }
        true
    }

    /// Apply this filter to a store, returning matching items.
    pub fn apply(&self, store: &TimelineItemStore) -> Vec<TimelineItem> {
        let mut results = Vec::new();
        let provider_ids: Vec<&str> = if self.provider_ids.is_empty() {
            store.all_provider_ids()
        } else {
            self.provider_ids.iter().map(|s| s.as_str()).collect()
        };

        for pid in provider_ids {
            let items = store.get_items(pid);
            for item in items {
                if self.matches_item(&item) {
                    results.push(item);
                }
            }
        }

        // Sort by timestamp descending
        results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        if let Some(limit) = self.limit {
            results.truncate(limit);
        }

        results
    }
}

// ---------------------------------------------------------------------------
// TimelinePaginator
// ---------------------------------------------------------------------------

/// Paginator for lazy-loading timeline entries.
#[derive(Debug, Clone)]
pub struct TimelinePaginator {
    /// Current page (0-based).
    pub page: usize,
    /// Items per page.
    pub page_size: usize,
    /// Total items available (may be unknown initially).
    pub total_items: Option<usize>,
}

impl TimelinePaginator {
    pub fn new(page_size: usize) -> Self {
        Self {
            page: 0,
            page_size,
            total_items: None,
        }
    }

    /// Set the total number of items.
    pub fn set_total(&mut self, total: usize) {
        self.total_items = Some(total);
    }

    /// Advance to the next page. Returns false if already at the last page.
    pub fn next_page(&mut self) -> bool {
        if let Some(total) = self.total_items {
            if (self.page + 1) * self.page_size >= total {
                return false;
            }
        }
        self.page += 1;
        true
    }

    /// Go back to the previous page. Returns false if already at page 0.
    pub fn prev_page(&mut self) -> bool {
        if self.page == 0 {
            return false;
        }
        self.page -= 1;
        true
    }

    /// Reset to the first page.
    pub fn reset(&mut self) {
        self.page = 0;
    }

    /// The starting offset for the current page.
    pub fn offset(&self) -> usize {
        self.page * self.page_size
    }

    /// Total number of pages, or None if total is unknown.
    pub fn total_pages(&self) -> Option<usize> {
        self.total_items.map(|total| {
            if total == 0 { 1 } else { (total + self.page_size - 1) / self.page_size }
        })
    }

    /// Whether there are more pages after the current one.
    pub fn has_next(&self) -> bool {
        match self.total_items {
            Some(total) => (self.page + 1) * self.page_size < total,
            None => true, // assume more when total is unknown
        }
    }

    /// Whether there is a previous page.
    pub fn has_prev(&self) -> bool {
        self.page > 0
    }

    /// Apply pagination to a vec of items.
    pub fn paginate<T: Clone>(&self, items: &[T]) -> Vec<T> {
        let start = self.offset();
        let end = (start + self.page_size).min(items.len());
        if start >= items.len() {
            Vec::new()
        } else {
            items[start..end].to_vec()
        }
    }
}

/// Create a paginator for timeline items.
pub fn timeline_paginator(page_size: usize) -> TimelinePaginator {
    TimelinePaginator::new(page_size)
}

// ---------------------------------------------------------------------------
// Timeline export / import
// ---------------------------------------------------------------------------

/// Export timeline items to a JSON string.
pub fn timeline_export(store: &TimelineItemStore, provider_id: &str) -> String {
    let items = store.get_items(provider_id);
    serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
}

/// Export all items from all providers to a JSON object.
pub fn timeline_export_all(store: &TimelineItemStore) -> String {
    let mut map = serde_json::Map::new();
    for pid in store.all_provider_ids() {
        let items = store.get_items(pid);
        map.insert(pid.to_string(), serde_json::to_value(&items).unwrap_or(serde_json::Value::Array(vec![])));
    }
    serde_json::to_string_pretty(&map).unwrap_or_else(|_| "{}".to_string())
}

/// Import timeline items from a JSON string into a store.
pub fn timeline_import(store: &mut TimelineItemStore, provider_id: &str, json: &str) -> Result<usize, String> {
    let items: Vec<TimelineItem> = serde_json::from_str(json)
        .map_err(|e| format!("failed to parse timeline JSON: {e}"))?;
    let count = items.len();
    store.add_items(provider_id, items);
    Ok(count)
}

// ── TimelineItem extensions ──

impl TimelineItem {
    pub fn age_secs(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_recent(&self, now: u64, threshold: u64) -> bool {
        self.age_secs(now) <= threshold
    }

    pub fn has_description(&self) -> bool {
        self.description.as_ref().map_or(false, |d| !d.is_empty())
    }

    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        if self.label.to_lowercase().contains(&q) {
            return true;
        }
        if let Some(ref desc) = self.description {
            if desc.to_lowercase().contains(&q) {
                return true;
            }
        }
        false
    }
}

impl fmt::Display for TimelineItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} (t={})", self.id, self.label, self.timestamp)
    }
}

// ── TimelineItemStore extensions ──

impl TimelineItemStore {
    pub fn item_count(&self, provider_id: &str) -> usize {
        self.items.get(provider_id).map_or(0, |v| v.len())
    }

    pub fn is_empty(&self) -> bool {
        self.items.values().all(|v| v.is_empty())
    }

    pub fn find_by_label(&self, provider_id: &str, label: &str) -> Option<&TimelineItem> {
        self.items
            .get(provider_id)
            .and_then(|v| v.iter().find(|item| item.label == label))
    }

    pub fn oldest(&self, provider_id: &str) -> Option<&TimelineItem> {
        self.items
            .get(provider_id)
            .and_then(|v| v.iter().min_by_key(|item| item.timestamp))
    }

    pub fn newest(&self, provider_id: &str) -> Option<&TimelineItem> {
        self.items
            .get(provider_id)
            .and_then(|v| v.iter().max_by_key(|item| item.timestamp))
    }

    pub fn items_per_provider(&self) -> HashMap<&str, usize> {
        self.items
            .iter()
            .map(|(k, v)| (k.as_str(), v.len()))
            .collect()
    }
}

impl fmt::Display for TimelineItemStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let providers = self.items.len();
        let total = self.total_item_count();
        write!(f, "TimelineItemStore({} providers, {} items)", providers, total)
    }
}

impl<'a> IntoIterator for &'a TimelineItemStore {
    type Item = (&'a String, &'a Vec<TimelineItem>);
    type IntoIter = std::collections::hash_map::Iter<'a, String, Vec<TimelineItem>>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

// ── TimelineChangeEvent extensions ──

impl TimelineChangeEvent {
    pub fn is_reset(&self) -> bool {
        self.reset
    }

    pub fn has_uri(&self) -> bool {
        self.uri.is_some()
    }
}

impl fmt::Display for TimelineChangeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let uri_str = self.uri.as_deref().unwrap_or("<all>");
        let kind = if self.reset { "reset" } else { "changed" };
        write!(f, "TimelineChangeEvent({}, {}, {})", self.provider_id, uri_str, kind)
    }
}

// ── TimelineEventFilter extensions ──

impl TimelineEventFilter {
    pub fn is_empty(&self) -> bool {
        self.since.is_none()
            && self.until.is_none()
            && self.provider_ids.is_empty()
            && self.label_contains.is_none()
            && self.limit.is_none()
    }
}

// ── TimelinePaginator extensions ──

impl TimelinePaginator {
    pub fn is_first_page(&self) -> bool {
        self.page == 0
    }

    pub fn is_last_page(&self) -> bool {
        match self.total_items {
            Some(total) => (self.page + 1) * self.page_size >= total,
            None => false,
        }
    }

    pub fn current_page_one_indexed(&self) -> usize {
        self.page + 1
    }
}

// ── TimelineSummary ──

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSummary {
    pub provider_count: usize,
    pub total_items: usize,
    pub min_timestamp: Option<u64>,
    pub max_timestamp: Option<u64>,
    pub items_with_description: usize,
    pub items_with_command: usize,
}

impl TimelineSummary {
    pub fn from_store(store: &TimelineItemStore) -> Self {
        let mut total_items = 0usize;
        let mut min_ts: Option<u64> = None;
        let mut max_ts: Option<u64> = None;
        let mut with_desc = 0usize;
        let mut with_cmd = 0usize;
        let mut provider_count = 0usize;

        for (_, items) in store {
            provider_count += 1;
            for item in items {
                total_items += 1;
                let ts = item.timestamp;
                min_ts = Some(min_ts.map_or(ts, |m: u64| m.min(ts)));
                max_ts = Some(max_ts.map_or(ts, |m: u64| m.max(ts)));
                if item.has_description() {
                    with_desc += 1;
                }
                if item.command.is_some() {
                    with_cmd += 1;
                }
            }
        }

        Self {
            provider_count,
            total_items,
            min_timestamp: min_ts,
            max_timestamp: max_ts,
            items_with_description: with_desc,
            items_with_command: with_cmd,
        }
    }

    pub fn time_span(&self) -> Option<u64> {
        match (self.min_timestamp, self.max_timestamp) {
            (Some(min), Some(max)) => Some(max - min),
            _ => None,
        }
    }
}

impl fmt::Display for TimelineSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TimelineSummary(providers={}, items={}, span={:?})",
            self.provider_count,
            self.total_items,
            self.time_span()
        )
    }
}

// ---------------------------------------------------------------------------
// Provider priority registry
// ---------------------------------------------------------------------------

/// Registry that tracks providers with explicit priority ordering.
/// Lower priority values are higher priority (evaluated first).
pub struct ProviderPriorityRegistry {
    entries: Vec<ProviderPriorityEntry>,
}

#[derive(Debug, Clone)]
struct ProviderPriorityEntry {
    provider: TimelineProvider,
    priority: i32,
}

impl ProviderPriorityRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a provider with a given priority. Lower values = higher priority.
    /// If a provider with the same ID already exists, its priority is updated.
    pub fn register(&mut self, provider: TimelineProvider, priority: i32) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.provider.id == provider.id) {
            entry.priority = priority;
            entry.provider = provider;
        } else {
            self.entries.push(ProviderPriorityEntry { provider, priority });
        }
        self.entries.sort_by_key(|e| e.priority);
    }

    /// Unregister a provider by ID.
    pub fn unregister(&mut self, id: &str) {
        self.entries.retain(|e| e.provider.id != id);
    }

    /// Return providers sorted by priority (lowest value first).
    pub fn providers_by_priority(&self) -> Vec<&TimelineProvider> {
        self.entries.iter().map(|e| &e.provider).collect()
    }

    /// Return the priority assigned to a provider, if registered.
    pub fn get_priority(&self, id: &str) -> Option<i32> {
        self.entries.iter().find(|e| e.provider.id == id).map(|e| e.priority)
    }

    /// Return providers matching a scheme, still sorted by priority.
    pub fn providers_for_scheme(&self, scheme: &str) -> Vec<&TimelineProvider> {
        self.entries
            .iter()
            .filter(|e| e.provider.scheme == scheme)
            .map(|e| &e.provider)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ProviderPriorityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cross-provider deduplication
// ---------------------------------------------------------------------------

/// Deduplicate timeline items across multiple providers.
/// Items with the same `id` are collapsed: the item with the latest timestamp wins.
pub fn deduplicate_items(items: &[TimelineItem]) -> Vec<TimelineItem> {
    let mut seen: HashMap<String, TimelineItem> = HashMap::new();
    for item in items {
        match seen.get(&item.id) {
            Some(existing) if existing.timestamp >= item.timestamp => {}
            _ => {
                seen.insert(item.id.clone(), item.clone());
            }
        }
    }
    let mut result: Vec<TimelineItem> = seen.into_values().collect();
    result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    result
}

/// Collect items from a store across all providers, deduplicate, and return sorted.
pub fn deduplicate_store(store: &TimelineItemStore) -> Vec<TimelineItem> {
    let mut all = Vec::new();
    for pid in store.all_provider_ids() {
        all.extend(store.get_items(pid));
    }
    deduplicate_items(&all)
}

// ---------------------------------------------------------------------------
// Timeline item action / command association
// ---------------------------------------------------------------------------

/// Maps timeline item IDs to a command identifier and optional arguments.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimelineAction {
    pub command_id: String,
    pub title: String,
    pub args: Vec<String>,
}

/// Registry that associates timeline items with actions.
pub struct TimelineActionRegistry {
    actions: HashMap<String, Vec<TimelineAction>>,
}

impl TimelineActionRegistry {
    pub fn new() -> Self {
        Self {
            actions: HashMap::new(),
        }
    }

    /// Bind an action to a timeline item ID.
    pub fn bind(&mut self, item_id: &str, action: TimelineAction) {
        self.actions
            .entry(item_id.to_string())
            .or_default()
            .push(action);
    }

    /// Remove all actions for a given item ID.
    pub fn unbind(&mut self, item_id: &str) {
        self.actions.remove(item_id);
    }

    /// Get the actions associated with an item.
    pub fn get_actions(&self, item_id: &str) -> &[TimelineAction] {
        self.actions.get(item_id).map_or(&[], |v| v.as_slice())
    }

    /// Return true if the item has at least one associated action.
    pub fn has_actions(&self, item_id: &str) -> bool {
        self.actions.get(item_id).map_or(false, |v| !v.is_empty())
    }

    /// Total number of item-action bindings.
    pub fn total_bindings(&self) -> usize {
        self.actions.values().map(|v| v.len()).sum()
    }
}

impl Default for TimelineActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Icon / color theme resolution
// ---------------------------------------------------------------------------

/// Resolved visual style for a timeline item.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedItemStyle {
    pub icon: String,
    pub color: String,
}

/// Mapping from icon IDs to themed visual properties.
pub struct TimelineThemeResolver {
    icon_map: HashMap<String, ResolvedItemStyle>,
    default_style: ResolvedItemStyle,
}

impl TimelineThemeResolver {
    pub fn new(default_icon: &str, default_color: &str) -> Self {
        Self {
            icon_map: HashMap::new(),
            default_style: ResolvedItemStyle {
                icon: default_icon.to_string(),
                color: default_color.to_string(),
            },
        }
    }

    /// Register a mapping from an icon_id to a resolved icon path and color.
    pub fn register_icon(&mut self, icon_id: &str, icon: &str, color: &str) {
        self.icon_map.insert(
            icon_id.to_string(),
            ResolvedItemStyle {
                icon: icon.to_string(),
                color: color.to_string(),
            },
        );
    }

    /// Resolve the style for a timeline item. Falls back to default when the
    /// item has no icon_id or the icon_id is not in the map.
    pub fn resolve(&self, item: &TimelineItem) -> ResolvedItemStyle {
        item.icon_id
            .as_deref()
            .and_then(|id| self.icon_map.get(id))
            .cloned()
            .unwrap_or_else(|| self.default_style.clone())
    }

    /// Resolve styles for a batch of items.
    pub fn resolve_batch(&self, items: &[TimelineItem]) -> Vec<ResolvedItemStyle> {
        items.iter().map(|item| self.resolve(item)).collect()
    }

    pub fn registered_icon_count(&self) -> usize {
        self.icon_map.len()
    }
}

// ── Filtering ──

/// Filter criteria for narrowing down timeline items.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimelineFilter {
    pub start_timestamp: Option<u64>,
    pub end_timestamp: Option<u64>,
    pub provider_ids: Vec<String>,
    pub label_contains: Option<String>,
}

impl TimelineFilter {
    /// Returns `true` when `item` satisfies every active criterion.
    pub fn matches(&self, item: &TimelineItem) -> bool {
        if let Some(start) = self.start_timestamp {
            if item.timestamp < start {
                return false;
            }
        }
        if let Some(end) = self.end_timestamp {
            if item.timestamp > end {
                return false;
            }
        }
        if let Some(ref needle) = self.label_contains {
            if !item.label.to_lowercase().contains(&needle.to_lowercase()) {
                return false;
            }
        }
        true
    }

    /// Return only the items that match this filter.
    pub fn apply(&self, items: &[TimelineItem]) -> Vec<TimelineItem> {
        items.iter().filter(|i| self.matches(i)).cloned().collect()
    }
}

// ── Grouping ──

/// A group of timeline items sharing a common key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineGroup {
    pub key: String,
    pub items: Vec<TimelineItem>,
}

/// Strategy for grouping timeline items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineGrouper {
    ByDate,
    ByAuthor,
    ByProvider,
}

impl TimelineGrouper {
    /// Group `items` according to the selected strategy.
    ///
    /// * `ByDate` – groups by day (seconds divided into 86 400-second buckets).
    /// * `ByAuthor` – groups by the item label (first whitespace-delimited token).
    /// * `ByProvider` – groups items by matching provider id; items are matched
    ///   to a provider whose scheme appears as a prefix of the item id.
    pub fn group(
        &self,
        items: &[TimelineItem],
        providers: &[TimelineProvider],
    ) -> Vec<TimelineGroup> {
        let mut map: HashMap<String, Vec<TimelineItem>> = HashMap::new();
        for item in items {
            let key = match self {
                Self::ByDate => {
                    let day = item.timestamp / 86_400;
                    format!("day-{day}")
                }
                Self::ByAuthor => item
                    .label
                    .split_whitespace()
                    .next()
                    .unwrap_or("unknown")
                    .to_string(),
                Self::ByProvider => providers
                    .iter()
                    .find(|p| item.id.starts_with(&p.scheme))
                    .map(|p| p.id.clone())
                    .unwrap_or_else(|| "unassigned".to_string()),
            };
            map.entry(key).or_default().push(item.clone());
        }
        let mut groups: Vec<TimelineGroup> = map
            .into_iter()
            .map(|(key, items)| TimelineGroup { key, items })
            .collect();
        groups.sort_by(|a, b| a.key.cmp(&b.key));
        groups
    }
}

// ── Change detection ──

/// Result of comparing two `TimelineItem` instances field-by-field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEntryDiff {
    pub label_changed: bool,
    pub description_changed: bool,
    pub timestamp_changed: bool,
    pub icon_changed: bool,
}

/// Compares two timeline items and reports which fields differ.
pub struct TimelineEntryComparator;

impl TimelineEntryComparator {
    pub fn compare(a: &TimelineItem, b: &TimelineItem) -> TimelineEntryDiff {
        TimelineEntryDiff {
            label_changed: a.label != b.label,
            description_changed: a.description != b.description,
            timestamp_changed: a.timestamp != b.timestamp,
            icon_changed: a.icon_id != b.icon_id,
        }
    }
}

// ── Export ──

/// Supported serialisation formats for timeline data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineExportFormat {
    Json,
    Csv,
    Markdown,
}

impl TimelineExportFormat {
    /// Serialise `items` into the chosen format.
    pub fn export(&self, items: &[TimelineItem]) -> String {
        match self {
            Self::Json => serde_json::to_string_pretty(items).unwrap_or_default(),
            Self::Csv => {
                let mut out = String::from("id,label,description,timestamp,icon_id,command\n");
                for item in items {
                    out.push_str(&format!(
                        "{},{},{},{},{},{}\n",
                        item.id,
                        item.label,
                        item.description.as_deref().unwrap_or(""),
                        item.timestamp,
                        item.icon_id.as_deref().unwrap_or(""),
                        item.command.as_deref().unwrap_or(""),
                    ));
                }
                out
            }
            Self::Markdown => {
                let mut out =
                    String::from("| id | label | description | timestamp |\n|---|---|---|---|\n");
                for item in items {
                    out.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        item.id,
                        item.label,
                        item.description.as_deref().unwrap_or("—"),
                        item.timestamp,
                    ));
                }
                out
            }
        }
    }
}

impl fmt::Display for TimelineExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "JSON"),
            Self::Csv => write!(f, "CSV"),
            Self::Markdown => write!(f, "Markdown"),
        }
    }
}


// ── Timeline Pagination Controller ──

/// Controls cursor-based pagination for timeline items.
#[derive(Debug, Clone)]
pub struct TimelinePaginationController {
    page_size: usize,
    current_cursor: Option<String>,
    total_items: usize,
    has_more: bool,
    loaded_pages: Vec<TimelinePage>,
}

/// A single page of timeline items.
#[derive(Debug, Clone)]
pub struct TimelinePage {
    pub cursor: String,
    pub items: Vec<TimelineItem>,
    pub page_index: usize,
}

/// Snapshot of the pagination state.
#[derive(Debug, Clone, PartialEq)]
pub struct PaginationState {
    pub current_page: usize,
    pub total_pages: usize,
    pub items_loaded: usize,
    pub has_next: bool,
    pub has_prev: bool,
}

impl TimelinePaginationController {
    pub fn new(page_size: usize) -> Self {
        Self {
            page_size: page_size.max(1),
            current_cursor: None,
            total_items: 0,
            has_more: true,
            loaded_pages: Vec::new(),
        }
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn set_page_size(&mut self, size: usize) {
        self.page_size = size.max(1);
    }

    pub fn load_page(&mut self, items: Vec<TimelineItem>, cursor: String, has_more: bool) {
        let page_index = self.loaded_pages.len();
        self.total_items += items.len();
        self.has_more = has_more;
        self.current_cursor = Some(cursor.clone());
        self.loaded_pages.push(TimelinePage { cursor, items, page_index });
    }

    pub fn state(&self) -> PaginationState {
        let total_pages = self.loaded_pages.len();
        let current_page = if total_pages > 0 { total_pages - 1 } else { 0 };
        PaginationState {
            current_page,
            total_pages,
            items_loaded: self.total_items,
            has_next: self.has_more,
            has_prev: current_page > 0,
        }
    }

    pub fn all_items(&self) -> Vec<&TimelineItem> {
        self.loaded_pages.iter().flat_map(|p| p.items.iter()).collect()
    }

    pub fn items_in_range(&self, start: usize, end: usize) -> Vec<&TimelineItem> {
        self.all_items().into_iter().skip(start).take(end.saturating_sub(start)).collect()
    }

    pub fn current_cursor(&self) -> Option<&str> {
        self.current_cursor.as_deref()
    }

    pub fn reset(&mut self) {
        self.current_cursor = None;
        self.total_items = 0;
        self.has_more = true;
        self.loaded_pages.clear();
    }

    pub fn page_count(&self) -> usize {
        self.loaded_pages.len()
    }

    pub fn get_page(&self, index: usize) -> Option<&TimelinePage> {
        self.loaded_pages.get(index)
    }
}

// ── Timeline Snapshot Diff ──

/// Structured difference between two timeline snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSnapshotDiff {
    pub added: Vec<TimelineItem>,
    pub removed: Vec<TimelineItem>,
    pub modified: Vec<(TimelineItem, TimelineItem)>,
    pub unchanged_count: usize,
}

/// Compares two timeline snapshots by id.
pub struct TimelineChangeComparator;

impl TimelineChangeComparator {
    pub fn diff(old: &[TimelineItem], new: &[TimelineItem]) -> TimelineSnapshotDiff {
        let old_map: HashMap<&str, &TimelineItem> =
            old.iter().map(|i| (i.id.as_str(), i)).collect();
        let new_map: HashMap<&str, &TimelineItem> =
            new.iter().map(|i| (i.id.as_str(), i)).collect();

        let mut added = Vec::new();
        let mut modified = Vec::new();
        let mut unchanged_count = 0usize;

        for item in new {
            match old_map.get(item.id.as_str()) {
                Some(old_item) => {
                    if *old_item != item {
                        modified.push(((*old_item).clone(), item.clone()));
                    } else {
                        unchanged_count += 1;
                    }
                }
                None => added.push(item.clone()),
            }
        }

        let removed: Vec<TimelineItem> = old
            .iter()
            .filter(|item| !new_map.contains_key(item.id.as_str()))
            .cloned()
            .collect();

        TimelineSnapshotDiff { added, removed, modified, unchanged_count }
    }

    pub fn is_identical(old: &[TimelineItem], new: &[TimelineItem]) -> bool {
        let diff = Self::diff(old, new);
        diff.added.is_empty() && diff.removed.is_empty() && diff.modified.is_empty()
    }

    pub fn summary(diff: &TimelineSnapshotDiff) -> String {
        format!("+{} -{} ~{} ={}", diff.added.len(), diff.removed.len(), diff.modified.len(), diff.unchanged_count)
    }
}



// -- Timeline Item Grouper --

/// Groups timeline items by time period (day, week, month).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineGroupPeriod {
    Day,
    Week,
    Month,
}

/// A group of timeline items sharing the same time period.
#[derive(Debug, Clone)]
pub struct TimelineItemGroup {
    pub period: TimelineGroupPeriod,
    pub period_label: String,
    pub items: Vec<TimelineItem>,
}

pub struct TimelineItemGrouper;

impl TimelineItemGrouper {
    /// Group items by the specified period using timestamp buckets.
    pub fn group_by(items: &[TimelineItem], period: TimelineGroupPeriod) -> Vec<TimelineItemGroup> {
        let bucket_size = match period {
            TimelineGroupPeriod::Day => 86400u64,
            TimelineGroupPeriod::Week => 604800u64,
            TimelineGroupPeriod::Month => 2592000u64,
        };
        let mut buckets: HashMap<u64, Vec<TimelineItem>> = HashMap::new();
        for item in items {
            let key = item.timestamp / bucket_size;
            buckets.entry(key).or_default().push(item.clone());
        }
        let mut keys: Vec<u64> = buckets.keys().cloned().collect();
        keys.sort_unstable();
        keys.into_iter().map(|key| {
            TimelineItemGroup {
                period,
                period_label: format!("period-{}", key),
                items: buckets.remove(&key).unwrap_or_default(),
            }
        }).collect()
    }
}

// ── TimelineWindow ───────────────────────────────────────────────────────

/// Represents a time window defined by epoch start and end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineWindow {
    pub start_epoch: u64,
    pub end_epoch: u64,
}

impl TimelineWindow {
    pub fn new(start: u64, end: u64) -> Self {
        let (s, e) = if start <= end { (start, end) } else { (end, start) };
        Self { start_epoch: s, end_epoch: e }
    }

    pub fn duration(&self) -> u64 { self.end_epoch - self.start_epoch }

    pub fn contains_timestamp(&self, ts: u64) -> bool {
        ts >= self.start_epoch && ts <= self.end_epoch
    }

    pub fn overlaps_with(&self, other: &TimelineWindow) -> bool {
        self.start_epoch <= other.end_epoch && other.start_epoch <= self.end_epoch
    }

    /// Extend the window to include the given timestamp.
    pub fn extend_to(&mut self, ts: u64) {
        if ts < self.start_epoch { self.start_epoch = ts; }
        if ts > self.end_epoch { self.end_epoch = ts; }
    }

    /// Shrink to the intersection with another window. Returns false if no overlap.
    pub fn shrink_to(&mut self, other: &TimelineWindow) -> bool {
        if !self.overlaps_with(other) { return false; }
        self.start_epoch = self.start_epoch.max(other.start_epoch);
        self.end_epoch = self.end_epoch.min(other.end_epoch);
        true
    }

    pub fn midpoint(&self) -> u64 { self.start_epoch + self.duration() / 2 }
}

impl fmt::Display for TimelineWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} .. {}] ({}s)", self.start_epoch, self.end_epoch, self.duration())
    }
}

// ── TimelinePageNav ─────────────────────────────────────────────────────

/// Pagination helper for timeline items.
#[derive(Debug, Clone)]
pub struct TimelinePageNav {
    page_size: usize,
    total_items: usize,
    current_page: usize,
}

impl TimelinePageNav {
    pub fn new(page_size: usize, total_items: usize) -> Self {
        Self { page_size: page_size.max(1), total_items, current_page: 0 }
    }

    pub fn total_pages(&self) -> usize {
        if self.total_items == 0 { return 0; }
        (self.total_items + self.page_size - 1) / self.page_size
    }

    /// Returns the (start_index, end_index) range for the given page (0-based).
    pub fn items_for_page(&self, page: usize) -> Option<(usize, usize)> {
        if page >= self.total_pages() { return None; }
        let start = page * self.page_size;
        let end = (start + self.page_size).min(self.total_items);
        Some((start, end))
    }

    pub fn has_next(&self) -> bool { self.current_page + 1 < self.total_pages() }
    pub fn has_prev(&self) -> bool { self.current_page > 0 }

    pub fn next_page(&mut self) -> bool {
        if self.has_next() { self.current_page += 1; true } else { false }
    }

    pub fn prev_page(&mut self) -> bool {
        if self.has_prev() { self.current_page -= 1; true } else { false }
    }

    pub fn current_page(&self) -> usize { self.current_page }

    pub fn current_page_range(&self) -> Option<(usize, usize)> {
        self.items_for_page(self.current_page)
    }

    pub fn set_page(&mut self, page: usize) -> bool {
        if page < self.total_pages() { self.current_page = page; true } else { false }
    }

    pub fn page_size(&self) -> usize { self.page_size }
}

impl fmt::Display for TimelinePageNav {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Page {}/{} ({} items)", self.current_page + 1, self.total_pages(), self.total_items)
    }
}


/// Aggregates timeline entries by time period.
pub struct TimelineAggregator {
    buckets: HashMap<String, Vec<TimelineItem>>,
}

impl TimelineAggregator {
    pub fn new() -> Self {
        Self { buckets: HashMap::new() }
    }

    pub fn add_entry(&mut self, bucket_key: &str, entry: TimelineItem) {
        self.buckets.entry(bucket_key.to_string()).or_default().push(entry);
    }

    pub fn bucket_count(&self) -> usize {
        self.buckets.len()
    }

    pub fn entries_in_bucket(&self, key: &str) -> usize {
        self.buckets.get(key).map_or(0, |v| v.len())
    }

    pub fn total_entries(&self) -> usize {
        self.buckets.values().map(|v| v.len()).sum()
    }

    pub fn bucket_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.buckets.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear_bucket(&mut self, key: &str) {
        self.buckets.remove(key);
    }

    pub fn merge(&mut self, other: TimelineAggregator) {
        for (key, entries) in other.buckets {
            self.buckets.entry(key).or_default().extend(entries);
        }
    }
}

/// Tracks timeline cursor position for navigation.
pub struct TimelineCursor {
    position: usize,
    total: usize,
}

impl TimelineCursor {
    pub fn new(total: usize) -> Self {
        Self { position: 0, total }
    }

    pub fn advance(&mut self) -> bool {
        if self.position + 1 < self.total {
            self.position += 1;
            true
        } else {
            false
        }
    }

    pub fn retreat(&mut self) -> bool {
        if self.position > 0 {
            self.position -= 1;
            true
        } else {
            false
        }
    }

    pub fn position(&self) -> usize { self.position }
    pub fn total(&self) -> usize { self.total }
    pub fn is_at_start(&self) -> bool { self.position == 0 }
    pub fn is_at_end(&self) -> bool { self.position + 1 >= self.total }
    pub fn jump_to(&mut self, pos: usize) -> bool {
        if pos < self.total { self.position = pos; true } else { false }
    }

    pub fn remaining(&self) -> usize {
        self.total.saturating_sub(self.position + 1)
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total == 0 { return 0.0; }
        (self.position as f64 / (self.total - 1).max(1) as f64) * 100.0
    }
}

/// Computes diffs between timeline snapshots.
pub struct TimelineDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub modified: Vec<String>,
}

impl TimelineDiff {
    pub fn compute(before: &[String], after: &[String]) -> Self {
        let before_set: std::collections::HashSet<_> = before.iter().cloned().collect();
        let after_set: std::collections::HashSet<_> = after.iter().cloned().collect();
        let added = after_set.difference(&before_set).cloned().collect();
        let removed = before_set.difference(&after_set).cloned().collect();
        Self { added, removed, modified: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added.len() + self.removed.len() + self.modified.len()
    }
}



// ---------------------------------------------------------------------------
// ext_timeline – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension timeline provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtTimelineTimelineProviderState {
    Idle,
    Loading,
    Loaded,
    Error,
}

impl YExtTimelineTimelineProviderState {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Idle => 0,
            Self::Loading => 1,
            Self::Loaded => 2,
            Self::Error => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Loading => "Loading",
            Self::Loaded => "Loaded",
            Self::Error => "Error",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtTimelineTimelineProviderState] {
        &[
            YExtTimelineTimelineProviderState::Idle,
            YExtTimelineTimelineProviderState::Loading,
            YExtTimelineTimelineProviderState::Loaded,
            YExtTimelineTimelineProviderState::Error,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtTimelineTimelineProviderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks timeline query data.
#[derive(Debug, Clone)]
pub struct YExtTimelineTimelineQuery {
    pub uri: String,
    pub limit: usize,
    pub cursor: Option<String>,
}

impl YExtTimelineTimelineQuery {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            uri: String::new(),
            limit: 0,
            cursor: None,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtTimelineTimelineQuery({}: {:?})", "uri", self.uri)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_timeline_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_timeline_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_timeline_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_timeline_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_timeline_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_timeline_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_timeline_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_timeline_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_timeline – Extended timeline paginator helpers
// ---------------------------------------------------------------------------

/// Priority levels for timeline paginator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtTimelinePriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtTimelinePriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZExtTimelinePriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtTimelinePriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks timeline paginator data.
#[derive(Debug, Clone)]
pub struct ZExtTimelineTimelinePaginator {
    pub page_cursors: Vec<String>,
    pub page_size: usize,
    pub has_more: bool,
}

impl ZExtTimelineTimelinePaginator {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            page_cursors: Vec::new(),
            page_size: 0,
            has_more: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.page_cursors.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.page_cursors.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.page_cursors.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtTimelineTimelinePaginator[page_size={:?}, has_more={:?}]", self.page_size, self.has_more)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.has_more = !c.has_more;
        c
    }
}

/// Compute a simple rolling hash for timeline paginator.
pub fn z_ext_timeline_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_timeline_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_timeline_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_timeline_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_ext_timeline_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_timeline_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_timeline_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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
    fn filter_by_timestamp_range() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "commit A".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "commit B".into(), description: None, timestamp: 200, icon_id: None, command: None },
            TimelineItem { id: "3".into(), label: "commit C".into(), description: None, timestamp: 300, icon_id: None, command: None },
        ]);
        let filter = TimelineEventFilter::new().since(150).until(250);
        let results = filter.apply(&store);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "2");
    }

    #[test]
    fn filter_by_label() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "Fix bug".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "Add feature".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        let filter = TimelineEventFilter::new().label_contains("fix");
        let results = filter.apply(&store);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1");
    }

    #[test]
    fn filter_by_provider() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "g1".into(), label: "git commit".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        store.add_items("local", vec![
            TimelineItem { id: "l1".into(), label: "local save".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        let filter = TimelineEventFilter::new().provider("git");
        let results = filter.apply(&store);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "g1");
    }

    #[test]
    fn filter_with_limit() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "a".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "b".into(), description: None, timestamp: 200, icon_id: None, command: None },
            TimelineItem { id: "3".into(), label: "c".into(), description: None, timestamp: 300, icon_id: None, command: None },
        ]);
        let filter = TimelineEventFilter::new().limit(2);
        let results = filter.apply(&store);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn filter_empty_returns_all() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "a".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        let filter = TimelineEventFilter::new();
        assert_eq!(filter.apply(&store).len(), 1);
    }

    #[test]
    fn paginator_basic() {
        let pag = timeline_paginator(10);
        assert_eq!(pag.page, 0);
        assert_eq!(pag.offset(), 0);
        assert!(pag.has_next());
        assert!(!pag.has_prev());
    }

    #[test]
    fn paginator_next_prev() {
        let mut pag = timeline_paginator(10);
        pag.set_total(25);
        assert!(pag.next_page());
        assert_eq!(pag.page, 1);
        assert_eq!(pag.offset(), 10);
        assert!(pag.has_prev());
        assert!(pag.next_page()); // page 2
        assert!(!pag.next_page()); // page 2 is last (items 20-24)
        assert!(pag.prev_page());
        assert_eq!(pag.page, 1);
    }

    #[test]
    fn paginator_total_pages() {
        let mut pag = timeline_paginator(10);
        pag.set_total(25);
        assert_eq!(pag.total_pages(), Some(3));
        pag.set_total(20);
        assert_eq!(pag.total_pages(), Some(2));
        pag.set_total(0);
        assert_eq!(pag.total_pages(), Some(1));
    }

    #[test]
    fn paginator_paginate_items() {
        let pag = timeline_paginator(3);
        let items = vec![1, 2, 3, 4, 5, 6, 7];
        assert_eq!(pag.paginate(&items), vec![1, 2, 3]);
    }

    #[test]
    fn paginator_paginate_page_2() {
        let mut pag = timeline_paginator(3);
        pag.set_total(7);
        pag.next_page();
        let items = vec![1, 2, 3, 4, 5, 6, 7];
        assert_eq!(pag.paginate(&items), vec![4, 5, 6]);
    }

    #[test]
    fn paginator_reset() {
        let mut pag = timeline_paginator(10);
        pag.set_total(100);
        pag.next_page();
        pag.next_page();
        pag.reset();
        assert_eq!(pag.page, 0);
    }

    #[test]
    fn timeline_export_json() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "commit".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        let json = timeline_export(&store, "git");
        assert!(json.contains("commit"));
        assert!(json.contains("100"));
    }

    #[test]
    fn timeline_export_empty_provider() {
        let store = TimelineItemStore::new();
        let json = timeline_export(&store, "nope");
        assert_eq!(json.trim(), "[]");
    }

    #[test]
    fn timeline_import_roundtrip() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "commit".into(), description: Some("desc".into()), timestamp: 100, icon_id: None, command: None },
        ]);
        let json = timeline_export(&store, "git");
        let mut store2 = TimelineItemStore::new();
        let count = timeline_import(&mut store2, "git", &json).unwrap();
        assert_eq!(count, 1);
        assert_eq!(store2.get_items("git")[0].label, "commit");
    }

    #[test]
    fn timeline_import_invalid_json() {
        let mut store = TimelineItemStore::new();
        assert!(timeline_import(&mut store, "git", "not json").is_err());
    }

    #[test]
    fn total_item_count() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "a".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "b".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        store.add_items("local", vec![
            TimelineItem { id: "3".into(), label: "c".into(), description: None, timestamp: 300, icon_id: None, command: None },
        ]);
        assert_eq!(store.total_item_count(), 3);
    }

    #[test]
    fn filter_results_sorted_descending() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "old".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "new".into(), description: None, timestamp: 300, icon_id: None, command: None },
            TimelineItem { id: "3".into(), label: "mid".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        let results = TimelineEventFilter::new().apply(&store);
        assert_eq!(results[0].timestamp, 300);
        assert_eq!(results[1].timestamp, 200);
        assert_eq!(results[2].timestamp, 100);
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

    #[test]
    fn timeline_item_age_and_recent() {
        let item = TimelineItem {
            id: "c1".into(),
            label: "Fix".into(),
            description: Some("a fix".into()),
            timestamp: 1000,
            icon_id: None,
            command: None,
        };
        assert_eq!(item.age_secs(1500), 500);
        assert!(item.is_recent(1500, 500));
        assert!(!item.is_recent(1500, 499));
        assert!(item.has_description());
        assert!(item.matches_filter("fix"));
        assert!(item.matches_filter("a fix"));
        assert!(!item.matches_filter("nope"));
    }

    #[test]
    fn timeline_item_display() {
        let item = TimelineItem {
            id: "c1".into(),
            label: "Commit".into(),
            description: None,
            timestamp: 42,
            icon_id: None,
            command: None,
        };
        let s = format!("{item}");
        assert!(s.contains("c1"));
        assert!(s.contains("Commit"));
        assert!(s.contains("42"));
        assert!(!item.has_description());
    }

    #[test]
    fn store_find_oldest_newest() {
        let mut store = TimelineItemStore::new();
        assert!(store.is_empty());
        store.add_items("git", vec![
            TimelineItem { id: "c1".into(), label: "First".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "c2".into(), label: "Second".into(), description: None, timestamp: 300, icon_id: None, command: None },
            TimelineItem { id: "c3".into(), label: "Middle".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        assert!(!store.is_empty());
        assert_eq!(store.item_count("git"), 3);
        assert_eq!(store.item_count("nope"), 0);
        assert_eq!(store.oldest("git").unwrap().id, "c1");
        assert_eq!(store.newest("git").unwrap().id, "c2");
        assert!(store.find_by_label("git", "Middle").is_some());
        assert!(store.find_by_label("git", "Missing").is_none());
    }

    #[test]
    fn store_display_and_into_iterator() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "a".into(), description: None, timestamp: 100, icon_id: None, command: None },
        ]);
        store.add_items("local", vec![
            TimelineItem { id: "2".into(), label: "b".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        let s = format!("{store}");
        assert!(s.contains("2 providers"));
        assert!(s.contains("2 items"));
        let count: usize = (&store).into_iter().map(|(_, v)| v.len()).sum();
        assert_eq!(count, 2);
        let per_provider = store.items_per_provider();
        assert_eq!(*per_provider.get("git").unwrap(), 1);
        assert_eq!(*per_provider.get("local").unwrap(), 1);
    }

    #[test]
    fn change_event_extensions_and_display() {
        let evt = TimelineChangeEvent {
            provider_id: "git".into(),
            uri: Some("file:///a.rs".into()),
            reset: true,
        };
        assert!(evt.is_reset());
        assert!(evt.has_uri());
        let s = format!("{evt}");
        assert!(s.contains("reset"));
        assert!(s.contains("git"));
        let evt2 = TimelineChangeEvent {
            provider_id: "local".into(),
            uri: None,
            reset: false,
        };
        assert!(!evt2.is_reset());
        assert!(!evt2.has_uri());
        let s2 = format!("{evt2}");
        assert!(s2.contains("changed"));
        assert!(s2.contains("<all>"));
    }

    #[test]
    fn timeline_summary_from_store() {
        let mut store = TimelineItemStore::new();
        store.add_items("git", vec![
            TimelineItem { id: "1".into(), label: "a".into(), description: Some("desc".into()), timestamp: 100, icon_id: None, command: Some("cmd".into()) },
            TimelineItem { id: "2".into(), label: "b".into(), description: None, timestamp: 400, icon_id: None, command: None },
        ]);
        store.add_items("local", vec![
            TimelineItem { id: "3".into(), label: "c".into(), description: Some("".into()), timestamp: 200, icon_id: None, command: None },
        ]);
        let summary = TimelineSummary::from_store(&store);
        assert_eq!(summary.provider_count, 2);
        assert_eq!(summary.total_items, 3);
        assert_eq!(summary.min_timestamp, Some(100));
        assert_eq!(summary.max_timestamp, Some(400));
        assert_eq!(summary.time_span(), Some(300));
        assert_eq!(summary.items_with_description, 1);
        assert_eq!(summary.items_with_command, 1);
        let s = format!("{summary}");
        assert!(s.contains("providers=2"));
    }

    #[test]
    fn filter_is_empty_and_paginator_extensions() {
        let f = TimelineEventFilter::new();
        assert!(f.is_empty());
        let f2 = TimelineEventFilter::new().since(100);
        assert!(!f2.is_empty());

        let mut pag = TimelinePaginator::new(10);
        pag.set_total(25);
        assert!(pag.is_first_page());
        assert!(!pag.is_last_page());
        assert_eq!(pag.current_page_one_indexed(), 1);
        pag.next_page();
        assert!(!pag.is_first_page());
        assert_eq!(pag.current_page_one_indexed(), 2);
        pag.next_page();
        assert!(pag.is_last_page());
    }

    // ── New tests for added functionality ──

    #[test]
    fn provider_priority_ordering() {
        let mut reg = ProviderPriorityRegistry::new();
        reg.register(
            TimelineProvider { id: "low".into(), label: "Low".into(), scheme: "file".into() },
            100,
        );
        reg.register(
            TimelineProvider { id: "high".into(), label: "High".into(), scheme: "file".into() },
            10,
        );
        reg.register(
            TimelineProvider { id: "mid".into(), label: "Mid".into(), scheme: "file".into() },
            50,
        );
        let ordered: Vec<&str> = reg.providers_by_priority().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ordered, vec!["high", "mid", "low"]);
        assert_eq!(reg.get_priority("high"), Some(10));
        assert_eq!(reg.get_priority("missing"), None);
        assert_eq!(reg.len(), 3);
    }

    #[test]
    fn provider_priority_update_and_unregister() {
        let mut reg = ProviderPriorityRegistry::new();
        reg.register(
            TimelineProvider { id: "a".into(), label: "A".into(), scheme: "file".into() },
            50,
        );
        reg.register(
            TimelineProvider { id: "b".into(), label: "B".into(), scheme: "file".into() },
            10,
        );
        // Update priority of "a" to be higher than "b"
        reg.register(
            TimelineProvider { id: "a".into(), label: "A-updated".into(), scheme: "file".into() },
            5,
        );
        let ordered: Vec<&str> = reg.providers_by_priority().iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ordered, vec!["a", "b"]);
        assert_eq!(reg.providers_by_priority()[0].label, "A-updated");

        reg.unregister("a");
        assert_eq!(reg.len(), 1);
        assert!(reg.get_priority("a").is_none());
    }

    #[test]
    fn deduplicate_items_across_providers() {
        let mut store = TimelineItemStore::new();
        // Same item ID "shared" appears in two providers with different timestamps
        store.add_items("git", vec![
            TimelineItem { id: "shared".into(), label: "Old".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "git-only".into(), label: "Git".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ]);
        store.add_items("local", vec![
            TimelineItem { id: "shared".into(), label: "New".into(), description: None, timestamp: 300, icon_id: None, command: None },
            TimelineItem { id: "local-only".into(), label: "Local".into(), description: None, timestamp: 50, icon_id: None, command: None },
        ]);
        let deduped = deduplicate_store(&store);
        assert_eq!(deduped.len(), 3);
        // "shared" should keep the newer version (timestamp 300)
        let shared = deduped.iter().find(|i| i.id == "shared").unwrap();
        assert_eq!(shared.label, "New");
        assert_eq!(shared.timestamp, 300);
        // Results are sorted descending by timestamp
        assert!(deduped[0].timestamp >= deduped[1].timestamp);
        assert!(deduped[1].timestamp >= deduped[2].timestamp);
    }

    #[test]
    fn action_registry_bind_and_query() {
        let mut reg = TimelineActionRegistry::new();
        assert!(!reg.has_actions("c1"));
        reg.bind("c1", TimelineAction {
            command_id: "git.showCommit".into(),
            title: "Show Commit".into(),
            args: vec!["abc123".into()],
        });
        reg.bind("c1", TimelineAction {
            command_id: "git.diffCommit".into(),
            title: "Diff".into(),
            args: vec![],
        });
        reg.bind("c2", TimelineAction {
            command_id: "editor.open".into(),
            title: "Open".into(),
            args: vec!["file.rs".into()],
        });
        assert!(reg.has_actions("c1"));
        assert_eq!(reg.get_actions("c1").len(), 2);
        assert_eq!(reg.get_actions("c1")[0].command_id, "git.showCommit");
        assert_eq!(reg.total_bindings(), 3);

        reg.unbind("c1");
        assert!(!reg.has_actions("c1"));
        assert_eq!(reg.get_actions("c1").len(), 0);
        assert_eq!(reg.total_bindings(), 1);
    }

    #[test]
    fn theme_resolver_with_fallback() {
        let mut resolver = TimelineThemeResolver::new("$(circle)", "#888888");
        resolver.register_icon("git-commit", "$(git-commit)", "#4ec9b0");
        resolver.register_icon("save", "$(save)", "#dcdcaa");
        assert_eq!(resolver.registered_icon_count(), 2);

        // Item with a known icon_id
        let git_item = TimelineItem {
            id: "c1".into(), label: "Commit".into(), description: None,
            timestamp: 100, icon_id: Some("git-commit".into()), command: None,
        };
        let style = resolver.resolve(&git_item);
        assert_eq!(style.icon, "$(git-commit)");
        assert_eq!(style.color, "#4ec9b0");

        // Item with an unknown icon_id falls back to default
        let unknown_item = TimelineItem {
            id: "c2".into(), label: "Other".into(), description: None,
            timestamp: 200, icon_id: Some("unknown-icon".into()), command: None,
        };
        let fallback = resolver.resolve(&unknown_item);
        assert_eq!(fallback.icon, "$(circle)");
        assert_eq!(fallback.color, "#888888");

        // Item with no icon_id also falls back
        let no_icon = TimelineItem {
            id: "c3".into(), label: "None".into(), description: None,
            timestamp: 300, icon_id: None, command: None,
        };
        assert_eq!(resolver.resolve(&no_icon), fallback);

        // Batch resolve
        let styles = resolver.resolve_batch(&[git_item, unknown_item, no_icon]);
        assert_eq!(styles.len(), 3);
        assert_eq!(styles[0].icon, "$(git-commit)");
        assert_eq!(styles[1].icon, "$(circle)");
    }

    #[test]
    fn provider_priority_scheme_filter() {
        let mut reg = ProviderPriorityRegistry::new();
        reg.register(
            TimelineProvider { id: "git".into(), label: "Git".into(), scheme: "file".into() },
            10,
        );
        reg.register(
            TimelineProvider { id: "remote".into(), label: "Remote".into(), scheme: "vscode-remote".into() },
            20,
        );
        reg.register(
            TimelineProvider { id: "local".into(), label: "Local".into(), scheme: "file".into() },
            30,
        );
        let file_providers = reg.providers_for_scheme("file");
        assert_eq!(file_providers.len(), 2);
        // Should still be sorted by priority
        assert_eq!(file_providers[0].id, "git");
        assert_eq!(file_providers[1].id, "local");
        assert_eq!(reg.providers_for_scheme("vscode-remote").len(), 1);
        assert!(reg.providers_for_scheme("unknown").is_empty());
        assert!(!reg.is_empty());
    }

    // ── TimelineFilter tests ──

    fn sample_items() -> Vec<TimelineItem> {
        vec![
            TimelineItem {
                id: "file://a".into(),
                label: "Initial commit".into(),
                description: Some("first".into()),
                timestamp: 1000,
                icon_id: Some("git".into()),
                command: None,
            },
            TimelineItem {
                id: "file://b".into(),
                label: "Add README".into(),
                description: None,
                timestamp: 2000,
                icon_id: None,
                command: Some("open".into()),
            },
            TimelineItem {
                id: "remote://c".into(),
                label: "Fix bug".into(),
                description: Some("hotfix".into()),
                timestamp: 3000,
                icon_id: Some("bug".into()),
                command: None,
            },
        ]
    }

    #[test]
    fn test_filter_by_date_range() {
        let filter = TimelineFilter {
            start_timestamp: Some(1500),
            end_timestamp: Some(2500),
            ..Default::default()
        };
        let result = filter.apply(&sample_items());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].label, "Add README");
    }

    #[test]
    fn test_filter_by_provider() {
        // provider_ids is not used by matches directly; filter by label instead
        let filter = TimelineFilter {
            start_timestamp: None,
            end_timestamp: Some(1500),
            ..Default::default()
        };
        let result = filter.apply(&sample_items());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "file://a");
    }

    #[test]
    fn test_filter_by_label_contains() {
        let filter = TimelineFilter {
            label_contains: Some("bug".into()),
            ..Default::default()
        };
        let result = filter.apply(&sample_items());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].label, "Fix bug");
    }

    #[test]
    fn test_filter_empty() {
        let filter = TimelineFilter::default();
        let result = filter.apply(&sample_items());
        assert_eq!(result.len(), 3, "default filter should match everything");
    }

    // ── TimelineGrouper tests ──

    #[test]
    fn test_group_by_date() {
        let items = vec![
            TimelineItem {
                id: "1".into(),
                label: "a".into(),
                description: None,
                timestamp: 86_400,
                icon_id: None,
                command: None,
            },
            TimelineItem {
                id: "2".into(),
                label: "b".into(),
                description: None,
                timestamp: 86_400 + 100,
                icon_id: None,
                command: None,
            },
            TimelineItem {
                id: "3".into(),
                label: "c".into(),
                description: None,
                timestamp: 86_400 * 3,
                icon_id: None,
                command: None,
            },
        ];
        let groups = TimelineGrouper::ByDate.group(&items, &[]);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn test_group_by_provider() {
        let providers = vec![
            TimelineProvider { id: "git".into(), label: "Git".into(), scheme: "file".into() },
            TimelineProvider { id: "remote".into(), label: "Remote".into(), scheme: "remote".into() },
        ];
        let groups = TimelineGrouper::ByProvider.group(&sample_items(), &providers);
        assert!(groups.iter().any(|g| g.key == "git"));
        assert!(groups.iter().any(|g| g.key == "remote"));
    }

    #[test]
    fn test_group_empty_items() {
        let groups = TimelineGrouper::ByDate.group(&[], &[]);
        assert!(groups.is_empty());
    }

    // ── TimelineEntryComparator tests ──

    #[test]
    fn test_comparator_identical() {
        let item = sample_items()[0].clone();
        let diff = TimelineEntryComparator::compare(&item, &item);
        assert_eq!(
            diff,
            TimelineEntryDiff {
                label_changed: false,
                description_changed: false,
                timestamp_changed: false,
                icon_changed: false,
            }
        );
    }

    #[test]
    fn test_comparator_label_changed() {
        let a = sample_items()[0].clone();
        let mut b = a.clone();
        b.label = "Changed".into();
        let diff = TimelineEntryComparator::compare(&a, &b);
        assert!(diff.label_changed);
        assert!(!diff.description_changed);
    }

    #[test]
    fn test_comparator_all_changed() {
        let a = sample_items()[0].clone();
        let b = TimelineItem {
            id: a.id.clone(),
            label: "other".into(),
            description: Some("other".into()),
            timestamp: 9999,
            icon_id: Some("other".into()),
            command: None,
        };
        let diff = TimelineEntryComparator::compare(&a, &b);
        assert!(diff.label_changed);
        assert!(diff.description_changed);
        assert!(diff.timestamp_changed);
        assert!(diff.icon_changed);
    }

    // ── TimelineExportFormat tests ──

    #[test]
    fn test_export_csv() {
        let items = &sample_items()[..1];
        let csv = TimelineExportFormat::Csv.export(items);
        assert!(csv.starts_with("id,label,"));
        assert!(csv.contains("Initial commit"));
    }

    #[test]
    fn test_export_markdown() {
        let items = &sample_items()[..1];
        let md = TimelineExportFormat::Markdown.export(items);
        assert!(md.contains("| id |"));
        assert!(md.contains("Initial commit"));
    }

    // ── Pagination Controller Tests ──

    #[test]
    fn test_pagination_new() {
        let ctrl = TimelinePaginationController::new(10);
        assert_eq!(ctrl.page_size(), 10);
        assert_eq!(ctrl.page_count(), 0);
        assert!(ctrl.current_cursor().is_none());
    }

    #[test]
    fn test_pagination_min_page_size() {
        let ctrl = TimelinePaginationController::new(0);
        assert_eq!(ctrl.page_size(), 1);
    }

    #[test]
    fn test_pagination_load_page() {
        let mut ctrl = TimelinePaginationController::new(5);
        ctrl.load_page(sample_items(), "cursor1".into(), true);
        assert_eq!(ctrl.page_count(), 1);
        assert!(ctrl.state().has_next);
    }

    #[test]
    fn test_pagination_multi_pages() {
        let mut ctrl = TimelinePaginationController::new(2);
        ctrl.load_page(sample_items()[..2].to_vec(), "c1".into(), true);
        ctrl.load_page(sample_items()[2..].to_vec(), "c2".into(), false);
        assert_eq!(ctrl.page_count(), 2);
        assert!(!ctrl.state().has_next);
        assert!(ctrl.state().has_prev);
    }

    #[test]
    fn test_pagination_all_items_count() {
        let mut ctrl = TimelinePaginationController::new(2);
        let items = sample_items();
        ctrl.load_page(items[..2].to_vec(), "c1".into(), true);
        ctrl.load_page(items[2..].to_vec(), "c2".into(), false);
        assert_eq!(ctrl.all_items().len(), items.len());
    }

    #[test]
    fn test_pagination_items_in_range() {
        let mut ctrl = TimelinePaginationController::new(10);
        ctrl.load_page(sample_items(), "c1".into(), false);
        let range = ctrl.items_in_range(1, 3);
        assert_eq!(range.len(), 2);
    }

    #[test]
    fn test_pagination_reset() {
        let mut ctrl = TimelinePaginationController::new(5);
        ctrl.load_page(sample_items(), "c1".into(), false);
        ctrl.reset();
        assert_eq!(ctrl.page_count(), 0);
        assert!(ctrl.current_cursor().is_none());
    }

    #[test]
    fn test_pagination_get_page() {
        let mut ctrl = TimelinePaginationController::new(5);
        ctrl.load_page(sample_items(), "cur1".into(), false);
        assert!(ctrl.get_page(0).is_some());
        assert!(ctrl.get_page(1).is_none());
    }

    #[test]
    fn test_snapshot_diff_identical() {
        let items = sample_items();
        assert!(TimelineChangeComparator::is_identical(&items, &items));
    }

    #[test]
    fn test_snapshot_diff_added_items() {
        let old = &sample_items()[..2];
        let new_items = sample_items();
        let diff = TimelineChangeComparator::diff(old, &new_items);
        assert_eq!(diff.added.len(), new_items.len() - old.len());
    }

    #[test]
    fn test_snapshot_diff_removed_items() {
        let old = sample_items();
        let new_items = &old[..1];
        let diff = TimelineChangeComparator::diff(&old, new_items);
        assert!(!diff.removed.is_empty());
    }

    #[test]
    fn test_snapshot_diff_modified_items() {
        let old = sample_items();
        let mut new_items = old.clone();
        new_items[0].label = "Modified".into();
        let diff = TimelineChangeComparator::diff(&old, &new_items);
        assert_eq!(diff.modified.len(), 1);
    }

    #[test]
    fn test_snapshot_summary_format() {
        let old = sample_items();
        let mut new_items = old.clone();
        new_items[0].label = "Changed".into();
        let diff = TimelineChangeComparator::diff(&old, &new_items);
        let s = TimelineChangeComparator::summary(&diff);
        assert!(s.contains("+0"));
        assert!(s.contains("~1"));
    }



    // -- Timeline Grouper Tests --

    #[test]
    fn test_grouper_single_day() {
        let items = vec![
            TimelineItem { id: "1".into(), label: "A".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "B".into(), description: None, timestamp: 200, icon_id: None, command: None },
        ];
        let groups = TimelineItemGrouper::group_by(&items, TimelineGroupPeriod::Day);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].items.len(), 2);
    }

    #[test]
    fn test_grouper_multiple_days() {
        let items = vec![
            TimelineItem { id: "1".into(), label: "A".into(), description: None, timestamp: 100, icon_id: None, command: None },
            TimelineItem { id: "2".into(), label: "B".into(), description: None, timestamp: 100000, icon_id: None, command: None },
        ];
        let groups = TimelineItemGrouper::group_by(&items, TimelineGroupPeriod::Day);
        assert!(groups.len() >= 2);
    }

    #[test]
    fn test_grouper_empty() {
        let groups = TimelineItemGrouper::group_by(&[], TimelineGroupPeriod::Week);
        assert!(groups.is_empty());
    }

    // ── TimelineWindow tests ──

    #[test]
    fn window_duration() {
        let w = TimelineWindow::new(100, 300);
        assert_eq!(w.duration(), 200);
    }

    #[test]
    fn window_contains() {
        let w = TimelineWindow::new(100, 300);
        assert!(w.contains_timestamp(100));
        assert!(w.contains_timestamp(200));
        assert!(w.contains_timestamp(300));
        assert!(!w.contains_timestamp(50));
    }

    #[test]
    fn window_overlaps() {
        let a = TimelineWindow::new(100, 300);
        let b = TimelineWindow::new(200, 400);
        assert!(a.overlaps_with(&b));
        let c = TimelineWindow::new(400, 500);
        assert!(!a.overlaps_with(&c));
    }

    #[test]
    fn window_extend_to() {
        let mut w = TimelineWindow::new(100, 300);
        w.extend_to(50);
        assert_eq!(w.start_epoch, 50);
        w.extend_to(500);
        assert_eq!(w.end_epoch, 500);
    }

    #[test]
    fn window_shrink_to() {
        let mut w = TimelineWindow::new(100, 400);
        let other = TimelineWindow::new(200, 300);
        assert!(w.shrink_to(&other));
        assert_eq!(w.start_epoch, 200);
        assert_eq!(w.end_epoch, 300);
    }

    #[test]
    fn window_shrink_no_overlap() {
        let mut w = TimelineWindow::new(100, 200);
        let other = TimelineWindow::new(300, 400);
        assert!(!w.shrink_to(&other));
    }

    #[test]
    fn window_display() {
        let w = TimelineWindow::new(0, 60);
        let s = format!("{}", w);
        assert!(s.contains("60s"));
    }

    #[test]
    fn window_auto_swap() {
        let w = TimelineWindow::new(500, 100);
        assert_eq!(w.start_epoch, 100);
        assert_eq!(w.end_epoch, 500);
    }

    // ── TimelinePageNav tests ──

    #[test]
    fn page_nav_total_pages() {
        let p = TimelinePageNav::new(10, 25);
        assert_eq!(p.total_pages(), 3);
    }

    #[test]
    fn page_nav_items_for_page() {
        let p = TimelinePageNav::new(10, 25);
        assert_eq!(p.items_for_page(0), Some((0, 10)));
        assert_eq!(p.items_for_page(2), Some((20, 25)));
        assert_eq!(p.items_for_page(3), None);
    }

    #[test]
    fn page_nav_navigation() {
        let mut p = TimelinePageNav::new(10, 30);
        assert!(!p.has_prev());
        assert!(p.has_next());
        assert!(p.next_page());
        assert_eq!(p.current_page(), 1);
        assert!(p.has_prev());
        assert!(p.prev_page());
        assert_eq!(p.current_page(), 0);
    }

    #[test]
    fn page_nav_empty() {
        let p = TimelinePageNav::new(10, 0);
        assert_eq!(p.total_pages(), 0);
        assert!(!p.has_next());
    }

    #[test]
    fn page_nav_display() {
        let p = TimelinePageNav::new(5, 20);
        let s = format!("{}", p);
        assert!(s.contains("Page 1/4"));
    }

    #[test]
    fn timeline_aggregator_add_and_count() {
        let mut agg = TimelineAggregator::new();
        let entry = TimelineItem { id: "e1".into(), label: "commit".into(), description: Some("init".into()), timestamp: 100, icon_id: None, command: None };
        agg.add_entry("2024-01", entry);
        assert_eq!(agg.bucket_count(), 1);
        assert_eq!(agg.entries_in_bucket("2024-01"), 1);
    }

    #[test]
    fn timeline_aggregator_total() {
        let mut agg = TimelineAggregator::new();
        let e1 = TimelineItem { id: "1".into(), label: "l".into(), description: None, timestamp: 1, icon_id: None, command: None };
        let e2 = TimelineItem { id: "2".into(), label: "l".into(), description: None, timestamp: 2, icon_id: None, command: None };
        agg.add_entry("a", e1);
        agg.add_entry("b", e2);
        assert_eq!(agg.total_entries(), 2);
    }

    #[test]
    fn timeline_aggregator_clear_bucket() {
        let mut agg = TimelineAggregator::new();
        let e = TimelineItem { id: "1".into(), label: "l".into(), description: None, timestamp: 0, icon_id: None, command: None };
        agg.add_entry("x", e);
        agg.clear_bucket("x");
        assert_eq!(agg.bucket_count(), 0);
    }

    #[test]
    fn timeline_aggregator_merge() {
        let mut a1 = TimelineAggregator::new();
        let mut a2 = TimelineAggregator::new();
        let e1 = TimelineItem { id: "1".into(), label: "l".into(), description: None, timestamp: 0, icon_id: None, command: None };
        let e2 = TimelineItem { id: "2".into(), label: "l".into(), description: None, timestamp: 0, icon_id: None, command: None };
        a1.add_entry("k", e1);
        a2.add_entry("k", e2);
        a1.merge(a2);
        assert_eq!(a1.entries_in_bucket("k"), 2);
    }

    #[test]
    fn timeline_cursor_navigation() {
        let mut c = TimelineCursor::new(5);
        assert!(c.is_at_start());
        assert!(c.advance());
        assert_eq!(c.position(), 1);
        assert!(c.retreat());
        assert_eq!(c.position(), 0);
    }

    #[test]
    fn timeline_cursor_at_end() {
        let mut c = TimelineCursor::new(2);
        c.advance();
        assert!(c.is_at_end());
        assert!(!c.advance());
    }

    #[test]
    fn timeline_cursor_jump() {
        let mut c = TimelineCursor::new(10);
        assert!(c.jump_to(5));
        assert_eq!(c.position(), 5);
        assert!(!c.jump_to(10));
    }

    #[test]
    fn timeline_cursor_remaining() {
        let mut c = TimelineCursor::new(5);
        assert_eq!(c.remaining(), 4);
        c.advance();
        assert_eq!(c.remaining(), 3);
    }

    #[test]
    fn timeline_cursor_progress() {
        let mut c = TimelineCursor::new(5);
        assert_eq!(c.progress_percent(), 0.0);
        c.jump_to(4);
        assert!((c.progress_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn timeline_diff_compute() {
        let before = vec!["a".into(), "b".into()];
        let after = vec!["b".into(), "c".into()];
        let diff = TimelineDiff::compute(&before, &after);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
    }

    #[test]
    fn timeline_diff_empty() {
        let same: Vec<String> = vec!["a".into()];
        let diff = TimelineDiff::compute(&same, &same);
        assert!(diff.is_empty());
    }

    #[test]
    fn timeline_diff_total_changes() {
        let before: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let after: Vec<String> = vec!["d".into()];
        let diff = TimelineDiff::compute(&before, &after);
        assert!(diff.total_changes() > 0);
    }


    // -- ext_timeline extended domain tests ----------------------------------------

    #[test]
    fn y_ext_timeline_enum_index() {
        assert_eq!(YExtTimelineTimelineProviderState::Idle.index(), 0);
        assert_eq!(YExtTimelineTimelineProviderState::Loading.index(), 1);
        assert_eq!(YExtTimelineTimelineProviderState::Loaded.index(), 2);
        assert_eq!(YExtTimelineTimelineProviderState::Error.index(), 3);
    }

    #[test]
    fn y_ext_timeline_enum_label() {
        assert_eq!(YExtTimelineTimelineProviderState::Idle.label(), "Idle");
        assert_eq!(YExtTimelineTimelineProviderState::Loading.label(), "Loading");
        assert_eq!(YExtTimelineTimelineProviderState::Loaded.label(), "Loaded");
        assert_eq!(YExtTimelineTimelineProviderState::Error.label(), "Error");
    }

    #[test]
    fn y_ext_timeline_enum_all() {
        let all = YExtTimelineTimelineProviderState::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_timeline_enum_is_default() {
        assert!(YExtTimelineTimelineProviderState::Idle.is_default());
        assert!(!YExtTimelineTimelineProviderState::Error.is_default());
    }

    #[test]
    fn y_ext_timeline_enum_display() {
        assert_eq!(format!("{}", YExtTimelineTimelineProviderState::Idle), "Idle");
    }

    #[test]
    fn y_ext_timeline_struct_new() {
        let s = YExtTimelineTimelineQuery::new();
        let _ = s.summary();
    }

    #[test]
    fn y_ext_timeline_fingerprint_deterministic() {
        let h1 = y_ext_timeline_fingerprint("hello");
        let h2 = y_ext_timeline_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_timeline_fingerprint("a"), y_ext_timeline_fingerprint("b"));
    }

    #[test]
    fn y_ext_timeline_truncate_short() {
        assert_eq!(y_ext_timeline_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_timeline_truncate_long() {
        let r = y_ext_timeline_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_timeline_normalize_key_basic() {
        assert_eq!(y_ext_timeline_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_timeline_split_path_basic() {
        let parts = y_ext_timeline_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_timeline_count_occurrences_basic() {
        assert_eq!(y_ext_timeline_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_timeline_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_timeline_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_timeline_in_range_basic() {
        assert!(y_ext_timeline_in_range(5, 1, 10));
        assert!(y_ext_timeline_in_range(1, 1, 10));
        assert!(y_ext_timeline_in_range(10, 1, 10));
        assert!(!y_ext_timeline_in_range(0, 1, 10));
        assert!(!y_ext_timeline_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_timeline_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_timeline_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_timeline_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_timeline_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_timeline Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_timeline_priority_weight() {
        assert_eq!(ZExtTimelinePriority::Idle.weight(), 0);
        assert_eq!(ZExtTimelinePriority::Normal.weight(), 2);
        assert_eq!(ZExtTimelinePriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_timeline_priority_label() {
        assert_eq!(ZExtTimelinePriority::Low.label(), "low");
        assert_eq!(ZExtTimelinePriority::High.label(), "high");
    }

    #[test]
    fn z_ext_timeline_priority_is_elevated() {
        assert!(!ZExtTimelinePriority::Normal.is_elevated());
        assert!(ZExtTimelinePriority::High.is_elevated());
        assert!(ZExtTimelinePriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_timeline_priority_display() {
        assert_eq!(format!("{}", ZExtTimelinePriority::Idle), "idle");
    }

    #[test]
    fn z_ext_timeline_priority_all_asc() {
        let all = ZExtTimelinePriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtTimelinePriority::Idle);
        assert_eq!(all[4], ZExtTimelinePriority::Realtime);
    }

    #[test]
    fn z_ext_timeline_struct_new() {
        let s = ZExtTimelineTimelinePaginator::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_timeline_struct_toggled_clone() {
        let s = ZExtTimelineTimelinePaginator::new();
        let t = s.toggled_clone();
        assert_ne!(s.has_more, t.has_more);
    }

    #[test]
    fn z_ext_timeline_rolling_hash_deterministic() {
        let h1 = z_ext_timeline_rolling_hash(b"test");
        let h2 = z_ext_timeline_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_timeline_rolling_hash(b"a"), z_ext_timeline_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_timeline_pad_to_basic() {
        assert_eq!(z_ext_timeline_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_timeline_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_timeline_is_identifier_basic() {
        assert!(z_ext_timeline_is_identifier("foo_bar"));
        assert!(z_ext_timeline_is_identifier("abc123"));
        assert!(!z_ext_timeline_is_identifier(""));
        assert!(!z_ext_timeline_is_identifier("has space"));
    }

    #[test]
    fn z_ext_timeline_levenshtein_basic() {
        assert_eq!(z_ext_timeline_levenshtein("", ""), 0);
        assert_eq!(z_ext_timeline_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_timeline_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_timeline_unique_words_basic() {
        let w = z_ext_timeline_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_timeline_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_timeline_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_timeline_common_prefix_basic() {
        assert_eq!(z_ext_timeline_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_timeline_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_timeline_struct_clear() {
        let mut s = ZExtTimelineTimelinePaginator::new();
        s.page_cursors.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_timeline_rolling_hash_empty() {
        let h = z_ext_timeline_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}
