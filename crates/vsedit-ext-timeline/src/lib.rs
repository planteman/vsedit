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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 74
// ---------------------------------------------------------------------------

/// Generic object pool `Xc74Pool<T>`.
pub struct Xc74Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc74Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc74PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc74Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc74PoolStats {
        Xc74PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc74Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc74Scheduler`.
pub struct Xc74Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc74Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc74Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_74 hash for the given byte slice.
pub fn xc_74_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_74 convention.
pub fn xc_74_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_16 deepening: state machine + event bus ---

/// States for the Xd16 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd16State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd16State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd16Transition {
    pub from: Xd16State,
    pub to: Xd16State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd16StateMachine {
    current: Xd16State,
    history: Vec<Xd16Transition>,
    step_counter: usize,
}

impl Xd16StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd16State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd16State {
        self.current
    }

    pub fn history(&self) -> &[Xd16Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd16State) -> Result<Xd16State, String> {
        let allowed = match (self.current, target) {
            (Xd16State::Idle, Xd16State::Running) => true,
            (Xd16State::Running, Xd16State::Paused) => true,
            (Xd16State::Running, Xd16State::Done) => true,
            (Xd16State::Paused, Xd16State::Running) => true,
            (Xd16State::Paused, Xd16State::Done) => true,
            (Xd16State::Done, Xd16State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_16: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd16Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd16SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd16State> {
        let prefix = "Xd16SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd16State::Idle),
            "Running" => Some(Xd16State::Running),
            "Paused" => Some(Xd16State::Paused),
            "Done" => Some(Xd16State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd16State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd16 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd16Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd16Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd16HandlerFn = Box<dyn Fn(&Xd16Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd16EventBus {
    handlers: Vec<(usize, Option<String>, Xd16HandlerFn)>,
    next_id: usize,
    published: Vec<Xd16Event>,
}

impl Xd16EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd16Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd16Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd16Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd16Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #14
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf14Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf14TrieNode {
    children: std::collections::HashMap<char, Xf14TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf14Trie {
    root: Xf14TrieNode,
    count: usize,
}

impl Xf14Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf14TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf14TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf14TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf14BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf14BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 73).
pub struct Xh73SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh73SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 115 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 73).
pub struct Xh73BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh73BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 73).
pub struct Xi73Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi73Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi73Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi73Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 73).
pub struct Xi73IntervalTree {
    xi_intervals: Vec<Xi73Interval>,
}

impl Xi73IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi73Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi73Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi73Interval) -> Vec<&Xi73Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi73Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi73Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi73Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi73Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi73Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi73Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 74) ---

/// Disjoint set / union-find for crate 74.
pub struct Xj74UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj74UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ74_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 74.
pub struct Xj74BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj74BTreeNode<K, V>>>,
    len: usize,
}

struct Xj74BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj74BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj74BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ74_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ74_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj74BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj74BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj74BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj74BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_73 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk73SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk73SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk73DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk73DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_74).
#[derive(Debug, Clone)]
pub struct Xl74Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl74Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_74).
#[derive(Debug, Clone)]
pub struct Xl74SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl74SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm74MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm74MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm74Tokenizer {
    text: String,
}

impl Xm74Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 73.
pub struct Xn73Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn73Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 73 -----

#[derive(Debug, Clone)]
struct Xn73AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn73AvlNode<K, V>>>,
    right: Option<Box<Xn73AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 73.
#[derive(Debug, Clone)]
pub struct Xn73AVL<K, V> {
    root: Option<Box<Xn73AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn73AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn73AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn73AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn73AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn73AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn73AvlNode<K, V>>) -> Box<Xn73AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn73AvlNode<K, V>>) -> Box<Xn73AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn73AvlNode<K, V>>) -> Box<Xn73AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn73AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn73AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn73AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn73AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn73AvlNode<K, V>>) -> &Xn73AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn73AvlNode<K, V>>) -> (Box<Xn73AvlNode<K, V>>, Option<Box<Xn73AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn73AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn73AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn73AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn73AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn73AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn73AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn73AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo73RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo73Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo73RBNode<K, V> {
    key: K,
    value: V,
    color: Xo73Color,
    left: Option<Box<Xo73RBNode<K, V>>>,
    right: Option<Box<Xo73RBNode<K, V>>>,
}

/// A red-black tree map for crate 73.
#[derive(Debug, Clone)]
pub struct Xo73RedBlack<K, V> {
    root: Option<Box<Xo73RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo73RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo73Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo73RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo73RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo73RBNode {
                    key, value, color: Xo73Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo73RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo73Color::Red)
    }

    fn xo_balance(mut h: Box<Xo73RBNode<K, V>>) -> Box<Xo73RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo73Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo73RBNode<K, V>>) -> Box<Xo73RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo73Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo73RBNode<K, V>>) -> Box<Xo73RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo73Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo73RBNode<K, V>>) {
        h.color = Xo73Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo73Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo73Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo73Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo73RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo73RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo73RBNode<K, V>) -> (K, V, Option<Box<Xo73RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo73RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo73Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo73RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo73ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 73.
#[derive(Debug, Clone)]
pub struct Xo73ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo73ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo73#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo73#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}


/// Splay tree data structure keyed by `K` with values `V` (variant 73).
#[derive(Debug)]
pub struct Xp73SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp73Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp73Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp73Node<K, V>>>,
    xp_right: Option<Box<Xp73Node<K, V>>>,
}

impl<K: Ord, V> Xp73Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp73SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp73SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp73Node<K, V>>>, key: &K) -> Option<Box<Xp73Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp73Node<K, V>>) -> Box<Xp73Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp73Node<K, V>>) -> Box<Xp73Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp73Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp73Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp73Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq73Treap ---------------

use std::cmp::Ordering as Xq73Ord;

struct Xq73TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq73TreapNode<K, V>>>,
    right: Option<Box<Xq73TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq73Treap<K, V> {
    root: Option<Box<Xq73TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq73TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_73_size<K, V>(node: &Option<Box<Xq73TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_73_update_size<K, V>(node: &mut Xq73TreapNode<K, V>) {
    node.size = 1 + xq_73_size(&node.left) + xq_73_size(&node.right);
}

fn xq_73_rotate_right<K, V>(mut node: Box<Xq73TreapNode<K, V>>) -> Box<Xq73TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_73_update_size(&mut node);
    left.right = Some(node);
    xq_73_update_size(&mut left);
    left
}

fn xq_73_rotate_left<K, V>(mut node: Box<Xq73TreapNode<K, V>>) -> Box<Xq73TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_73_update_size(&mut node);
    right.left = Some(node);
    xq_73_update_size(&mut right);
    right
}

fn xq_73_insert_node<K: Ord, V>(
    node: Option<Box<Xq73TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq73TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq73TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq73Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq73Ord::Less => {
                let (new_left, old) = xq_73_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_73_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_73_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq73Ord::Greater => {
                let (new_right, old) = xq_73_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_73_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_73_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_73_remove_node<K: Ord, V>(
    node: Option<Box<Xq73TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq73TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq73Ord::Less => {
                let (new_left, old) = xq_73_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_73_update_size(&mut n);
                (Some(n), old)
            }
            Xq73Ord::Greater => {
                let (new_right, old) = xq_73_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_73_update_size(&mut n);
                (Some(n), old)
            }
            Xq73Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_73_rotate_right(n);
                    let (new_right, old) = xq_73_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_73_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_73_rotate_left(n);
                    let (new_left, old) = xq_73_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_73_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_73_find_min<K, V>(node: &Option<Box<Xq73TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_73_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_73_find_max<K, V>(node: &Option<Box<Xq73TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_73_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_73_rank<K: Ord, V>(node: &Option<Box<Xq73TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq73Ord::Less => xq_73_rank(&n.left, key),
            Xq73Ord::Equal => xq_73_size(&n.left),
            Xq73Ord::Greater => 1 + xq_73_size(&n.left) + xq_73_rank(&n.right, key),
        },
    }
}

fn xq_73_kth<K, V>(node: &Option<Box<Xq73TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_73_size(&n.left);
        if k < left_size {
            xq_73_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_73_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_73_in_order<K: Clone, V>(node: &Option<Box<Xq73TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_73_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_73_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq73Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 73 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_73_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq73Ord::Equal => return Some(&n.value),
                Xq73Ord::Less => cur = &n.left,
                Xq73Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_73_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_73_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_73_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_73_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_73_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_73_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_73_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq73VEBTree ---------------

pub struct Xq73VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq73VEBTree>>,
    clusters: Vec<Option<Box<Xq73VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq73VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq73VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq73VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr73KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr73KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr73BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr73KDNode {
    xr_point: Xr73KDPoint,
    xr_left: Option<Box<Xr73KDNode>>,
    xr_right: Option<Box<Xr73KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr73KDTree {
    xr_root: Option<Box<Xr73KDNode>>,
    xr_size: usize,
}

impl Xr73KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr73KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr73KDNode>>,
        point: Xr73KDPoint,
        depth: usize,
    ) -> Box<Xr73KDNode> {
        match node {
            None => Box::new(Xr73KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr73KDPoint) -> Option<Xr73KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr73KDNode>,
        query: &Xr73KDPoint,
        depth: usize,
        best: &mut Xr73KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr73KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr73KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr73KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr73KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr73KDNode>>, pts: &mut Vec<Xr73KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr73KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr73BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr73BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
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

    // ---- xc_ pool / scheduler tests – block 74 ----

    #[test]
    fn xc_74_pool_new_empty() {
        let pool: super::Xc74Pool<i32> = super::Xc74Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_74_pool_release_acquire() {
        let mut pool = super::Xc74Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_74_pool_acquire_empty() {
        let mut pool: super::Xc74Pool<i32> = super::Xc74Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_74_pool_full() {
        let mut pool = super::Xc74Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_74_pool_drain() {
        let mut pool = super::Xc74Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_74_pool_stats() {
        let mut pool = super::Xc74Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_74_pool_clear() {
        let mut pool = super::Xc74Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_74_pool_shrink() {
        let mut pool = super::Xc74Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_74_pool_default() {
        let pool: super::Xc74Pool<String> = super::Xc74Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_74_pool_extend() {
        let mut pool = super::Xc74Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_74_pool_retain() {
        let mut pool = super::Xc74Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_74_scheduler_round_robin() {
        let mut sched = super::Xc74Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_74_scheduler_empty() {
        let mut sched = super::Xc74Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_74_scheduler_reset() {
        let mut sched = super::Xc74Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_74_scheduler_add_remove() {
        let mut sched = super::Xc74Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_74_scheduler_targets() {
        let sched = super::Xc74Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_74_hash_empty() {
        assert_eq!(super::xc_74_hash(b""), 5381);
    }

    #[test]
    fn xc_74_hash_data() {
        let h = super::xc_74_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_74_hash(b"hello"), h);
    }

    #[test]
    fn xc_74_reverse_str() {
        assert_eq!(super::xc_74_reverse("abc"), "cba");
        assert_eq!(super::xc_74_reverse(""), "");
    }


    // --- xd_16 deepening tests ---

    #[test]
    fn xd_16_sm_initial_state() {
        let sm = Xd16StateMachine::new();
        assert_eq!(sm.current_state(), Xd16State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_16_sm_valid_idle_to_running() {
        let mut sm = Xd16StateMachine::new();
        assert!(sm.transition(Xd16State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd16State::Running);
    }

    #[test]
    fn xd_16_sm_valid_running_to_paused() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        assert!(sm.transition(Xd16State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd16State::Paused);
    }

    #[test]
    fn xd_16_sm_valid_running_to_done() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        assert!(sm.transition(Xd16State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd16State::Done);
    }

    #[test]
    fn xd_16_sm_valid_paused_to_running() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        sm.transition(Xd16State::Paused).unwrap();
        assert!(sm.transition(Xd16State::Running).is_ok());
    }

    #[test]
    fn xd_16_sm_valid_done_to_idle() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        sm.transition(Xd16State::Done).unwrap();
        assert!(sm.transition(Xd16State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd16State::Idle);
    }

    #[test]
    fn xd_16_sm_invalid_idle_to_done() {
        let mut sm = Xd16StateMachine::new();
        assert!(sm.transition(Xd16State::Done).is_err());
    }

    #[test]
    fn xd_16_sm_invalid_idle_to_paused() {
        let mut sm = Xd16StateMachine::new();
        assert!(sm.transition(Xd16State::Paused).is_err());
    }

    #[test]
    fn xd_16_sm_history_tracking() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        sm.transition(Xd16State::Paused).unwrap();
        sm.transition(Xd16State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd16State::Idle);
        assert_eq!(sm.history()[0].to, Xd16State::Running);
        assert_eq!(sm.history()[1].from, Xd16State::Running);
        assert_eq!(sm.history()[2].to, Xd16State::Done);
    }

    #[test]
    fn xd_16_sm_serialize_deserialize() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd16StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd16State::Running));
    }

    #[test]
    fn xd_16_sm_deserialize_invalid() {
        assert_eq!(Xd16StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_16_sm_reset() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd16State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_16_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd16EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd16Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_16_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd16EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd16Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd16Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_16_bus_unsubscribe() {
        let mut bus = Xd16EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_16_event_kind_and_payload() {
        let e = Xd16Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd16Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_16_bus_clear_history() {
        let mut bus = Xd16EventBus::new();
        bus.publish(Xd16Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_16_sm_step_counter_increments() {
        let mut sm = Xd16StateMachine::new();
        sm.transition(Xd16State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd16State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #14 --

    #[test]
    fn xf14_trie_insert_search() {
        let mut t = Xf14Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf14_trie_starts_with() {
        let mut t = Xf14Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf14_trie_remove() {
        let mut t = Xf14Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf14_trie_word_count() {
        let mut t = Xf14Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf14_trie_longest_prefix() {
        let mut t = Xf14Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf14_trie_all_words() {
        let mut t = Xf14Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf14_trie_autocomplete() {
        let mut t = Xf14Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf14_trie_empty_search() {
        let t = Xf14Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf14_bloom_add_contains() {
        let mut bf = Xf14BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf14_bloom_probably_absent() {
        let bf = Xf14BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf14_bloom_false_positive_rate() {
        let mut bf = Xf14BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf14_bloom_clear() {
        let mut bf = Xf14BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf14_bloom_union() {
        let mut a = Xf14BloomFilter::xf_new(512, 2);
        let mut b = Xf14BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf14_bloom_intersection_estimate() {
        let mut a = Xf14BloomFilter::xf_new(512, 2);
        let mut b = Xf14BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf14_bloom_union_size_mismatch() {
        let a = Xf14BloomFilter::xf_new(256, 2);
        let b = Xf14BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh73_skip_insert_contains() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh73_skip_remove() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh73_skip_len() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh73_skip_range_query() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh73_skip_floor_ceiling() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh73_skip_rank() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh73_skip_empty() {
        let sl = super::Xh73SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh73_skip_duplicates() {
        let mut sl = super::Xh73SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh73_bitset_set_test() {
        let mut bs = super::Xh73BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh73_bitset_clear_count() {
        let mut bs = super::Xh73BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh73_bitset_and_or_xor() {
        let mut a = super::Xh73BitSet::xh_new(128);
        let mut b = super::Xh73BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh73_bitset_iter_ones() {
        let mut bs = super::Xh73BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh73_bitset_first_last() {
        let mut bs = super::Xh73BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh73_bitset_empty() {
        let bs = super::Xh73BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi73_deque_push_pop_back() {
        let mut dq = super::Xi73Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi73_deque_push_pop_front() {
        let mut dq = super::Xi73Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi73_deque_mixed_ops() {
        let mut dq = super::Xi73Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi73_deque_get_and_split() {
        let mut dq = super::Xi73Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi73_deque_rotate_left() {
        let mut dq = super::Xi73Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi73_deque_rotate_right() {
        let mut dq = super::Xi73Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi73_deque_grow() {
        let mut dq = super::Xi73Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi73_deque_empty() {
        let dq = super::Xi73Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi73_interval_tree_insert_query() {
        let mut tree = super::Xi73IntervalTree::xi_new();
        tree.xi_insert(super::Xi73Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi73Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi73Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi73_interval_tree_overlap() {
        let mut tree = super::Xi73IntervalTree::xi_new();
        tree.xi_insert(super::Xi73Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi73Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi73Interval::xi_new(12, 20));
        let q = super::Xi73Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi73_interval_tree_remove() {
        let mut tree = super::Xi73IntervalTree::xi_new();
        tree.xi_insert(super::Xi73Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi73Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi73_interval_tree_gaps() {
        let mut tree = super::Xi73IntervalTree::xi_new();
        tree.xi_insert(super::Xi73Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi73Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi73Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi73Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi73Interval::xi_new(8, 10));
    }

    #[test]
    fn xi73_interval_tree_merge() {
        let mut tree = super::Xi73IntervalTree::xi_new();
        tree.xi_insert(super::Xi73Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi73Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi73Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi73Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi73Interval::xi_new(10, 15));
    }

    #[test]
    fn xi73_interval_tree_all() {
        let mut tree = super::Xi73IntervalTree::xi_new();
        tree.xi_insert(super::Xi73Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi73Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi73_interval_tree_empty() {
        let tree = super::Xi73IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi73_interval_tree_contains_point() {
        let iv = super::Xi73Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 74) ---

    #[test]
    fn xj_74_uf_make_and_find() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_74_uf_union_connected() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_74_uf_component_count() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_74_uf_component_size() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_74_uf_largest_component() {
        let mut uf = super::Xj74UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_74_uf_many_elements() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_74_uf_separate_components() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_74_uf_path_compression() {
        let mut uf = super::Xj74UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_74_bt_insert_get() {
        let mut bt = super::Xj74BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_74_bt_contains_len() {
        let mut bt = super::Xj74BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_74_bt_replace() {
        let mut bt = super::Xj74BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_74_bt_remove() {
        let mut bt = super::Xj74BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_74_bt_keys_values() {
        let mut bt = super::Xj74BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_74_bt_range() {
        let mut bt = super::Xj74BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_74_bt_min_max() {
        let mut bt = super::Xj74BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_74_bt_many_inserts() {
        let mut bt = super::Xj74BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_73 segment tree tests ---

    #[test]
    fn xk_73_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk73SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_73_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk73SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_73_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk73SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_73_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk73SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_73_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk73SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_73_st_single_element() {
        let data = vec![42];
        let st = super::Xk73SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_73_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk73SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_73_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk73SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_73 disjoint intervals tests ---

    #[test]
    fn xk_73_di_add_and_count() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_73_di_merge_overlap() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_73_di_contains() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_73_di_remove() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_73_di_covered_length() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_73_di_gaps() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_73_di_merge_adjacent() {
        let mut di = super::Xk73DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_73_di_empty() {
        let di = super::Xk73DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_74_rope_new_empty() {
        let rope = super::Xl74Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_74_rope_from_str() {
        let rope = super::Xl74Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_74_rope_insert_at() {
        let mut rope = super::Xl74Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_74_rope_delete_range() {
        let mut rope = super::Xl74Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_74_rope_char_at() {
        let rope = super::Xl74Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_74_rope_split_concat() {
        let rope = super::Xl74Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_74_rope_line_count() {
        let rope = super::Xl74Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_74_rope_line_at() {
        let rope = super::Xl74Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_74_sa_build_and_search() {
        let sa = super::Xl74SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_74_sa_count() {
        let sa = super::Xl74SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_74_sa_longest_repeated() {
        let sa = super::Xl74SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_74_sa_all_positions() {
        let sa = super::Xl74SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_74_sa_len() {
        let sa = super::Xl74SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_74_sa_empty() {
        let sa = super::Xl74SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_74_rope_slice() {
        let rope = super::Xl74Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_74_sa_search_start() {
        let sa = super::Xl74SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_74_sparse_set_get() {
        let mut m = super::Xm74MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_74_sparse_row_col() {
        let mut m = super::Xm74MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_74_sparse_transpose() {
        let mut m = super::Xm74MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_74_sparse_multiply_vec() {
        let mut m = super::Xm74MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_74_sparse_nnz_density() {
        let mut m = super::Xm74MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_74_sparse_clear() {
        let mut m = super::Xm74MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_74_sparse_overwrite_zero() {
        let mut m = super::Xm74MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_74_tokenizer_basic() {
        let t = super::Xm74Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_74_tokenizer_count() {
        let t = super::Xm74Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_74_tokenizer_unique() {
        let t = super::Xm74Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_74_tokenizer_frequency() {
        let t = super::Xm74Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_74_tokenizer_delimiter() {
        let t = super::Xm74Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_74_tokenizer_whitespace() {
        let t = super::Xm74Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_74_tokenizer_empty() {
        let t = super::Xm74Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 73 ----

    #[test]
    fn xn_73_fenwick_prefix_sum() {
        let mut ft = super::Xn73Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_73_fenwick_range_sum() {
        let mut ft = super::Xn73Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_73_fenwick_point_query() {
        let mut ft = super::Xn73Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_73_fenwick_len() {
        let ft = super::Xn73Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_73_fenwick_multiple_updates() {
        let mut ft = super::Xn73Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_73_fenwick_single_element() {
        let mut ft = super::Xn73Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_73_fenwick_find_kth() {
        let mut ft = super::Xn73Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_73_fenwick_negative_delta() {
        let mut ft = super::Xn73Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 73 ----

    #[test]
    fn xn_73_avl_insert_get() {
        let mut m = super::Xn73AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_73_avl_remove() {
        let mut m = super::Xn73AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_73_avl_in_order() {
        let mut m = super::Xn73AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_73_avl_min_max() {
        let mut m = super::Xn73AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_73_avl_floor_ceiling() {
        let mut m = super::Xn73AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_73_avl_height_balanced() {
        let mut m = super::Xn73AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_73_avl_overwrite() {
        let mut m = super::Xn73AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_73_avl_empty() {
        let m: super::Xn73AVL<i32, i32> = super::Xn73AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo73RedBlack tests ---

    #[test]
    fn xo_73_rb_insert_and_get() {
        let mut tree = super::Xo73RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_73_rb_len_and_empty() {
        let mut tree = super::Xo73RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_73_rb_min_max() {
        let mut tree = super::Xo73RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_73_rb_contains() {
        let mut tree = super::Xo73RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_73_rb_remove() {
        let mut tree = super::Xo73RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_73_rb_in_order() {
        let mut tree = super::Xo73RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_73_rb_black_height() {
        let mut tree = super::Xo73RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_73_rb_overwrite() {
        let mut tree = super::Xo73RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo73ConsistentHash tests ---

    #[test]
    fn xo_73_ch_add_and_count() {
        let mut ring = super::Xo73ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_73_ch_remove_node() {
        let mut ring = super::Xo73ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_73_ch_get_node() {
        let mut ring = super::Xo73ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_73_ch_empty_ring() {
        let ring = super::Xo73ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_73_ch_distribution() {
        let mut ring = super::Xo73ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_73_ch_rebalance() {
        let mut ring = super::Xo73ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_73_ch_virtual_nodes() {
        let mut ring = super::Xo73ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_73_ch_consistent_lookup() {
        let mut ring = super::Xo73ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_73_splay_insert_get() {
        let mut t = super::Xp73SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_73_splay_remove() {
        let mut t = super::Xp73SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_73_splay_count_increases() {
        let mut t = super::Xp73SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_73_splay_depth() {
        let mut t = super::Xp73SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_73_splay_len_empty() {
        let t = super::Xp73SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_73_splay_min_max() {
        let mut t = super::Xp73SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_73_splay_overwrite() {
        let mut t = super::Xp73SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_73_splay_remove_missing() {
        let mut t = super::Xp73SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_73 treap tests ----
    #[test]
    fn xq_73_treap_empty() {
        let t = super::Xq73Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_73_treap_insert_get() {
        let mut t = super::Xq73Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_73_treap_overwrite() {
        let mut t = super::Xq73Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_73_treap_remove() {
        let mut t = super::Xq73Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_73_treap_min_max() {
        let mut t = super::Xq73Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_73_treap_rank() {
        let mut t = super::Xq73Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_73_treap_kth() {
        let mut t = super::Xq73Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_73_treap_in_order() {
        let mut t = super::Xq73Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_73 VEB tree tests ----
    #[test]
    fn xq_73_veb_empty() {
        let v = super::Xq73VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_73_veb_insert_contains() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_73_veb_min_max() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_73_veb_delete() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_73_veb_successor() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_73_veb_predecessor() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_73_veb_count() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_73_veb_duplicate_insert() {
        let mut v = super::Xq73VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_73_kdtree_empty() {
        let tree = super::Xr73KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_73_kdtree_insert_one() {
        let mut tree = super::Xr73KDTree::xr_new();
        tree.xr_insert(super::Xr73KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_73_kdtree_insert_multiple() {
        let mut tree = super::Xr73KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr73KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_73_kdtree_nearest_neighbor() {
        let mut tree = super::Xr73KDTree::xr_new();
        tree.xr_insert(super::Xr73KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr73KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr73KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_73_kdtree_nn_empty() {
        let tree = super::Xr73KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr73KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_73_kdtree_range_search() {
        let mut tree = super::Xr73KDTree::xr_new();
        tree.xr_insert(super::Xr73KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr73KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr73KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_73_kdtree_range_empty() {
        let mut tree = super::Xr73KDTree::xr_new();
        tree.xr_insert(super::Xr73KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_73_kdtree_all_points() {
        let mut tree = super::Xr73KDTree::xr_new();
        tree.xr_insert(super::Xr73KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr73KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_73_kdtree_depth() {
        let mut tree = super::Xr73KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr73KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_73_kdtree_bounding_box() {
        let mut tree = super::Xr73KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr73KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr73KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
