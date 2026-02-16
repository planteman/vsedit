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
}
