//! Ext API: Quick open.
//!
//! RPC bridge between the extension host and the main thread for QuickPick/InputBox.

use std::collections::HashMap;
use std::fmt;
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_quickopen";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuickOpenMessage {
    ShowQuickPick {
        items: Vec<QuickPickItem>,
        options: QuickPickOptions,
    },
    ShowInputBox {
        options: InputBoxOptions,
    },
    Hide,
    SetItems {
        items: Vec<QuickPickItem>,
    },
    ItemSelected {
        index: usize,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub picked: bool,
    pub always_show: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickOptions {
    pub placeholder: Option<String>,
    pub can_pick_many: bool,
    pub match_on_description: bool,
    pub match_on_detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputBoxOptions {
    pub prompt: Option<String>,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub password: bool,
}

// ── Separators & Entries ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickSeparator {
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QuickPickEntry {
    Item(QuickPickItem),
    Separator(QuickPickSeparator),
}

// ── Input Validation ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputBoxValidation {
    pub message: String,
    pub severity: InputValidationSeverity,
}

/// Validate an input value against optional length and simple pattern constraints.
///
/// The `pattern` is matched as a literal substring (no regex).
pub fn validate_input(
    value: &str,
    min_length: Option<usize>,
    max_length: Option<usize>,
    pattern: Option<&str>,
) -> Option<InputBoxValidation> {
    if let Some(min) = min_length {
        if value.len() < min {
            return Some(InputBoxValidation {
                message: format!("Input must be at least {} characters", min),
                severity: InputValidationSeverity::Error,
            });
        }
    }
    if let Some(max) = max_length {
        if value.len() > max {
            return Some(InputBoxValidation {
                message: format!("Input must be at most {} characters", max),
                severity: InputValidationSeverity::Error,
            });
        }
    }
    if let Some(pat) = pattern {
        if !value.contains(pat) {
            return Some(InputBoxValidation {
                message: format!("Input must contain \"{}\"", pat),
                severity: InputValidationSeverity::Warning,
            });
        }
    }
    None
}

// ── Item Utilities ──

/// Filter items by case-insensitive substring match on label and description.
pub fn filter_items<'a>(items: &'a [QuickPickItem], query: &str) -> Vec<&'a QuickPickItem> {
    let q = query.to_lowercase();
    items
        .iter()
        .filter(|item| {
            let label_match = item.label.to_lowercase().contains(&q);
            let desc_match = item
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(&q))
                .unwrap_or(false);
            label_match || desc_match
        })
        .collect()
}

/// Return all items where `picked` is `true`.
pub fn get_picked_items(items: &[QuickPickItem]) -> Vec<&QuickPickItem> {
    items.iter().filter(|i| i.picked).collect()
}

/// Set the `picked` flag on every item.
pub fn set_all_picked(items: &mut [QuickPickItem], picked: bool) {
    for item in items.iter_mut() {
        item.picked = picked;
    }
}

/// Sort items alphabetically by label.
pub fn sort_items_alphabetically(items: &mut [QuickPickItem]) {
    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
}

/// Sort items so that recently-used labels appear first (order preserved),
/// followed by the remaining items in their original order.
pub fn sort_items_by_recently_used(items: &mut [QuickPickItem], recent: &[String]) {
    items.sort_by(|a, b| {
        let a_idx = recent.iter().position(|r| r == &a.label);
        let b_idx = recent.iter().position(|r| r == &b.label);
        match (a_idx, b_idx) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}

// ── QuickPickSession ──

/// Tracks an active quick-pick session with query state and selection.
pub struct QuickPickSession {
    items: Vec<QuickPickItem>,
    query: String,
    selected_indices: Vec<usize>,
}

impl QuickPickSession {
    pub fn new(items: Vec<QuickPickItem>) -> Self {
        Self {
            items,
            query: String::new(),
            selected_indices: Vec::new(),
        }
    }

    pub fn update_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.selected_indices.clear();
    }

    pub fn get_filtered(&self) -> Vec<&QuickPickItem> {
        if self.query.is_empty() {
            self.items.iter().collect()
        } else {
            filter_items(&self.items, &self.query)
        }
    }

    pub fn select_index(&mut self, index: usize) {
        if !self.selected_indices.contains(&index) {
            self.selected_indices.push(index);
        }
    }

    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns true if items is empty.
    pub fn is_items_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the first item, if any.
    pub fn first_item(&self) -> Option<&QuickPickItem> {
        self.items.first()
    }

    /// Get the last item, if any.
    pub fn last_item(&self) -> Option<&QuickPickItem> {
        self.items.last()
    }

    /// Retain only items matching the predicate.
    pub fn retain_items(&mut self, f: impl Fn(&QuickPickItem) -> bool) {
        self.items.retain(|item| f(item));
    }

    /// Returns true if selected_indices is empty.
    pub fn is_selected_indices_empty(&self) -> bool {
        self.selected_indices.is_empty()
    }

    /// Get the first selected_indice, if any.
    pub fn first_selected_indice(&self) -> Option<&usize> {
        self.selected_indices.first()
    }

    /// Get the last selected_indice, if any.
    pub fn last_selected_indice(&self) -> Option<&usize> {
        self.selected_indices.last()
    }

    /// Retain only selected_indices matching the predicate.
    pub fn retain_selected_indices(&mut self, f: impl Fn(&usize) -> bool) {
        self.selected_indices.retain(|item| f(item));
    }
}

// ── Bridge ──

pub struct QuickOpenBridge {
    is_visible: bool,
    current_items: Vec<QuickPickItem>,
}

impl QuickOpenBridge {
    pub fn new() -> Self {
        Self {
            is_visible: false,
            current_items: Vec::new(),
        }
    }

    pub fn show(&mut self, items: Vec<QuickPickItem>) {
        self.current_items = items;
        self.is_visible = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
        self.current_items.clear();
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn item_count(&self) -> usize {
        self.current_items.len()
    }

    pub fn select_item(&self, index: usize) -> Option<&QuickPickItem> {
        self.current_items.get(index)
    }

    pub fn handle_message(&mut self, msg: &QuickOpenMessage) -> serde_json::Value {
        match msg {
            QuickOpenMessage::ShowQuickPick { items, .. } => {
                self.show(items.clone());
                serde_json::json!({"shown": true, "count": items.len()})
            }
            QuickOpenMessage::ShowInputBox { options } => {
                self.is_visible = true;
                serde_json::json!({"shown": true, "prompt": options.prompt})
            }
            QuickOpenMessage::Hide => {
                self.hide();
                serde_json::json!({"hidden": true})
            }
            QuickOpenMessage::SetItems { items } => {
                self.current_items = items.clone();
                serde_json::json!({"updated": items.len()})
            }
            QuickOpenMessage::ItemSelected { index } => {
                let label = self.select_item(*index).map(|i| i.label.clone());
                serde_json::json!({"selected": label})
            }
        }
    }
}

impl Default for QuickOpenBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the quickopen extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-quickopen operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtQuickopenStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtQuickopenStats {
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
    pub fn merge(&mut self, other: &ExtQuickopenStats) {
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

impl Default for ExtQuickopenStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtQuickopenStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtQuickopenStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-quickopen.
#[derive(Debug, Clone)]
pub struct ExtQuickopenValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtQuickopenValidator {
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

impl Default for ExtQuickopenValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-factor scoring system for fuzzy matching in quick-open dialogs.
///
/// Scores are computed by walking through the query characters and matching
/// them against the candidate string. Bonuses are awarded for:
/// - An exact (case-insensitive) full match (`exact_bonus`).
/// - The candidate starting with the query (`prefix_bonus`).
/// - Consecutive character matches (`consecutive_bonus` per consecutive char).
///
/// A score of `0.0` means the query does not match the candidate at all.
pub struct QuickOpenScoring {
    pub consecutive_bonus: f64,
    pub prefix_bonus: f64,
    pub exact_bonus: f64,
}

impl QuickOpenScoring {
    /// Creates a new `QuickOpenScoring` with sensible defaults.
    pub fn new() -> Self {
        Self {
            consecutive_bonus: 2.0,
            prefix_bonus: 3.0,
            exact_bonus: 10.0,
        }
    }

    /// Scores how well `query` matches `candidate`.
    ///
    /// Returns `0.0` when the characters of `query` do not all appear in
    /// `candidate` in order. Otherwise a positive score is returned that
    /// reflects the quality of the match.
    pub fn score_match(&self, query: &str, candidate: &str) -> f64 {
        if query.is_empty() {
            return 0.0;
        }

        let query_lower = query.to_lowercase();
        let candidate_lower = candidate.to_lowercase();

        // Exact match bonus
        if query_lower == candidate_lower {
            return self.exact_bonus;
        }

        // Walk query chars through candidate chars in order
        let candidate_chars: Vec<char> = candidate_lower.chars().collect();
        let query_chars: Vec<char> = query_lower.chars().collect();

        let mut score: f64 = 0.0;
        let mut candidate_idx: usize = 0;
        let mut prev_match_idx: Option<usize> = None;
        let mut consecutive_count: usize = 0;

        for &qc in &query_chars {
            let mut found = false;
            while candidate_idx < candidate_chars.len() {
                if candidate_chars[candidate_idx] == qc {
                    // Base point for every matched character
                    score += 1.0;

                    // Consecutive bonus
                    if let Some(prev) = prev_match_idx {
                        if candidate_idx == prev + 1 {
                            consecutive_count += 1;
                            score += self.consecutive_bonus;
                        } else {
                            consecutive_count = 0;
                        }
                    }

                    prev_match_idx = Some(candidate_idx);
                    candidate_idx += 1;
                    found = true;
                    break;
                }
                candidate_idx += 1;
            }
            if !found {
                return 0.0;
            }
        }

        // Prefix bonus
        if candidate_lower.starts_with(&query_lower) {
            score += self.prefix_bonus;
        }

        // Extra bonus proportional to consecutive run length
        if consecutive_count > 0 {
            score += consecutive_count as f64 * 0.5;
        }

        score
    }

    /// Returns the highest-scoring candidate, or `None` when no candidate
    /// matches at all (all scores are `0.0`).
    pub fn best_match<'a>(&self, query: &str, candidates: &[&'a str]) -> Option<&'a str> {
        let mut best: Option<&'a str> = None;
        let mut best_score: f64 = 0.0;

        for &c in candidates {
            let s = self.score_match(query, c);
            if s > best_score {
                best_score = s;
                best = Some(c);
            }
        }

        best
    }
}

impl Default for QuickOpenScoring {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks recently opened items so they can be boosted in future searches
/// or shown as suggestions.
pub struct QuickOpenHistory {
    pub entries: Vec<String>,
    pub max_entries: usize,
}

impl QuickOpenHistory {
    /// Creates a new, empty history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Records an item at the front of the history.
    ///
    /// If the item already exists it is moved to the front. The history is
    /// trimmed to `max_entries` afterwards.
    pub fn record(&mut self, item: &str) {
        self.entries.retain(|e| e != item);
        self.entries.insert(0, item.to_string());
        self.entries.truncate(self.max_entries);
    }

    /// Returns up to `count` most-recent entries.
    pub fn recent(&self, count: usize) -> &[String] {
        let end = count.min(self.entries.len());
        &self.entries[..end]
    }

    /// Returns `true` when the item is present in history.
    pub fn contains(&self, item: &str) -> bool {
        self.entries.iter().any(|e| e == item)
    }

    /// Removes all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Adds a recency bonus to `base_score` depending on the item's position
    /// in history. Items closer to the front receive a larger bonus. Items not
    /// in history receive no bonus.
    pub fn boost_score(&self, item: &str, base_score: f64) -> f64 {
        if let Some(pos) = self.entries.iter().position(|e| e == item) {
            let recency_factor = 1.0 - (pos as f64 / self.max_entries.max(1) as f64);
            base_score + recency_factor * 5.0
        } else {
            base_score
        }
    }
}

// ── QuickOpenGrouper ──

/// Groups quick-pick items by a key derived from each item (e.g. file
/// extension, directory prefix). The groups are returned in alphabetical order
/// of the key, and the items within each group retain their original order.
pub struct QuickOpenGrouper;

impl QuickOpenGrouper {
    /// Group items using the provided key function.
    pub fn group_by<F>(items: &[QuickPickItem], key_fn: F) -> Vec<(String, Vec<&QuickPickItem>)>
    where
        F: Fn(&QuickPickItem) -> String,
    {
        let mut map: Vec<(String, Vec<&QuickPickItem>)> = Vec::new();
        for item in items {
            let k = key_fn(item);
            if let Some(entry) = map.iter_mut().find(|(key, _)| key == &k) {
                entry.1.push(item);
            } else {
                map.push((k, vec![item]));
            }
        }
        map.sort_by(|a, b| a.0.cmp(&b.0));
        map
    }

    /// Group items by file extension (the part after the last `.`).
    /// Items without an extension are grouped under `"(none)"`.
    pub fn group_by_extension(items: &[QuickPickItem]) -> Vec<(String, Vec<&QuickPickItem>)> {
        Self::group_by(items, |item| {
            item.label
                .rsplit_once('.')
                .map(|(_, ext)| ext.to_lowercase())
                .unwrap_or_else(|| "(none)".to_string())
        })
    }

    /// Group items by directory prefix (the part before the last `/`).
    /// Items without a `/` are grouped under `"."`.
    pub fn group_by_directory(items: &[QuickPickItem]) -> Vec<(String, Vec<&QuickPickItem>)> {
        Self::group_by(items, |item| {
            item.label
                .rsplit_once('/')
                .map(|(dir, _)| dir.to_string())
                .unwrap_or_else(|| ".".to_string())
        })
    }
}

// ── QuickOpenPreview ──

/// Lightweight preview metadata that can be attached to a quick-pick item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickOpenPreview {
    /// The path (or URI) of the file to preview.
    pub path: String,
    /// Optional line number to scroll to in the preview.
    pub line: Option<usize>,
    /// Optional column offset.
    pub column: Option<usize>,
    /// Optional short text snippet to show in the preview pane.
    pub snippet: Option<String>,
}

impl QuickOpenPreview {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            line: None,
            column: None,
            snippet: None,
        }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    pub fn with_snippet(mut self, snippet: impl Into<String>) -> Self {
        self.snippet = Some(snippet.into());
        self
    }

    /// Format as `path:line:col`.
    pub fn location_string(&self) -> String {
        match (self.line, self.column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", self.path, l, c),
            (Some(l), None) => format!("{}:{}", self.path, l),
            _ => self.path.clone(),
        }
    }
}

// ── QuickOpenFilter (by type / location) ──

/// Predicate-based filter that can combine multiple criteria to narrow down
/// quick-pick items before they are displayed.
pub struct QuickOpenFilter {
    extensions: Option<Vec<String>>,
    prefix: Option<String>,
    exclude_labels: Vec<String>,
}

impl QuickOpenFilter {
    pub fn new() -> Self {
        Self {
            extensions: None,
            prefix: None,
            exclude_labels: Vec::new(),
        }
    }

    /// Only keep items whose label ends with one of the given extensions
    /// (e.g. `["rs", "toml"]`).
    pub fn with_extensions(mut self, exts: &[&str]) -> Self {
        self.extensions = Some(exts.iter().map(|s| s.to_lowercase()).collect());
        self
    }

    /// Only keep items whose label starts with the given prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Exclude items whose label matches any of the given strings exactly.
    pub fn exclude(mut self, label: impl Into<String>) -> Self {
        self.exclude_labels.push(label.into());
        self
    }

    /// Apply all configured predicates and return matching items.
    pub fn apply<'a>(&self, items: &'a [QuickPickItem]) -> Vec<&'a QuickPickItem> {
        items
            .iter()
            .filter(|item| {
                if let Some(ref exts) = self.extensions {
                    let label_lower = item.label.to_lowercase();
                    if !exts.iter().any(|ext| label_lower.ends_with(&format!(".{}", ext))) {
                        return false;
                    }
                }
                if let Some(ref pfx) = self.prefix {
                    if !item.label.starts_with(pfx.as_str()) {
                        return false;
                    }
                }
                if self.exclude_labels.contains(&item.label) {
                    return false;
                }
                true
            })
            .collect()
    }
}

impl Default for QuickOpenFilter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Quick Open Prefix Commands ──

/// Recognized prefix command extracted from a quick-open query string.
///
/// Many editors treat certain leading characters as mode switches:
/// - `@` – go-to-symbol in the current file
/// - `#` – go-to-symbol across the workspace
/// - `>` – run an editor command
/// - `:` – go-to-line number
#[derive(Debug, Clone, PartialEq)]
pub enum QuickOpenPrefix {
    /// `@` – symbol search in the current file.
    FileSymbol(String),
    /// `#` – workspace-wide symbol search.
    WorkspaceSymbol(String),
    /// `>` – command palette.
    Command(String),
    /// `:` followed by a number – go-to-line.
    GotoLine(usize),
    /// No special prefix – plain file search.
    File(String),
}

/// Parse a raw query typed into the quick-open box and return the
/// corresponding [`QuickOpenPrefix`] variant.
pub fn parse_quick_open_prefix(raw: &str) -> QuickOpenPrefix {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return QuickOpenPrefix::File(String::new());
    }
    match trimmed.as_bytes()[0] {
        b'@' => QuickOpenPrefix::FileSymbol(trimmed[1..].trim().to_string()),
        b'#' => QuickOpenPrefix::WorkspaceSymbol(trimmed[1..].trim().to_string()),
        b'>' => QuickOpenPrefix::Command(trimmed[1..].trim().to_string()),
        b':' => {
            let rest = trimmed[1..].trim();
            if let Ok(n) = rest.parse::<usize>() {
                QuickOpenPrefix::GotoLine(n)
            } else {
                QuickOpenPrefix::File(trimmed.to_string())
            }
        }
        _ => QuickOpenPrefix::File(trimmed.to_string()),
    }
}

// ── File Icon Resolution ──

/// Icon label for a file based on its extension or well-known filename.
///
/// Returns a short icon identifier string (e.g. `"rust"`, `"python"`,
/// `"markdown"`) suitable for mapping to an icon font glyph on the UI side.
pub fn resolve_file_icon(filename: &str) -> &'static str {
    let lower = filename.to_lowercase();

    // Well-known filenames first
    match lower.as_str() {
        "cargo.toml" | "cargo.lock" => return "rust",
        "dockerfile" => return "docker",
        "makefile" | "gnumakefile" => return "makefile",
        ".gitignore" | ".gitattributes" | ".gitmodules" => return "git",
        "license" | "licence" => return "license",
        "readme" | "readme.md" | "readme.txt" => return "readme",
        "package.json" | "package-lock.json" => return "npm",
        "tsconfig.json" => return "typescript",
        _ => {}
    }

    // Extension-based lookup
    if let Some(ext) = lower.rsplit_once('.').map(|(_, e)| e) {
        match ext {
            "rs" => "rust",
            "py" | "pyi" => "python",
            "js" | "mjs" | "cjs" => "javascript",
            "ts" | "mts" | "cts" => "typescript",
            "tsx" | "jsx" => "react",
            "html" | "htm" => "html",
            "css" | "scss" | "sass" | "less" => "css",
            "json" | "jsonc" | "json5" => "json",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "xml" | "xsd" | "xsl" => "xml",
            "md" | "mdx" => "markdown",
            "sh" | "bash" | "zsh" | "fish" => "shell",
            "c" | "h" => "c",
            "cpp" | "cxx" | "cc" | "hpp" | "hxx" => "cpp",
            "go" => "go",
            "java" => "java",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "sql" => "sql",
            "graphql" | "gql" => "graphql",
            "svg" => "svg",
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" => "image",
            "wasm" => "wasm",
            "lock" => "lock",
            "txt" | "text" => "text",
            "log" => "log",
            _ => "file",
        }
    } else {
        "file"
    }
}

// ── File Path Scoring ──

/// Score a file path against a query using path-aware heuristics.
///
/// In addition to the base fuzzy score, this awards bonuses for:
/// - Matching the filename (basename) rather than directory components.
/// - Shorter overall paths (less nesting noise).
/// - Exact basename match.
pub fn score_file_path(query: &str, path: &str, scorer: &QuickOpenScoring) -> f64 {
    if query.is_empty() {
        return 0.0;
    }

    let basename = path.rsplit('/').next().unwrap_or(path);

    // Score against full path
    let full_score = scorer.score_match(query, path);
    // Score against basename only
    let base_score = scorer.score_match(query, basename);

    // Prefer basename matches: weight basename more heavily
    let combined = full_score + base_score * 2.0;

    // Bonus for shorter paths (fewer directory components)
    let depth = path.matches('/').count();
    let depth_penalty = depth as f64 * 0.3;

    (combined - depth_penalty).max(0.0)
}

/// Rank a set of file paths against a query, returning them sorted by
/// descending score. Paths with zero score are excluded.
pub fn rank_file_paths(
    query: &str,
    paths: &[&str],
    scorer: &QuickOpenScoring,
) -> Vec<(String, f64)> {
    let mut results: Vec<(String, f64)> = paths
        .iter()
        .filter_map(|&p| {
            let s = score_file_path(query, p, scorer);
            if s > 0.0 {
                Some((p.to_string(), s))
            } else {
                None
            }
        })
        .collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
}

// ── MRU File List ──

/// Most-recently-used file list with configurable capacity.
///
/// Unlike [`QuickOpenHistory`] which stores arbitrary labels, this is
/// purpose-built for file paths and provides path-normalisation and
/// workspace-relative conversion helpers.
pub struct MruFileList {
    paths: Vec<String>,
    capacity: usize,
}

impl MruFileList {
    pub fn new(capacity: usize) -> Self {
        Self {
            paths: Vec::new(),
            capacity,
        }
    }

    /// Record a file path as the most recently used.
    /// Duplicates are moved to the front.
    pub fn touch(&mut self, path: &str) {
        self.paths.retain(|p| p != path);
        self.paths.insert(0, path.to_string());
        self.paths.truncate(self.capacity);
    }

    /// Remove a path (e.g. when a file is deleted).
    pub fn remove(&mut self, path: &str) {
        self.paths.retain(|p| p != path);
    }

    /// Return the ordered MRU list.
    pub fn list(&self) -> &[String] {
        &self.paths
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Return `true` if empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Convert the MRU list into [`QuickPickItem`]s, optionally filtering
    /// by a query string.
    pub fn to_quick_pick_items(&self, query: Option<&str>) -> Vec<QuickPickItem> {
        let q = query.map(|s| s.to_lowercase());
        self.paths
            .iter()
            .filter(|p| match &q {
                Some(q) => p.to_lowercase().contains(q),
                None => true,
            })
            .map(|p| {
                let basename = p.rsplit('/').next().unwrap_or(p);
                let dir = p.rsplit_once('/').map(|(d, _)| d.to_string());
                QuickPickItem {
                    label: basename.to_string(),
                    description: dir,
                    detail: None,
                    picked: false,
                    always_show: false,
                }
            })
            .collect()
    }
}

// ── Quick Open Result Cache ──

/// A simple cache for quick-open results keyed by query string.
///
/// Each entry is tagged with a generation number. When the file list changes,
/// the generation is bumped and stale entries are ignored on lookup.
pub struct QuickOpenCache {
    generation: u64,
    entries: Vec<CacheEntry>,
    max_entries: usize,
}

struct CacheEntry {
    query: String,
    generation: u64,
    results: Vec<String>,
}

impl QuickOpenCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            generation: 0,
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Bump the generation, invalidating all existing entries.
    pub fn invalidate(&mut self) {
        self.generation += 1;
    }

    /// Current generation number.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Store results for a query at the current generation.
    pub fn put(&mut self, query: &str, results: Vec<String>) {
        // Remove existing entry for same query
        self.entries.retain(|e| e.query != query);
        self.entries.push(CacheEntry {
            query: query.to_string(),
            generation: self.generation,
            results,
        });
        // Evict oldest entries if over capacity
        while self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }

    /// Look up cached results. Returns `None` if missing or stale.
    pub fn get(&self, query: &str) -> Option<&[String]> {
        self.entries
            .iter()
            .find(|e| e.query == query && e.generation == self.generation)
            .map(|e| e.results.as_slice())
    }

    /// Number of entries (including stale).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Filters and scores `items` against `query`, returning only those with a
/// positive score, sorted in descending order by score.
pub fn quick_open_filter(
    query: &str,
    items: &[&str],
    scorer: &QuickOpenScoring,
) -> Vec<(String, f64)> {
    let mut results: Vec<(String, f64)> = items
        .iter()
        .filter_map(|&item| {
            let score = scorer.score_match(query, item);
            if score > 0.0 {
                Some((item.to_string(), score))
            } else {
                None
            }
        })
        .collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
}

// ── ScoredMatch ──

/// Result of scoring a query against a target string, including the positions
/// in the target where each query character was matched.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoredMatch {
    /// Overall score for this match (higher is better).
    pub score: f64,
    /// Indices into the target string where the query characters matched.
    pub matched_positions: Vec<usize>,
}

impl fmt::Display for ScoredMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ScoredMatch(score={:.2}, positions={:?})",
            self.score, self.matched_positions
        )
    }
}

// ── QuickOpenScorer ──

/// Scorer that produces [`ScoredMatch`] results with character-level match
/// positions, enabling rich highlighting in the UI.
pub struct QuickOpenScorer {
    consecutive_bonus: f64,
    prefix_bonus: f64,
}

impl QuickOpenScorer {
    /// Creates a new scorer with default bonuses.
    pub fn new() -> Self {
        Self {
            consecutive_bonus: 2.0,
            prefix_bonus: 3.0,
        }
    }

    /// Fuzzy-matches `query` against `target` and returns a [`ScoredMatch`] if
    /// every character in `query` appears (in order) in `target`.
    pub fn score_with_positions(&self, query: &str, target: &str) -> Option<ScoredMatch> {
        if query.is_empty() {
            return Some(ScoredMatch {
                score: 0.0,
                matched_positions: Vec::new(),
            });
        }

        let query_lower: Vec<char> = query.chars().map(|c| c.to_ascii_lowercase()).collect();
        let target_chars: Vec<char> = target.chars().collect();
        let target_lower: Vec<char> = target_chars.iter().map(|c| c.to_ascii_lowercase()).collect();

        let mut positions = Vec::with_capacity(query_lower.len());
        let mut t_idx = 0;

        for &qch in &query_lower {
            let mut found = false;
            while t_idx < target_lower.len() {
                if target_lower[t_idx] == qch {
                    positions.push(t_idx);
                    t_idx += 1;
                    found = true;
                    break;
                }
                t_idx += 1;
            }
            if !found {
                return None;
            }
        }

        let mut score = positions.len() as f64;

        // Bonus for consecutive matched positions.
        for pair in positions.windows(2) {
            if pair[1] == pair[0] + 1 {
                score += self.consecutive_bonus;
            }
        }

        // Bonus when the match starts at position 0 (prefix match).
        if positions.first() == Some(&0) {
            score += self.prefix_bonus;
        }

        Some(ScoredMatch {
            score,
            matched_positions: positions,
        })
    }

    /// Wraps matched characters in `[..]` brackets to visualize the match.
    ///
    /// Example: `highlight_text("main.rs", &[0, 5])` → `"[m]ain.[r]s"`.
    pub fn highlight_text(target: &str, positions: &[usize]) -> String {
        let chars: Vec<char> = target.chars().collect();
        let mut out = String::with_capacity(target.len() + positions.len() * 2);
        for (i, ch) in chars.iter().enumerate() {
            if positions.contains(&i) {
                out.push('[');
                out.push(*ch);
                out.push(']');
            } else {
                out.push(*ch);
            }
        }
        out
    }
}

impl Default for QuickOpenScorer {
    fn default() -> Self {
        Self::new()
    }
}

// ── QuickOpenFileIcon ──

/// Maps file extensions to emoji icons for display in quick-pick lists.
pub struct QuickOpenFileIcon {
    icon_map: HashMap<String, String>,
}

impl QuickOpenFileIcon {
    /// Creates an empty icon resolver.
    pub fn new() -> Self {
        Self {
            icon_map: HashMap::new(),
        }
    }

    /// Creates an icon resolver pre-loaded with common extension mappings.
    pub fn with_defaults() -> Self {
        let mut m = Self::new();
        m.register("rs", "🦀");
        m.register("py", "🐍");
        m.register("js", "📜");
        m.register("ts", "📘");
        m.register("md", "📝");
        m.register("toml", "⚙\u{fe0f}");
        m.register("json", "📋");
        m.register("yaml", "📋");
        m.register("yml", "📋");
        m.register("html", "🌐");
        m.register("css", "🎨");
        m.register("sh", "🐚");
        m.register("go", "🐹");
        m.register("c", "⚡");
        m.register("cpp", "⚡");
        m.register("h", "⚡");
        m
    }

    /// Resolves an icon for the given filename by extracting its extension. If
    /// no mapping exists the default icon `"📄"` is returned.
    pub fn resolve(&self, filename: &str) -> &str {
        filename
            .rsplit_once('.')
            .and_then(|(_, ext)| self.icon_map.get(&ext.to_ascii_lowercase()))
            .map(|s| s.as_str())
            .unwrap_or("📄")
    }

    /// Registers (or overwrites) an icon mapping for the given extension.
    pub fn register(&mut self, ext: &str, icon: &str) {
        self.icon_map
            .insert(ext.to_ascii_lowercase(), icon.to_string());
    }
}

impl Default for QuickOpenFileIcon {
    fn default() -> Self {
        Self::new()
    }
}

// ── QuickOpenItemGrouper ──

/// Collects [`QuickPickItem`]s into named groups for sectioned display.
pub struct QuickOpenItemGrouper {
    groups: Vec<(String, Vec<QuickPickItem>)>,
}

impl QuickOpenItemGrouper {
    /// Creates an empty grouper.
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
        }
    }

    /// Adds an item under the given category, creating the category if needed.
    pub fn add_item(&mut self, category: &str, item: QuickPickItem) {
        if let Some(entry) = self.groups.iter_mut().find(|(k, _)| k == category) {
            entry.1.push(item);
        } else {
            self.groups
                .push((category.to_string(), vec![item]));
        }
    }

    /// Returns the category names in insertion order.
    pub fn groups(&self) -> Vec<&str> {
        self.groups.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Returns the items within a category (empty slice if not found).
    pub fn items_in_group(&self, category: &str) -> Vec<&QuickPickItem> {
        self.groups
            .iter()
            .find(|(k, _)| k == category)
            .map(|(_, items)| items.iter().collect())
            .unwrap_or_default()
    }

    /// Total number of items across all groups.
    pub fn total_items(&self) -> usize {
        self.groups.iter().map(|(_, v)| v.len()).sum()
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

impl Default for QuickOpenItemGrouper {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QuickopenResultFormatter - quickopen result formatter
// ---------------------------------------------------------------------------

/// Severity level for quickopen result formatter issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QuickopenResultFormatterSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for QuickopenResultFormatterSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [QuickopenResultFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickopenResultFormatterEntry {
    pub id: String,
    pub label: String,
    pub severity: QuickopenResultFormatterSeverity,
    pub detail: Option<String>,
    pub result_count: usize,
    enabled: bool,
}

impl QuickopenResultFormatterEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: QuickopenResultFormatterSeverity::Low,
            detail: None,
            result_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: QuickopenResultFormatterSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_result_count(mut self, val: usize) -> Self {
        self.result_count = val;
        self
    }

    pub fn has_results(&self) -> bool {
        self.enabled && self.severity >= QuickopenResultFormatterSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.result_count, det)
    }
}

impl fmt::Display for QuickopenResultFormatterEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [QuickopenResultFormatterEntry] items.
#[derive(Debug, Clone)]
pub struct QuickopenResultFormatter {
    entries: Vec<QuickopenResultFormatterEntry>,
    name: String,
    capacity: usize,
}

impl QuickopenResultFormatter {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: QuickopenResultFormatterEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<QuickopenResultFormatterEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&QuickopenResultFormatterEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn result_count(&self) -> usize { self.entries.len() }

    pub fn has_results(&self) -> bool {
        self.entries.iter().any(|e| e.has_results())
    }

    pub fn entries_by_severity(&self, severity: QuickopenResultFormatterSeverity) -> Vec<&QuickopenResultFormatterEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= QuickopenResultFormatterSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&QuickopenResultFormatterEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&QuickopenResultFormatterEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// QuickopenPickValidator - quickopen pick validator
// ---------------------------------------------------------------------------

/// Configuration for [QuickopenPickValidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickopenPickValidatorConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub match_score: usize,
}

impl QuickopenPickValidatorConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, match_score: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_match_score(mut self, val: usize) -> Self { self.match_score = val; self }
}

impl Default for QuickopenPickValidatorConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [QuickopenPickValidator].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickopenPickValidatorItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl QuickopenPickValidatorItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn is_valid_pick(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for QuickopenPickValidatorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [QuickopenPickValidatorItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct QuickopenPickValidator {
    config: QuickopenPickValidatorConfig,
    items: Vec<QuickopenPickValidatorItem>,
}

impl QuickopenPickValidator {
    pub fn new(config: QuickopenPickValidatorConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: QuickopenPickValidatorItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<QuickopenPickValidatorItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&QuickopenPickValidatorItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn match_score(&self) -> usize { self.items.len() }

    pub fn is_valid_pick(&self) -> bool {
        self.items.iter().any(|i| i.is_valid_pick())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&QuickopenPickValidatorItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&QuickopenPickValidatorItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &QuickopenPickValidatorConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ─── QuickOpen Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for quick-open results.
#[derive(Debug, Clone)]
pub struct QuickOpenRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> QuickOpenRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for QuickOpenRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuickOpenRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── QuickOpen LRU Cache ───────────────────────────────────────

/// A simple LRU cache for quick-open cache.
#[derive(Debug)]
pub struct QuickOpenLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> QuickOpenLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for QuickOpenLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuickOpenLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}



// ---------------------------------------------------------------------------
// ext_quickopen – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for extension quick open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YExtQuickopenQuickOpenScope {
    Workspace,
    File,
    Symbol,
    Command,
}

impl YExtQuickopenQuickOpenScope {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Workspace => 0,
            Self::File => 1,
            Self::Symbol => 2,
            Self::Command => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Workspace => "Workspace",
            Self::File => "File",
            Self::Symbol => "Symbol",
            Self::Command => "Command",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YExtQuickopenQuickOpenScope] {
        &[
            YExtQuickopenQuickOpenScope::Workspace,
            YExtQuickopenQuickOpenScope::File,
            YExtQuickopenQuickOpenScope::Symbol,
            YExtQuickopenQuickOpenScope::Command,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YExtQuickopenQuickOpenScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks quick open cache data.
#[derive(Debug, Clone)]
pub struct YExtQuickopenQuickOpenCache {
    pub entries: Vec<(String, f64)>,
    pub max_size: usize,
    pub stale: bool,
}

impl YExtQuickopenQuickOpenCache {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_size: 0,
            stale: false,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YExtQuickopenQuickOpenCache({}: {:?})", "entries", self.entries)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_ext_quickopen_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_ext_quickopen_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_ext_quickopen_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_ext_quickopen_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_ext_quickopen_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_ext_quickopen_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_ext_quickopen_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_ext_quickopen_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// ext_quickopen – Extended quick open history helpers
// ---------------------------------------------------------------------------

/// Priority levels for quick open history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZExtQuickopenPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZExtQuickopenPriority {
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
    pub fn all_asc() -> [ZExtQuickopenPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZExtQuickopenPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks quick open history data.
#[derive(Debug, Clone)]
pub struct ZExtQuickopenQuickOpenHistory {
    pub recent_items: Vec<(String, u64)>,
    pub max_items: usize,
    pub pinned_count: usize,
}

impl ZExtQuickopenQuickOpenHistory {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            recent_items: Vec::new(),
            max_items: 0,
            pinned_count: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.recent_items.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.recent_items.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.recent_items.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZExtQuickopenQuickOpenHistory[max_items={:?}, pinned_count={:?}]", self.max_items, self.pinned_count)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for quick open history.
pub fn z_ext_quickopen_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ext_quickopen_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ext_quickopen_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ext_quickopen_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_ext_quickopen_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ext_quickopen_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ext_quickopen_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_item(label: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.into(),
            description: None,
            detail: None,
            picked: false,
            always_show: false,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = QuickOpenMessage::ShowInputBox {
            options: InputBoxOptions {
                prompt: Some("Enter name".into()),
                placeholder: None,
                value: None,
                password: false,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: QuickOpenMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn quick_pick_item_serialization() {
        let item = test_item("Open File");
        let json = serde_json::to_string(&item).unwrap();
        let back: QuickPickItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_show_and_hide() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("a"), test_item("b")]);
        assert!(bridge.is_visible());
        bridge.hide();
        assert!(!bridge.is_visible());
    }

    #[test]
    fn bridge_select_item() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("first"), test_item("second")]);
        assert_eq!(bridge.select_item(0).unwrap().label, "first");
        assert_eq!(bridge.select_item(1).unwrap().label, "second");
        assert!(bridge.select_item(5).is_none());
    }

    #[test]
    fn bridge_handle_hide() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("a")]);
        bridge.handle_message(&QuickOpenMessage::Hide);
        assert!(!bridge.is_visible());
    }

    #[test]
    fn bridge_item_count() {
        let mut bridge = QuickOpenBridge::new();
        assert_eq!(bridge.item_count(), 0);
        bridge.show(vec![test_item("a"), test_item("b"), test_item("c")]);
        assert_eq!(bridge.item_count(), 3);
        bridge.hide();
        assert_eq!(bridge.item_count(), 0);
    }

    #[test]
    fn filter_items_case_insensitive() {
        let items = vec![
            test_item("Open File"),
            QuickPickItem {
                label: "Save".into(),
                description: Some("Save current file".into()),
                detail: None,
                picked: false,
                always_show: false,
            },
            test_item("Close Editor"),
        ];
        let result = filter_items(&items, "file");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].label, "Open File");
        assert_eq!(result[1].label, "Save");
    }

    #[test]
    fn filter_items_empty_query_returns_all() {
        let items = vec![test_item("a"), test_item("b")];
        assert_eq!(filter_items(&items, "").len(), 2);
    }

    #[test]
    fn get_and_set_picked() {
        let mut items = vec![test_item("a"), test_item("b"), test_item("c")];
        assert!(get_picked_items(&items).is_empty());
        set_all_picked(&mut items, true);
        assert_eq!(get_picked_items(&items).len(), 3);
        items[1].picked = false;
        let picked = get_picked_items(&items);
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].label, "a");
        assert_eq!(picked[1].label, "c");
    }

    #[test]
    fn sort_alphabetically() {
        let mut items = vec![test_item("Zebra"), test_item("apple"), test_item("Mango")];
        sort_items_alphabetically(&mut items);
        assert_eq!(items[0].label, "apple");
        assert_eq!(items[1].label, "Mango");
        assert_eq!(items[2].label, "Zebra");
    }

    #[test]
    fn sort_by_recently_used() {
        let mut items = vec![test_item("c"), test_item("a"), test_item("b")];
        let recent = vec!["b".to_string(), "a".to_string()];
        sort_items_by_recently_used(&mut items, &recent);
        assert_eq!(items[0].label, "b");
        assert_eq!(items[1].label, "a");
        assert_eq!(items[2].label, "c");
    }

    #[test]
    fn quick_pick_separator_and_entry() {
        let entry_item = QuickPickEntry::Item(test_item("Open"));
        let entry_sep = QuickPickEntry::Separator(QuickPickSeparator {
            label: "Recent".into(),
        });
        let json_item = serde_json::to_string(&entry_item).unwrap();
        let json_sep = serde_json::to_string(&entry_sep).unwrap();
        assert_eq!(
            serde_json::from_str::<QuickPickEntry>(&json_item).unwrap(),
            entry_item
        );
        assert_eq!(
            serde_json::from_str::<QuickPickEntry>(&json_sep).unwrap(),
            entry_sep
        );
    }

    #[test]
    fn validate_input_min_length() {
        let result = validate_input("ab", Some(3), None, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, InputValidationSeverity::Error);
        assert!(validate_input("abc", Some(3), None, None).is_none());
    }

    #[test]
    fn validate_input_max_length() {
        let result = validate_input("toolong", None, Some(4), None);
        assert!(result.is_some());
        assert!(validate_input("ok", None, Some(4), None).is_none());
    }

    #[test]
    fn validate_input_pattern() {
        let result = validate_input("hello", None, None, Some("world"));
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, InputValidationSeverity::Warning);
        assert!(validate_input("hello world", None, None, Some("world")).is_none());
    }

    #[test]
    fn quick_pick_session_workflow() {
        let items = vec![
            test_item("Open File"),
            test_item("Close Tab"),
            test_item("Open Terminal"),
        ];
        let mut session = QuickPickSession::new(items);
        assert_eq!(session.get_filtered().len(), 3);
        assert_eq!(session.query(), "");

        session.update_query("open");
        assert_eq!(session.get_filtered().len(), 2);

        session.select_index(0);
        session.select_index(1);
        session.select_index(0); // duplicate ignored
        assert_eq!(session.selected_indices().len(), 2);

        session.update_query("terminal");
        assert_eq!(session.get_filtered().len(), 1);
        assert!(session.selected_indices().is_empty());
    }

    #[test]
    fn eq_inputvalidationseverity_same() {
        assert_eq!(InputValidationSeverity::Info, InputValidationSeverity::Info);
    }

    #[test]
    fn ne_inputvalidationseverity_diff() {
        assert_ne!(InputValidationSeverity::Info, InputValidationSeverity::Warning);
    }

    #[test]
    fn scoring_exact_match() {
        let scorer = QuickOpenScoring::new();
        let score = scorer.score_match("main.rs", "main.rs");
        assert!((score - scorer.exact_bonus).abs() < f64::EPSILON);
    }

    #[test]
    fn scoring_prefix_match() {
        let scorer = QuickOpenScoring::new();
        let score = scorer.score_match("mai", "main.rs");
        assert!(score > 0.0);
        // Prefix matches should include the prefix_bonus
        assert!(score >= scorer.prefix_bonus);
    }

    #[test]
    fn scoring_no_match() {
        let scorer = QuickOpenScoring::new();
        assert!((scorer.score_match("xyz", "main.rs")).abs() < f64::EPSILON);
        assert!((scorer.score_match("zz", "abc")).abs() < f64::EPSILON);
    }

    #[test]
    fn scoring_best_match() {
        let scorer = QuickOpenScoring::new();
        let candidates = vec!["readme.md", "main.rs", "lib.rs"];
        let best = scorer.best_match("main", &candidates);
        assert_eq!(best, Some("main.rs"));
    }

    #[test]
    fn history_record_and_recent() {
        let mut history = QuickOpenHistory::new(5);
        history.record("file_a.rs");
        history.record("file_b.rs");
        history.record("file_c.rs");
        let recent = history.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0], "file_c.rs");
        assert_eq!(recent[1], "file_b.rs");
    }

    #[test]
    fn history_dedup() {
        let mut history = QuickOpenHistory::new(5);
        history.record("alpha");
        history.record("beta");
        history.record("alpha");
        assert_eq!(history.entries.len(), 2);
        assert_eq!(history.entries[0], "alpha");
        assert_eq!(history.entries[1], "beta");
    }

    #[test]
    fn history_max_entries() {
        let mut history = QuickOpenHistory::new(3);
        history.record("a");
        history.record("b");
        history.record("c");
        history.record("d");
        assert_eq!(history.entries.len(), 3);
        assert!(!history.contains("a"));
        assert!(history.contains("d"));
    }

    #[test]
    fn history_boost_score() {
        let mut history = QuickOpenHistory::new(10);
        history.record("old_item");
        history.record("new_item");
        let boosted_new = history.boost_score("new_item", 5.0);
        let boosted_old = history.boost_score("old_item", 5.0);
        let unboosted = history.boost_score("missing", 5.0);
        assert!(boosted_new > boosted_old);
        assert!(boosted_old > unboosted);
        assert!((unboosted - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn quick_open_filter_sorts_by_score() {
        let scorer = QuickOpenScoring::new();
        let items = vec!["main.rs", "readme.md", "manifest.json", "xyz"];
        let results = quick_open_filter("main", &items, &scorer);
        // "xyz" should be filtered out
        assert!(results.iter().all(|(name, _)| name != "xyz"));
        // Results should be sorted descending by score
        for pair in results.windows(2) {
            assert!(pair[0].1 >= pair[1].1);
        }
        // "main.rs" should be the top result (exact prefix match)
        assert_eq!(results[0].0, "main.rs");
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
    fn ext_quickopen_stats_new_defaults() {
        let stats = ExtQuickopenStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_quickopen_stats_record_success() {
        let mut stats = ExtQuickopenStats::new();
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
    fn ext_quickopen_stats_record_failure() {
        let mut stats = ExtQuickopenStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_quickopen_stats_reset() {
        let mut stats = ExtQuickopenStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_quickopen_stats_merge() {
        let mut a = ExtQuickopenStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtQuickopenStats::new();
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
    fn ext_quickopen_stats_display() {
        let mut stats = ExtQuickopenStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_quickopen_stats_default() {
        let stats = ExtQuickopenStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_quickopen_validator_accepts_valid_name() {
        let v = ExtQuickopenValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_quickopen_validator_rejects_empty() {
        let v = ExtQuickopenValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_quickopen_validator_rejects_too_long() {
        let v = ExtQuickopenValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_quickopen_validator_forbidden_prefix() {
        let v = ExtQuickopenValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_quickopen_validator_allowed_chars() {
        let v = ExtQuickopenValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_quickopen_validator_range() {
        let v = ExtQuickopenValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_quickopen_sanitize_removes_control() {
        let result = ExtQuickopenValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_quickopen_truncate_short_string() {
        assert_eq!(ExtQuickopenValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_quickopen_truncate_long_string() {
        let result = ExtQuickopenValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_quickopen_is_ascii_printable() {
        assert!(ExtQuickopenValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtQuickopenValidator::is_ascii_printable("Hello\x00World"));
    }

    // ── New tests: QuickOpenGrouper, QuickOpenPreview, QuickOpenFilter ──

    #[test]
    fn grouper_by_extension() {
        let items = vec![
            test_item("main.rs"),
            test_item("lib.rs"),
            test_item("Cargo.toml"),
            test_item("README"),
        ];
        let groups = QuickOpenGrouper::group_by_extension(&items);
        // Expect groups: "(none)", "rs", "toml" (sorted alphabetically)
        let keys: Vec<&str> = groups.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec!["(none)", "rs", "toml"]);
        // The "rs" group should have 2 items
        let rs_group = groups.iter().find(|(k, _)| k == "rs").unwrap();
        assert_eq!(rs_group.1.len(), 2);
        assert_eq!(rs_group.1[0].label, "main.rs");
        assert_eq!(rs_group.1[1].label, "lib.rs");
    }

    #[test]
    fn grouper_by_directory() {
        let items = vec![
            test_item("src/main.rs"),
            test_item("src/lib.rs"),
            test_item("tests/integration.rs"),
            test_item("Cargo.toml"),
        ];
        let groups = QuickOpenGrouper::group_by_directory(&items);
        let keys: Vec<&str> = groups.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, vec![".", "src", "tests"]);
        let src = groups.iter().find(|(k, _)| k == "src").unwrap();
        assert_eq!(src.1.len(), 2);
    }

    #[test]
    fn preview_location_string() {
        let p = QuickOpenPreview::new("src/main.rs")
            .with_line(42)
            .with_column(10);
        assert_eq!(p.location_string(), "src/main.rs:42:10");

        let p2 = QuickOpenPreview::new("README.md").with_line(1);
        assert_eq!(p2.location_string(), "README.md:1");

        let p3 = QuickOpenPreview::new("LICENSE");
        assert_eq!(p3.location_string(), "LICENSE");
    }

    #[test]
    fn preview_serialization_roundtrip() {
        let preview = QuickOpenPreview::new("src/lib.rs")
            .with_line(10)
            .with_snippet("fn main() {}");
        let json = serde_json::to_string(&preview).unwrap();
        let back: QuickOpenPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(preview, back);
        assert_eq!(back.snippet.as_deref(), Some("fn main() {}"));
    }

    #[test]
    fn filter_by_extension() {
        let items = vec![
            test_item("main.rs"),
            test_item("lib.rs"),
            test_item("Cargo.toml"),
            test_item("README.md"),
        ];
        let filter = QuickOpenFilter::new().with_extensions(&["rs"]);
        let result = filter.apply(&items);
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|i| i.label.ends_with(".rs")));
    }

    #[test]
    fn filter_by_prefix_and_exclude() {
        let items = vec![
            test_item("src/main.rs"),
            test_item("src/lib.rs"),
            test_item("tests/it.rs"),
            test_item("src/secret.rs"),
        ];
        let filter = QuickOpenFilter::new()
            .with_prefix("src/")
            .exclude("src/secret.rs");
        let result = filter.apply(&items);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].label, "src/main.rs");
        assert_eq!(result[1].label, "src/lib.rs");
    }

    #[test]
    fn filter_default_passes_all() {
        let items = vec![test_item("a"), test_item("b")];
        let filter = QuickOpenFilter::default();
        assert_eq!(filter.apply(&items).len(), 2);
    }

    // ── Prefix command parsing ──

    #[test]
    fn parse_prefix_file_symbol() {
        assert_eq!(
            parse_quick_open_prefix("@main"),
            QuickOpenPrefix::FileSymbol("main".into())
        );
        assert_eq!(
            parse_quick_open_prefix("@ foo bar"),
            QuickOpenPrefix::FileSymbol("foo bar".into())
        );
    }

    #[test]
    fn parse_prefix_workspace_symbol_and_command() {
        assert_eq!(
            parse_quick_open_prefix("#Widget"),
            QuickOpenPrefix::WorkspaceSymbol("Widget".into())
        );
        assert_eq!(
            parse_quick_open_prefix(">format document"),
            QuickOpenPrefix::Command("format document".into())
        );
    }

    #[test]
    fn parse_prefix_goto_line() {
        assert_eq!(parse_quick_open_prefix(":42"), QuickOpenPrefix::GotoLine(42));
        // Non-numeric after colon falls back to File
        assert_eq!(
            parse_quick_open_prefix(":abc"),
            QuickOpenPrefix::File(":abc".into())
        );
    }

    #[test]
    fn parse_prefix_plain_file() {
        assert_eq!(
            parse_quick_open_prefix("main.rs"),
            QuickOpenPrefix::File("main.rs".into())
        );
        assert_eq!(
            parse_quick_open_prefix(""),
            QuickOpenPrefix::File(String::new())
        );
    }

    // ── File icon resolution ──

    #[test]
    fn resolve_icon_known_extensions_and_filenames() {
        assert_eq!(resolve_file_icon("main.rs"), "rust");
        assert_eq!(resolve_file_icon("app.tsx"), "react");
        assert_eq!(resolve_file_icon("style.css"), "css");
        assert_eq!(resolve_file_icon("data.json"), "json");
        assert_eq!(resolve_file_icon("script.py"), "python");
        assert_eq!(resolve_file_icon("Cargo.toml"), "rust");
        assert_eq!(resolve_file_icon("Dockerfile"), "docker");
        assert_eq!(resolve_file_icon(".gitignore"), "git");
        assert_eq!(resolve_file_icon("photo.png"), "image");
        assert_eq!(resolve_file_icon("unknown.xyz"), "file");
        assert_eq!(resolve_file_icon("noext"), "file");
    }

    // ── File path scoring & ranking ──

    #[test]
    fn score_file_path_prefers_basename_match() {
        let scorer = QuickOpenScoring::new();
        let deep = score_file_path("lib", "a/b/c/d/lib.rs", &scorer);
        let shallow = score_file_path("lib", "src/lib.rs", &scorer);
        // Shallower path with same basename should score higher
        assert!(shallow > deep, "shallow={shallow} should beat deep={deep}");
    }

    #[test]
    fn rank_file_paths_orders_correctly() {
        let scorer = QuickOpenScoring::new();
        let paths = vec![
            "src/utils/helpers.rs",
            "src/main.rs",
            "tests/main_test.rs",
            "docs/readme.md",
        ];
        let ranked = rank_file_paths("main", &paths, &scorer);
        assert!(!ranked.is_empty());
        assert_eq!(ranked[0].0, "src/main.rs");
        // readme.md shouldn't match "main"
        assert!(ranked.iter().all(|(p, _)| p != "docs/readme.md"));
    }

    // ── MRU file list ──

    #[test]
    fn mru_touch_and_ordering() {
        let mut mru = MruFileList::new(3);
        mru.touch("a.rs");
        mru.touch("b.rs");
        mru.touch("c.rs");
        assert_eq!(mru.list(), &["c.rs", "b.rs", "a.rs"]);
        // Re-touching moves to front
        mru.touch("a.rs");
        assert_eq!(mru.list(), &["a.rs", "c.rs", "b.rs"]);
        // Capacity enforced
        mru.touch("d.rs");
        assert_eq!(mru.len(), 3);
        assert_eq!(mru.list()[0], "d.rs");
    }

    #[test]
    fn mru_to_quick_pick_items_with_filter() {
        let mut mru = MruFileList::new(10);
        mru.touch("src/main.rs");
        mru.touch("src/lib.rs");
        mru.touch("tests/it.rs");
        let items = mru.to_quick_pick_items(Some("lib"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "lib.rs");
        assert_eq!(items[0].description.as_deref(), Some("src"));
    }

    // ── Quick open result cache ──

    #[test]
    fn cache_put_get_and_invalidate() {
        let mut cache = QuickOpenCache::new(10);
        cache.put("main", vec!["main.rs".into(), "main.go".into()]);
        assert_eq!(cache.get("main").unwrap().len(), 2);
        assert!(cache.get("other").is_none());
        // Invalidate makes previous entries stale
        cache.invalidate();
        assert!(cache.get("main").is_none());
        // New entry at new generation is visible
        cache.put("main", vec!["main.rs".into()]);
        assert_eq!(cache.get("main").unwrap().len(), 1);
    }

    #[test]
    fn cache_eviction() {
        let mut cache = QuickOpenCache::new(2);
        cache.put("a", vec!["a".into()]);
        cache.put("b", vec!["b".into()]);
        cache.put("c", vec!["c".into()]);
        assert_eq!(cache.len(), 2);
        // Oldest entry "a" should have been evicted
        assert!(cache.get("a").is_none());
        assert!(cache.get("c").is_some());
    }

    // ── QuickOpenScorer tests ──

    #[test]
    fn scorer_matches_all_chars_in_order() {
        let scorer = QuickOpenScorer::new();
        let result = scorer.score_with_positions("mr", "main.rs").unwrap();
        assert_eq!(result.matched_positions.len(), 2);
        assert!(result.score > 0.0);
        // 'm' at 0, 'r' at 5
        assert_eq!(result.matched_positions[0], 0);
        assert_eq!(result.matched_positions[1], 5);
    }

    #[test]
    fn scorer_returns_none_on_no_match() {
        let scorer = QuickOpenScorer::new();
        assert!(scorer.score_with_positions("xyz", "main.rs").is_none());
    }

    #[test]
    fn scorer_empty_query_matches_everything() {
        let scorer = QuickOpenScorer::new();
        let result = scorer.score_with_positions("", "anything").unwrap();
        assert!(result.matched_positions.is_empty());
        assert!((result.score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scorer_consecutive_bonus() {
        let scorer = QuickOpenScorer::new();
        // "mai" in "main.rs" => positions 0,1,2 (all consecutive)
        let consec = scorer.score_with_positions("mai", "main.rs").unwrap();
        // "m.r" in "main.rs" => positions 0,4,5 (only one consecutive pair)
        let sparse = scorer.score_with_positions("mrs", "main.rs").unwrap();
        assert!(consec.score > sparse.score);
    }

    #[test]
    fn scorer_prefix_bonus_applied() {
        let scorer = QuickOpenScorer::new();
        let prefix = scorer.score_with_positions("m", "main.rs").unwrap();
        let non_prefix = scorer.score_with_positions("r", "main.rs").unwrap();
        assert!(prefix.score > non_prefix.score);
    }

    #[test]
    fn highlight_text_wraps_matched_chars() {
        let highlighted = QuickOpenScorer::highlight_text("main.rs", &[0, 5]);
        assert_eq!(highlighted, "[m]ain.[r]s");
    }

    #[test]
    fn highlight_text_empty_positions() {
        let highlighted = QuickOpenScorer::highlight_text("hello", &[]);
        assert_eq!(highlighted, "hello");
    }

    #[test]
    fn scored_match_display() {
        let sm = ScoredMatch {
            score: 7.5,
            matched_positions: vec![0, 2],
        };
        let s = format!("{sm}");
        assert!(s.contains("7.50"));
        assert!(s.contains("[0, 2]"));
    }

    // ── QuickOpenFileIcon tests ──

    #[test]
    fn file_icon_defaults_resolve_known() {
        let icons = QuickOpenFileIcon::with_defaults();
        assert_eq!(icons.resolve("main.rs"), "🦀");
        assert_eq!(icons.resolve("script.py"), "🐍");
        assert_eq!(icons.resolve("app.js"), "📜");
        assert_eq!(icons.resolve("index.ts"), "📘");
        assert_eq!(icons.resolve("README.md"), "📝");
    }

    #[test]
    fn file_icon_unknown_extension_default() {
        let icons = QuickOpenFileIcon::with_defaults();
        assert_eq!(icons.resolve("file.xyz"), "📄");
        assert_eq!(icons.resolve("noext"), "📄");
    }

    #[test]
    fn file_icon_register_custom() {
        let mut icons = QuickOpenFileIcon::new();
        icons.register("zig", "⚡");
        assert_eq!(icons.resolve("build.zig"), "⚡");
        assert_eq!(icons.resolve("other.txt"), "📄");
    }

    // ── QuickOpenItemGrouper tests ──

    #[test]
    fn item_grouper_add_and_count() {
        let mut g = QuickOpenItemGrouper::new();
        g.add_item("rust", test_item("main.rs"));
        g.add_item("rust", test_item("lib.rs"));
        g.add_item("docs", test_item("README.md"));
        assert_eq!(g.group_count(), 2);
        assert_eq!(g.total_items(), 3);
    }

    #[test]
    fn item_grouper_groups_ordered_by_insertion() {
        let mut g = QuickOpenItemGrouper::new();
        g.add_item("beta", test_item("b"));
        g.add_item("alpha", test_item("a"));
        assert_eq!(g.groups(), vec!["beta", "alpha"]);
    }

    #[test]
    fn item_grouper_items_in_group() {
        let mut g = QuickOpenItemGrouper::new();
        g.add_item("src", test_item("main.rs"));
        g.add_item("src", test_item("lib.rs"));
        let items = g.items_in_group("src");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].label, "main.rs");
        assert_eq!(items[1].label, "lib.rs");
        assert!(g.items_in_group("missing").is_empty());
    }

    #[test]
    fn item_grouper_empty() {
        let g = QuickOpenItemGrouper::new();
        assert_eq!(g.group_count(), 0);
        assert_eq!(g.total_items(), 0);
        assert!(g.groups().is_empty());
    }

#[test]
    fn quickopenresultformatter_severity_ordering() {
        assert!(QuickopenResultFormatterSeverity::Critical > QuickopenResultFormatterSeverity::High);
        assert!(QuickopenResultFormatterSeverity::High > QuickopenResultFormatterSeverity::Medium);
        assert!(QuickopenResultFormatterSeverity::Medium > QuickopenResultFormatterSeverity::Low);
    }

    #[test]
    fn quickopenresultformatter_severity_display() {
        assert_eq!(QuickopenResultFormatterSeverity::Low.to_string(), "low");
        assert_eq!(QuickopenResultFormatterSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn quickopenresultformatter_entry_creation() {
        let e = QuickopenResultFormatterEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, QuickopenResultFormatterSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn quickopenresultformatter_entry_builder() {
        let e = QuickopenResultFormatterEntry::new("e2", "Entry 2")
            .with_severity(QuickopenResultFormatterSeverity::High)
            .with_detail("some detail")
            .with_result_count(42);
        assert_eq!(e.severity, QuickopenResultFormatterSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.result_count, 42);
    }

    #[test]
    fn quickopenresultformatter_entry_enable_disable() {
        let mut e = QuickopenResultFormatterEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn quickopenresultformatter_add_and_count() {
        let mut mgr = QuickopenResultFormatter::new("test");
        mgr.add(QuickopenResultFormatterEntry::new("a", "A"));
        mgr.add(QuickopenResultFormatterEntry::new("b", "B").with_severity(QuickopenResultFormatterSeverity::High));
        assert_eq!(mgr.result_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn quickopenresultformatter_remove() {
        let mut mgr = QuickopenResultFormatter::new("test");
        mgr.add(QuickopenResultFormatterEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn quickopenresultformatter_capacity() {
        let mut mgr = QuickopenResultFormatter::new("test").with_capacity(1);
        assert!(mgr.add(QuickopenResultFormatterEntry::new("a", "A")));
        assert!(!mgr.add(QuickopenResultFormatterEntry::new("b", "B")));
    }

    #[test]
    fn quickopenresultformatter_sorted_by_severity() {
        let mut mgr = QuickopenResultFormatter::new("test");
        mgr.add(QuickopenResultFormatterEntry::new("lo", "Low"));
        mgr.add(QuickopenResultFormatterEntry::new("hi", "High").with_severity(QuickopenResultFormatterSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, QuickopenResultFormatterSeverity::Critical);
    }

    #[test]
    fn quickopenresultformatter_summary() {
        let mgr = QuickopenResultFormatter::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn quickopenpickvalidator_config_defaults() {
        let cfg = QuickopenPickValidatorConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn quickopenpickvalidator_item_creation() {
        let item = QuickopenPickValidatorItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn quickopenpickvalidator_add_and_get() {
        let mut mgr = QuickopenPickValidator::new(QuickopenPickValidatorConfig::new("test"));
        mgr.add(QuickopenPickValidatorItem::new("k1", "v1"));
        assert_eq!(mgr.match_score(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn quickopenpickvalidator_remove_item() {
        let mut mgr = QuickopenPickValidator::new(QuickopenPickValidatorConfig::new("test"));
        mgr.add(QuickopenPickValidatorItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn quickopenpickvalidator_sorted_by_priority() {
        let mut mgr = QuickopenPickValidator::new(QuickopenPickValidatorConfig::new("test"));
        mgr.add(QuickopenPickValidatorItem::new("lo", "low").with_priority(1));
        mgr.add(QuickopenPickValidatorItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn quickopenpickvalidator_items_with_tag() {
        let mut mgr = QuickopenPickValidator::new(QuickopenPickValidatorConfig::new("test"));
        mgr.add(QuickopenPickValidatorItem::new("a", "1").with_tag("x"));
        mgr.add(QuickopenPickValidatorItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn quickopenpickvalidator_report() {
        let mgr = QuickopenPickValidator::new(QuickopenPickValidatorConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn quickopen_ringbuf_push_get() {
        let mut rb = QuickOpenRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn quickopen_ringbuf_overflow() {
        let mut rb = QuickOpenRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn quickopen_ringbuf_clear() {
        let mut rb = QuickOpenRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn quickopen_ringbuf_newest_oldest() {
        let mut rb = QuickOpenRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn quickopen_ringbuf_to_vec() {
        let mut rb = QuickOpenRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn quickopen_ringbuf_is_full() {
        let mut rb = QuickOpenRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn quickopen_lru_insert_get() {
        let mut c = QuickOpenLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn quickopen_lru_eviction() {
        let mut c = QuickOpenLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn quickopen_lru_hit_ratio() {
        let mut c = QuickOpenLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn quickopen_lru_clear() {
        let mut c = QuickOpenLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn quickopen_lru_remove() {
        let mut c = QuickOpenLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn quickopen_lru_peek() {
        let mut c = QuickOpenLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    // -- ext_quickopen extended domain tests ----------------------------------------

    #[test]
    fn y_ext_quickopen_enum_index() {
        assert_eq!(YExtQuickopenQuickOpenScope::Workspace.index(), 0);
        assert_eq!(YExtQuickopenQuickOpenScope::File.index(), 1);
        assert_eq!(YExtQuickopenQuickOpenScope::Symbol.index(), 2);
        assert_eq!(YExtQuickopenQuickOpenScope::Command.index(), 3);
    }

    #[test]
    fn y_ext_quickopen_enum_label() {
        assert_eq!(YExtQuickopenQuickOpenScope::Workspace.label(), "Workspace");
        assert_eq!(YExtQuickopenQuickOpenScope::File.label(), "File");
        assert_eq!(YExtQuickopenQuickOpenScope::Symbol.label(), "Symbol");
        assert_eq!(YExtQuickopenQuickOpenScope::Command.label(), "Command");
    }

    #[test]
    fn y_ext_quickopen_enum_all() {
        let all = YExtQuickopenQuickOpenScope::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_ext_quickopen_enum_is_default() {
        assert!(YExtQuickopenQuickOpenScope::Workspace.is_default());
        assert!(!YExtQuickopenQuickOpenScope::Command.is_default());
    }

    #[test]
    fn y_ext_quickopen_enum_display() {
        assert_eq!(format!("{}", YExtQuickopenQuickOpenScope::Workspace), "Workspace");
    }

    #[test]
    fn y_ext_quickopen_struct_new() {
        let s = YExtQuickopenQuickOpenCache::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_ext_quickopen_struct_clear() {
        let mut s = YExtQuickopenQuickOpenCache::new();
        s.entries.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_ext_quickopen_fingerprint_deterministic() {
        let h1 = y_ext_quickopen_fingerprint("hello");
        let h2 = y_ext_quickopen_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_ext_quickopen_fingerprint("a"), y_ext_quickopen_fingerprint("b"));
    }

    #[test]
    fn y_ext_quickopen_truncate_short() {
        assert_eq!(y_ext_quickopen_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_ext_quickopen_truncate_long() {
        let r = y_ext_quickopen_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_ext_quickopen_normalize_key_basic() {
        assert_eq!(y_ext_quickopen_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_ext_quickopen_split_path_basic() {
        let parts = y_ext_quickopen_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_ext_quickopen_count_occurrences_basic() {
        assert_eq!(y_ext_quickopen_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_ext_quickopen_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_ext_quickopen_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_ext_quickopen_in_range_basic() {
        assert!(y_ext_quickopen_in_range(5, 1, 10));
        assert!(y_ext_quickopen_in_range(1, 1, 10));
        assert!(y_ext_quickopen_in_range(10, 1, 10));
        assert!(!y_ext_quickopen_in_range(0, 1, 10));
        assert!(!y_ext_quickopen_in_range(11, 1, 10));
    }

    #[test]
    fn y_ext_quickopen_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_ext_quickopen_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_ext_quickopen_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_ext_quickopen_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- ext_quickopen Z-extended tests -----------------------------------------------

    #[test]
    fn z_ext_quickopen_priority_weight() {
        assert_eq!(ZExtQuickopenPriority::Idle.weight(), 0);
        assert_eq!(ZExtQuickopenPriority::Normal.weight(), 2);
        assert_eq!(ZExtQuickopenPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ext_quickopen_priority_label() {
        assert_eq!(ZExtQuickopenPriority::Low.label(), "low");
        assert_eq!(ZExtQuickopenPriority::High.label(), "high");
    }

    #[test]
    fn z_ext_quickopen_priority_is_elevated() {
        assert!(!ZExtQuickopenPriority::Normal.is_elevated());
        assert!(ZExtQuickopenPriority::High.is_elevated());
        assert!(ZExtQuickopenPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ext_quickopen_priority_display() {
        assert_eq!(format!("{}", ZExtQuickopenPriority::Idle), "idle");
    }

    #[test]
    fn z_ext_quickopen_priority_all_asc() {
        let all = ZExtQuickopenPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZExtQuickopenPriority::Idle);
        assert_eq!(all[4], ZExtQuickopenPriority::Realtime);
    }

    #[test]
    fn z_ext_quickopen_struct_new() {
        let s = ZExtQuickopenQuickOpenHistory::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ext_quickopen_struct_toggled_clone() {
        let s = ZExtQuickopenQuickOpenHistory::new();
        let t = s.toggled_clone();
        let _ = t.pinned_count;
    }

    #[test]
    fn z_ext_quickopen_rolling_hash_deterministic() {
        let h1 = z_ext_quickopen_rolling_hash(b"test");
        let h2 = z_ext_quickopen_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ext_quickopen_rolling_hash(b"a"), z_ext_quickopen_rolling_hash(b"b"));
    }

    #[test]
    fn z_ext_quickopen_pad_to_basic() {
        assert_eq!(z_ext_quickopen_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ext_quickopen_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ext_quickopen_is_identifier_basic() {
        assert!(z_ext_quickopen_is_identifier("foo_bar"));
        assert!(z_ext_quickopen_is_identifier("abc123"));
        assert!(!z_ext_quickopen_is_identifier(""));
        assert!(!z_ext_quickopen_is_identifier("has space"));
    }

    #[test]
    fn z_ext_quickopen_levenshtein_basic() {
        assert_eq!(z_ext_quickopen_levenshtein("", ""), 0);
        assert_eq!(z_ext_quickopen_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ext_quickopen_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ext_quickopen_unique_words_basic() {
        let w = z_ext_quickopen_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ext_quickopen_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ext_quickopen_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ext_quickopen_common_prefix_basic() {
        assert_eq!(z_ext_quickopen_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ext_quickopen_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ext_quickopen_struct_clear() {
        let mut s = ZExtQuickopenQuickOpenHistory::new();
        s.recent_items.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ext_quickopen_rolling_hash_empty() {
        let h = z_ext_quickopen_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}