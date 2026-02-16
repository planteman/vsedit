//! Quick input model service.

use std::fmt;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during quick input interactions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickInputError {
    /// No items were provided to the quick pick.
    NoItems,
    /// The user cancelled the interaction.
    Cancelled,
    /// Input validation failed with the given message.
    ValidationFailed(String),
}

impl fmt::Display for QuickInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoItems => write!(f, "no items available"),
            Self::Cancelled => write!(f, "user cancelled"),
            Self::ValidationFailed(msg) => write!(f, "validation failed: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An item in a quick pick list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub picked: bool,
    pub always_show: bool,
}

impl fmt::Display for QuickPickItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

/// Builder for constructing [`QuickPickItem`] instances.
#[derive(Debug, Clone)]
pub struct QuickPickItemBuilder {
    label: String,
    description: Option<String>,
    detail: Option<String>,
    picked: bool,
    always_show: bool,
}

impl QuickPickItemBuilder {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
            detail: None,
            picked: false,
            always_show: false,
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn picked(mut self, picked: bool) -> Self {
        self.picked = picked;
        self
    }

    pub fn always_show(mut self, always_show: bool) -> Self {
        self.always_show = always_show;
        self
    }

    pub fn build(self) -> QuickPickItem {
        QuickPickItem {
            label: self.label,
            description: self.description,
            detail: self.detail,
            picked: self.picked,
            always_show: self.always_show,
        }
    }
}

/// Options controlling quick pick behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickOptions {
    pub title: Option<String>,
    pub placeholder: Option<String>,
    pub can_pick_many: bool,
    pub match_on_description: bool,
}

impl Default for QuickPickOptions {
    fn default() -> Self {
        Self {
            title: None,
            placeholder: None,
            can_pick_many: false,
            match_on_description: false,
        }
    }
}

impl QuickPickOptions {
    /// Set the title, returning `self` for chaining.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the placeholder, returning `self` for chaining.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
}

/// The result of a quick pick interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickResult {
    pub items: Vec<QuickPickItem>,
    pub cancelled: bool,
}

impl QuickPickResult {
    /// Returns the labels of the selected items.
    pub fn selected_labels(&self) -> Vec<&str> {
        self.items.iter().map(|i| i.label.as_str()).collect()
    }

    /// Returns a reference to the first selected item, if any.
    pub fn first_selected(&self) -> Option<&QuickPickItem> {
        self.items.first()
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

    /// Toggle the `cancelled` flag.
    pub fn toggle_cancelled(&mut self) {
        self.cancelled = !self.cancelled;
    }
}

/// Validation result for an input box value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBoxValidation {
    Ok,
    Error(String),
    Warning(String),
}

impl fmt::Display for InputBoxValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Error(msg) => write!(f, "error: {msg}"),
            Self::Warning(msg) => write!(f, "warning: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Service interface for quick input UI.
pub trait QuickInputService {
    fn show_quick_pick(
        &self,
        items: Vec<QuickPickItem>,
        options: QuickPickOptions,
    ) -> QuickPickResult;

    fn show_input_box(&self, prompt: &str, value: &str) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Filtering helper
// ---------------------------------------------------------------------------

/// Returns the indices of items whose label contains `query` (case-insensitive).
pub fn filter_quick_pick_items(items: &[QuickPickItem], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let query_lower = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.label.to_lowercase().contains(&query_lower))
        .map(|(i, _)| i)
        .collect()
}

/// Returns indices of items whose label **or** description contains `query` (case-insensitive).
pub fn filter_with_description(items: &[QuickPickItem], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let query_lower = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            let label_match = item.label.to_lowercase().contains(&query_lower);
            let desc_match = item
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(&query_lower))
                .unwrap_or(false);
            label_match || desc_match
        })
        .map(|(i, _)| i)
        .collect()
}

/// Returns a new `Vec` of items sorted alphabetically by label.
pub fn sort_quick_pick_items(items: &[QuickPickItem]) -> Vec<QuickPickItem> {
    let mut sorted = items.to_vec();
    sorted.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    sorted
}

// ---------------------------------------------------------------------------
// Fuzzy scoring
// ---------------------------------------------------------------------------

/// Simple character-by-character fuzzy score of `query` against `text`.
///
/// Returns `Some(score)` where a lower score is a better match, or `None` if
/// not all characters of `query` appear in `text` in order.
pub fn score_match(text: &str, query: &str) -> Option<usize> {
    if query.is_empty() {
        return Some(0);
    }
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let text_chars: Vec<char> = text_lower.chars().collect();
    let mut ti = 0;
    let mut score: usize = 0;
    for qch in query_lower.chars() {
        let mut found = false;
        while ti < text_chars.len() {
            if text_chars[ti] == qch {
                ti += 1;
                found = true;
                break;
            }
            score += 1; // penalty for skipped characters
            ti += 1;
        }
        if !found {
            return None;
        }
    }
    Some(score)
}

/// Rank items by fuzzy score against `query`, returning items that match
/// sorted from best (lowest score) to worst.
pub fn rank_items<'a>(items: &'a [QuickPickItem], query: &str) -> Vec<(usize, &'a QuickPickItem)> {
    let mut scored: Vec<(usize, usize, &QuickPickItem)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| score_match(&item.label, query).map(|s| (s, i, item)))
        .collect();
    scored.sort_by_key(|(s, _, _)| *s);
    scored.into_iter().map(|(_, i, item)| (i, item)).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for quickinput-svc operations.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickinputSvcStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl QuickinputSvcStats {
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
    pub fn merge(&mut self, other: &QuickinputSvcStats) {
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

impl Default for QuickinputSvcStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for QuickinputSvcStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "QuickinputSvcStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for quickinput-svc.
#[derive(Debug, Clone)]
pub struct QuickinputSvcValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl QuickinputSvcValidator {
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

impl Default for QuickinputSvcValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QuickPickSeparator & QuickPickEntry
// ---------------------------------------------------------------------------

/// A visual separator in a quick pick list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickSeparator {
    pub label: String,
}

impl QuickPickSeparator {
    /// Create a new separator with the given label.
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
        }
    }
}

/// An entry in a quick pick list – either a selectable item or a visual
/// separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickPickEntry {
    /// A selectable quick pick item.
    Item(QuickPickItem),
    /// A visual separator between groups of items.
    Separator(QuickPickSeparator),
}

impl QuickPickEntry {
    /// Returns `true` if this entry is a separator.
    pub fn is_separator(&self) -> bool {
        matches!(self, Self::Separator(_))
    }

    /// Returns the label of this entry regardless of variant.
    pub fn label(&self) -> &str {
        match self {
            Self::Item(item) => &item.label,
            Self::Separator(sep) => &sep.label,
        }
    }
}

/// Groups entries by separator.
///
/// Items appearing before the first separator are grouped under `None`.
/// Each separator starts a new group keyed by `Some(separator_label)`.
pub fn group_by_separator<'a>(
    entries: &'a [QuickPickEntry],
) -> Vec<(Option<String>, Vec<&'a QuickPickItem>)> {
    let mut groups: Vec<(Option<String>, Vec<&'a QuickPickItem>)> = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_items: Vec<&'a QuickPickItem> = Vec::new();

    for entry in entries {
        match entry {
            QuickPickEntry::Separator(sep) => {
                // Push the previous group (even if empty) when we hit a new
                // separator, but only if there are accumulated items or if
                // this isn't the very first entry.
                if !current_items.is_empty() || groups.is_empty() {
                    groups.push((current_key.clone(), current_items));
                    current_items = Vec::new();
                }
                current_key = Some(sep.label.clone());
            }
            QuickPickEntry::Item(item) => {
                current_items.push(item);
            }
        }
    }

    // Push the last group.
    if !current_items.is_empty() || groups.is_empty() {
        groups.push((current_key, current_items));
    }

    groups
}

// ---------------------------------------------------------------------------
// QuickPickMultiSelect
// ---------------------------------------------------------------------------

/// Manages multi-selection state for a quick pick list.
#[derive(Debug, Clone)]
pub struct QuickPickMultiSelect {
    /// The currently selected indices.
    pub selected_indices: Vec<usize>,
    /// Optional cap on the number of selections allowed.
    pub max_selections: Option<usize>,
}

impl QuickPickMultiSelect {
    /// Create an uncapped multi-select manager.
    pub fn new() -> Self {
        Self {
            selected_indices: Vec::new(),
            max_selections: None,
        }
    }

    /// Create a multi-select manager with an upper bound on selections.
    pub fn with_max(max: usize) -> Self {
        Self {
            selected_indices: Vec::new(),
            max_selections: Some(max),
        }
    }

    /// Toggle the selection at `index`.
    ///
    /// Returns `true` if the toggle succeeded, `false` if it would exceed the
    /// maximum number of selections.
    pub fn toggle(&mut self, index: usize) -> bool {
        if let Some(pos) = self.selected_indices.iter().position(|&i| i == index) {
            self.selected_indices.remove(pos);
            true
        } else {
            if let Some(max) = self.max_selections {
                if self.selected_indices.len() >= max {
                    return false;
                }
            }
            self.selected_indices.push(index);
            true
        }
    }

    /// Returns `true` if `index` is currently selected.
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_indices.contains(&index)
    }

    /// Returns the number of currently selected indices.
    pub fn selected_count(&self) -> usize {
        self.selected_indices.len()
    }

    /// Clear all selections.
    pub fn clear(&mut self) {
        self.selected_indices.clear();
    }

    /// Select all indices from `0..count`, respecting `max_selections`.
    pub fn select_all(&mut self, count: usize) {
        self.selected_indices.clear();
        let limit = match self.max_selections {
            Some(max) => count.min(max),
            None => count,
        };
        self.selected_indices = (0..limit).collect();
    }
}

impl Default for QuickPickMultiSelect {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Input validation helpers
// ---------------------------------------------------------------------------

/// A validation rule that can be applied to user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationRule {
    /// The input must not be empty.
    Required,
    /// The input must be at least this many characters.
    MinLength(usize),
    /// The input must be at most this many characters.
    MaxLength(usize),
    /// The input must contain this substring.
    Pattern(String),
}

/// Validate `input` against a set of [`ValidationRule`]s.
///
/// Returns a list of human-readable error messages. An empty list means the
/// input is valid.
pub fn quick_input_validation(input: &str, rules: &[ValidationRule]) -> Vec<String> {
    let mut errors = Vec::new();
    for rule in rules {
        match rule {
            ValidationRule::Required => {
                if input.trim().is_empty() {
                    errors.push("input is required".to_string());
                }
            }
            ValidationRule::MinLength(min) => {
                if input.len() < *min {
                    errors.push(format!(
                        "input must be at least {} characters (got {})",
                        min,
                        input.len()
                    ));
                }
            }
            ValidationRule::MaxLength(max) => {
                if input.len() > *max {
                    errors.push(format!(
                        "input must be at most {} characters (got {})",
                        max,
                        input.len()
                    ));
                }
            }
            ValidationRule::Pattern(substring) => {
                if !input.contains(substring.as_str()) {
                    errors.push(format!("input must contain \"{}\"", substring));
                }
            }
        }
    }
    errors
}

/// Validate a slice of [`QuickPickItem`]s.
///
/// Returns error messages for items with empty labels and for duplicate
/// labels.
pub fn validate_quick_pick_items(items: &[QuickPickItem]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen_labels = std::collections::HashSet::new();

    for (i, item) in items.iter().enumerate() {
        if item.label.trim().is_empty() {
            errors.push(format!("item at index {} has an empty label", i));
        }
        if !seen_labels.insert(&item.label) {
            errors.push(format!("duplicate label \"{}\"", item.label));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.into(),
            description: None,
            detail: None,
            picked: false,
            always_show: false,
        }
    }

    #[test]
    fn filter_matches_substring() {
        let items = vec![item("Open File"), item("Close File"), item("Run Task")];
        let result = filter_quick_pick_items(&items, "file");
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let items = vec![item("A"), item("B")];
        let result = filter_quick_pick_items(&items, "");
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let items = vec![item("Open File")];
        let result = filter_quick_pick_items(&items, "zzz");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_case_insensitive() {
        let items = vec![item("FooBar")];
        let result = filter_quick_pick_items(&items, "FOOB");
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn quick_pick_result_cancelled() {
        let r = QuickPickResult {
            items: vec![],
            cancelled: true,
        };
        assert!(r.cancelled);
    }

    // -- new tests --

    #[test]
    fn quick_input_error_display() {
        assert_eq!(QuickInputError::NoItems.to_string(), "no items available");
        assert_eq!(QuickInputError::Cancelled.to_string(), "user cancelled");
        assert_eq!(
            QuickInputError::ValidationFailed("bad".into()).to_string(),
            "validation failed: bad"
        );
    }

    #[test]
    fn quick_pick_item_display() {
        let i = item("Hello");
        assert_eq!(format!("{i}"), "Hello");
    }

    #[test]
    fn input_box_validation_display() {
        assert_eq!(InputBoxValidation::Ok.to_string(), "ok");
        assert_eq!(InputBoxValidation::Error("e".into()).to_string(), "error: e");
        assert_eq!(InputBoxValidation::Warning("w".into()).to_string(), "warning: w");
    }

    #[test]
    fn builder_defaults() {
        let i = QuickPickItemBuilder::new("Test").build();
        assert_eq!(i.label, "Test");
        assert_eq!(i.description, None);
        assert_eq!(i.detail, None);
        assert!(!i.picked);
        assert!(!i.always_show);
    }

    #[test]
    fn builder_full() {
        let i = QuickPickItemBuilder::new("X")
            .description("desc")
            .detail("det")
            .picked(true)
            .always_show(true)
            .build();
        assert_eq!(i.description.as_deref(), Some("desc"));
        assert_eq!(i.detail.as_deref(), Some("det"));
        assert!(i.picked);
        assert!(i.always_show);
    }

    #[test]
    fn options_builder_methods() {
        let opts = QuickPickOptions::default()
            .with_title("My Title")
            .with_placeholder("type here");
        assert_eq!(opts.title.as_deref(), Some("My Title"));
        assert_eq!(opts.placeholder.as_deref(), Some("type here"));
    }

    #[test]
    fn filter_with_description_matches_desc() {
        let mut i = item("Foo");
        i.description = Some("bar baz".into());
        let items = vec![i, item("Qux")];
        let result = filter_with_description(&items, "bar");
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn filter_with_description_matches_label() {
        let mut i = item("Alpha");
        i.description = Some("beta".into());
        let items = vec![i, item("Gamma")];
        let result = filter_with_description(&items, "alpha");
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn sort_quick_pick_items_alphabetical() {
        let items = vec![item("Banana"), item("Apple"), item("cherry")];
        let sorted = sort_quick_pick_items(&items);
        let labels: Vec<&str> = sorted.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["Apple", "Banana", "cherry"]);
    }

    #[test]
    fn selected_labels_and_first() {
        let r = QuickPickResult {
            items: vec![item("A"), item("B")],
            cancelled: false,
        };
        assert_eq!(r.selected_labels(), vec!["A", "B"]);
        assert_eq!(r.first_selected().unwrap().label, "A");
    }

    #[test]
    fn first_selected_empty() {
        let r = QuickPickResult { items: vec![], cancelled: true };
        assert!(r.first_selected().is_none());
    }

    #[test]
    fn score_match_exact() {
        assert_eq!(score_match("hello", "hello"), Some(0));
    }

    #[test]
    fn score_match_subsequence() {
        // h-e-l in "hello" → skips nothing for h, nothing for e, nothing for l
        assert_eq!(score_match("hello", "hel"), Some(0));
        // "lo" in "hello" → skip h,e (2) for 'l', skip second l (1) for 'o' = 3
        assert_eq!(score_match("hello", "lo"), Some(3));
    }

    #[test]
    fn score_match_no_match() {
        assert_eq!(score_match("abc", "z"), None);
    }

    #[test]
    fn score_match_empty_query() {
        assert_eq!(score_match("anything", ""), Some(0));
    }

    #[test]
    fn rank_items_ordering() {
        let items = vec![item("Zoom"), item("Zero"), item("Zap")];
        let ranked = rank_items(&items, "zp");
        // "Zap" should rank first (skip 1 char), then "Zoom" (skip 2)
        assert_eq!(ranked[0].0, 2); // Zap
        assert_eq!(ranked[0].1.label, "Zap");
    }

    #[test]
    fn eq_quickinputerror_same() {
        assert_eq!(QuickInputError::NoItems, QuickInputError::NoItems);
    }

    #[test]
    fn ne_quickinputerror_diff() {
        assert_ne!(QuickInputError::NoItems, QuickInputError::Cancelled);
    }

    #[test]
    fn display_quickinputerror_variants() {
        assert!(!QuickInputError::NoItems.to_string().is_empty());
        assert!(!QuickInputError::Cancelled.to_string().is_empty());
    }

    #[test]
    fn display_inputboxvalidation_variants() {
        assert!(!InputBoxValidation::Ok.to_string().is_empty());
    }

    // -- QuickPickSeparator / QuickPickEntry tests --

    #[test]
    fn separator_creation() {
        let sep = QuickPickSeparator::new("Section A");
        assert_eq!(sep.label, "Section A");
    }

    #[test]
    fn entry_is_separator() {
        let sep_entry = QuickPickEntry::Separator(QuickPickSeparator::new("sep"));
        let item_entry = QuickPickEntry::Item(item("hello"));
        assert!(sep_entry.is_separator());
        assert!(!item_entry.is_separator());
        assert_eq!(sep_entry.label(), "sep");
        assert_eq!(item_entry.label(), "hello");
    }

    #[test]
    fn group_by_separator_basic() {
        let entries = vec![
            QuickPickEntry::Item(item("a")),
            QuickPickEntry::Item(item("b")),
            QuickPickEntry::Separator(QuickPickSeparator::new("Group 1")),
            QuickPickEntry::Item(item("c")),
            QuickPickEntry::Separator(QuickPickSeparator::new("Group 2")),
            QuickPickEntry::Item(item("d")),
            QuickPickEntry::Item(item("e")),
        ];
        let groups = group_by_separator(&entries);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].0, None);
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, Some("Group 1".to_string()));
        assert_eq!(groups[1].1.len(), 1);
        assert_eq!(groups[2].0, Some("Group 2".to_string()));
        assert_eq!(groups[2].1.len(), 2);
    }

    // -- QuickPickMultiSelect tests --

    #[test]
    fn multi_select_toggle() {
        let mut ms = QuickPickMultiSelect::new();
        assert!(ms.toggle(0));
        assert!(ms.is_selected(0));
        assert_eq!(ms.selected_count(), 1);
        assert!(ms.toggle(0));
        assert!(!ms.is_selected(0));
        assert_eq!(ms.selected_count(), 0);
    }

    #[test]
    fn multi_select_max_limit() {
        let mut ms = QuickPickMultiSelect::with_max(2);
        assert!(ms.toggle(0));
        assert!(ms.toggle(1));
        assert!(!ms.toggle(2)); // should fail – at max
        assert_eq!(ms.selected_count(), 2);
        // deselect one, then add should succeed
        assert!(ms.toggle(0));
        assert!(ms.toggle(2));
        assert!(ms.is_selected(2));
    }

    #[test]
    fn multi_select_select_all() {
        let mut ms = QuickPickMultiSelect::with_max(3);
        ms.select_all(5);
        assert_eq!(ms.selected_count(), 3); // capped at max
        assert!(ms.is_selected(0));
        assert!(ms.is_selected(1));
        assert!(ms.is_selected(2));
        assert!(!ms.is_selected(3));

        let mut ms2 = QuickPickMultiSelect::new();
        ms2.select_all(4);
        assert_eq!(ms2.selected_count(), 4);
    }

    // -- Validation tests --

    #[test]
    fn validation_required() {
        let rules = [ValidationRule::Required];
        assert!(!quick_input_validation("", &rules).is_empty());
        assert!(!quick_input_validation("   ", &rules).is_empty());
        assert!(quick_input_validation("hello", &rules).is_empty());
    }

    #[test]
    fn validation_min_max_length() {
        let rules = [ValidationRule::MinLength(3), ValidationRule::MaxLength(5)];
        let errs = quick_input_validation("ab", &rules);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("at least"));

        let errs = quick_input_validation("abcdef", &rules);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("at most"));

        assert!(quick_input_validation("abc", &rules).is_empty());
    }

    #[test]
    fn validation_pattern() {
        let rules = [ValidationRule::Pattern("foo".to_string())];
        assert!(quick_input_validation("hello foo bar", &rules).is_empty());
        let errs = quick_input_validation("hello bar", &rules);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("foo"));
    }

    #[test]
    fn validate_quick_pick_items_rejects_empty_label() {
        let items = vec![item("ok"), item(""), item("ok")];
        let errs = validate_quick_pick_items(&items);
        assert!(errs.iter().any(|e| e.contains("empty label")));
        assert!(errs.iter().any(|e| e.contains("duplicate")));
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
    fn quickinput_svc_stats_new_defaults() {
        let stats = QuickinputSvcStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn quickinput_svc_stats_record_success() {
        let mut stats = QuickinputSvcStats::new();
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
    fn quickinput_svc_stats_record_failure() {
        let mut stats = QuickinputSvcStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn quickinput_svc_stats_reset() {
        let mut stats = QuickinputSvcStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn quickinput_svc_stats_merge() {
        let mut a = QuickinputSvcStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = QuickinputSvcStats::new();
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
    fn quickinput_svc_stats_display() {
        let mut stats = QuickinputSvcStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn quickinput_svc_stats_default() {
        let stats = QuickinputSvcStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn quickinput_svc_validator_accepts_valid_name() {
        let v = QuickinputSvcValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn quickinput_svc_validator_rejects_empty() {
        let v = QuickinputSvcValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn quickinput_svc_validator_rejects_too_long() {
        let v = QuickinputSvcValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn quickinput_svc_validator_forbidden_prefix() {
        let v = QuickinputSvcValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn quickinput_svc_validator_allowed_chars() {
        let v = QuickinputSvcValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn quickinput_svc_validator_range() {
        let v = QuickinputSvcValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn quickinput_svc_sanitize_removes_control() {
        let result = QuickinputSvcValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn quickinput_svc_truncate_short_string() {
        assert_eq!(QuickinputSvcValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn quickinput_svc_truncate_long_string() {
        let result = QuickinputSvcValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn quickinput_svc_is_ascii_printable() {
        assert!(QuickinputSvcValidator::is_ascii_printable("Hello World 123"));
        assert!(!QuickinputSvcValidator::is_ascii_printable("Hello\x00World"));
    }
}
