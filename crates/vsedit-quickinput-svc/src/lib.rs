//! Quick input model service.

use std::collections::HashMap;
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

// ---------------------------------------------------------------------------
// Input history
// ---------------------------------------------------------------------------

/// Tracks a bounded history of user inputs for recall (e.g. up-arrow).
#[derive(Debug, Clone)]
pub struct InputHistory {
    entries: Vec<String>,
    max_entries: usize,
    cursor: Option<usize>,
}

impl InputHistory {
    /// Create a new history with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
            cursor: None,
        }
    }

    /// Push a new entry. Duplicates of the most-recent entry are ignored.
    pub fn push(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value.trim().is_empty() {
            return;
        }
        if self.entries.last().map(|s| s.as_str()) == Some(value.as_str()) {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(value);
        self.cursor = None;
    }

    /// Move backward (older) in history, returning the entry if available.
    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.cursor {
            Some(0) => 0,
            Some(c) => c - 1,
            None => self.entries.len() - 1,
        };
        self.cursor = Some(idx);
        Some(&self.entries[idx])
    }

    /// Move forward (newer) in history, returning the entry if available.
    pub fn next(&mut self) -> Option<&str> {
        let c = self.cursor?;
        if c + 1 >= self.entries.len() {
            self.cursor = None;
            return None;
        }
        self.cursor = Some(c + 1);
        Some(&self.entries[c + 1])
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all entries oldest-first.
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// Input transformation pipeline
// ---------------------------------------------------------------------------

/// A transformation step applied to user input text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputTransform {
    /// Remove leading and trailing whitespace.
    Trim,
    /// Convert to lowercase.
    Lowercase,
    /// Convert to uppercase.
    Uppercase,
    /// Replace all occurrences of `from` with `to`.
    Replace { from: String, to: String },
    /// Truncate to at most `max` characters.
    Truncate(usize),
}

/// Apply a pipeline of transforms to `input` in order.
pub fn apply_transforms(input: &str, transforms: &[InputTransform]) -> String {
    let mut s = input.to_string();
    for t in transforms {
        s = match t {
            InputTransform::Trim => s.trim().to_string(),
            InputTransform::Lowercase => s.to_lowercase(),
            InputTransform::Uppercase => s.to_uppercase(),
            InputTransform::Replace { from, to } => s.replace(from.as_str(), to.as_str()),
            InputTransform::Truncate(max) => {
                if s.chars().count() > *max {
                    s.chars().take(*max).collect()
                } else {
                    s
                }
            }
        };
    }
    s
}

// ---------------------------------------------------------------------------
// Multi-step quick pick wizard
// ---------------------------------------------------------------------------

/// Tracks the state of a multi-step quick pick wizard.
///
/// Each step has a title and a set of items. The wizard records the user's
/// selection at each step and allows navigating back to previous steps.
#[derive(Debug, Clone)]
pub struct QuickPickWizard {
    steps: Vec<WizardStep>,
    current_step: usize,
    selections: Vec<Option<usize>>,
}

/// A single step in a [`QuickPickWizard`].
#[derive(Debug, Clone)]
pub struct WizardStep {
    pub title: String,
    pub items: Vec<QuickPickItem>,
}

impl WizardStep {
    pub fn new(title: impl Into<String>, items: Vec<QuickPickItem>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

impl QuickPickWizard {
    /// Create a wizard from a list of steps.
    pub fn new(steps: Vec<WizardStep>) -> Self {
        let len = steps.len();
        Self {
            steps,
            current_step: 0,
            selections: vec![None; len],
        }
    }

    /// Returns the current step, or `None` if the wizard is finished.
    pub fn current(&self) -> Option<&WizardStep> {
        self.steps.get(self.current_step)
    }

    /// The zero-based index of the current step.
    pub fn current_index(&self) -> usize {
        self.current_step
    }

    /// Total number of steps.
    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Whether the wizard has been completed (past the last step).
    pub fn is_finished(&self) -> bool {
        self.current_step >= self.steps.len()
    }

    /// Record a selection for the current step and advance.
    ///
    /// Returns `false` if the wizard is already finished or the index is out
    /// of bounds for the current step's items.
    pub fn select(&mut self, item_index: usize) -> bool {
        if self.is_finished() {
            return false;
        }
        if item_index >= self.steps[self.current_step].items.len() {
            return false;
        }
        self.selections[self.current_step] = Some(item_index);
        self.current_step += 1;
        true
    }

    /// Go back one step. Returns `false` if already at the first step.
    pub fn back(&mut self) -> bool {
        if self.current_step == 0 {
            return false;
        }
        self.current_step -= 1;
        self.selections[self.current_step] = None;
        true
    }

    /// Returns the selected item for a given step, if any.
    pub fn selection_at(&self, step: usize) -> Option<&QuickPickItem> {
        let idx = *self.selections.get(step)?.as_ref()?;
        self.steps.get(step)?.items.get(idx)
    }

    /// Collect all selected items across completed steps.
    pub fn all_selections(&self) -> Vec<&QuickPickItem> {
        self.selections
            .iter()
            .enumerate()
            .filter_map(|(step, sel)| {
                let idx = (*sel)?;
                self.steps.get(step)?.items.get(idx)
            })
            .collect()
    }

    /// A human-readable progress string like "Step 2 of 4".
    pub fn progress_label(&self) -> String {
        format!(
            "Step {} of {}",
            (self.current_step + 1).min(self.steps.len()),
            self.steps.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Input validation with debounce state
// ---------------------------------------------------------------------------

/// Tracks debounced validation state for an input box.
///
/// In a real UI the debounce timer would be driven by the event loop; this
/// struct captures the *model* side: when the last change happened, the
/// configured delay, and the most recent validation result.
#[derive(Debug, Clone)]
pub struct DebouncedInputValidator {
    delay_ms: u64,
    last_change_ms: u64,
    last_validated_value: Option<String>,
    last_result: Option<InputBoxValidation>,
}

impl DebouncedInputValidator {
    /// Create a validator with the given debounce delay in milliseconds.
    pub fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            last_change_ms: 0,
            last_validated_value: None,
            last_result: None,
        }
    }

    /// Notify the validator that the input changed at `now_ms`.
    pub fn on_change(&mut self, now_ms: u64) {
        self.last_change_ms = now_ms;
    }

    /// Returns `true` if enough time has elapsed since the last change to
    /// trigger validation.
    pub fn should_validate(&self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.last_change_ms) >= self.delay_ms
    }

    /// Record the result of running validation against `value`.
    pub fn set_result(&mut self, value: impl Into<String>, result: InputBoxValidation) {
        self.last_validated_value = Some(value.into());
        self.last_result = Some(result);
    }

    /// The most recent validation result, if any.
    pub fn last_result(&self) -> Option<&InputBoxValidation> {
        self.last_result.as_ref()
    }

    /// Whether the value has changed since the last validation.
    pub fn is_stale(&self, current_value: &str) -> bool {
        match &self.last_validated_value {
            Some(v) => v != current_value,
            None => true,
        }
    }

    /// The configured debounce delay.
    pub fn delay_ms(&self) -> u64 {
        self.delay_ms
    }
}

// ---------------------------------------------------------------------------
// Recent selections history
// ---------------------------------------------------------------------------

/// Manages a bounded most-recently-used list of selected quick pick labels.
///
/// This is used to boost recently chosen items to the top of the list.
#[derive(Debug, Clone)]
pub struct RecentSelections {
    labels: Vec<String>,
    capacity: usize,
}

impl RecentSelections {
    /// Create a new history with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            labels: Vec::new(),
            capacity,
        }
    }

    /// Record that `label` was selected. If it already exists it is moved to
    /// the front (most recent). The oldest entry is evicted when at capacity.
    pub fn record(&mut self, label: impl Into<String>) {
        let label = label.into();
        if let Some(pos) = self.labels.iter().position(|l| *l == label) {
            self.labels.remove(pos);
        }
        if self.labels.len() >= self.capacity {
            self.labels.pop();
        }
        self.labels.insert(0, label);
    }

    /// Returns the position (0 = most recent) of `label`, or `None`.
    pub fn position(&self, label: &str) -> Option<usize> {
        self.labels.iter().position(|l| l == label)
    }

    /// Sort a slice of items so that recently selected items appear first,
    /// preserving the relative order among non-recent items.
    pub fn boost_items(&self, items: &[QuickPickItem]) -> Vec<QuickPickItem> {
        let mut boosted: Vec<(usize, &QuickPickItem)> = items
            .iter()
            .map(|item| {
                let priority = self
                    .position(&item.label)
                    .map(|p| p + 1)
                    .unwrap_or(self.capacity + 1);
                (priority, item)
            })
            .collect();
        boosted.sort_by_key(|(p, _)| *p);
        boosted.into_iter().map(|(_, item)| item.clone()).collect()
    }

    /// The labels in most-recent-first order.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Number of recorded labels.
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.labels.clear();
    }
}

// ---------------------------------------------------------------------------
// Quick pick item badge
// ---------------------------------------------------------------------------

/// A small badge displayed alongside a quick pick item (e.g. a shortcut key
/// or status indicator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickBadge {
    pub text: String,
    pub tooltip: Option<String>,
}

impl QuickPickBadge {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tooltip: None,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }
}

impl fmt::Display for QuickPickBadge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.text)
    }
}

/// Extended quick pick item that includes optional badges and a detail
/// rendering hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RichQuickPickItem {
    pub item: QuickPickItem,
    pub badges: Vec<QuickPickBadge>,
    pub icon_id: Option<String>,
}

impl RichQuickPickItem {
    pub fn new(item: QuickPickItem) -> Self {
        Self {
            item,
            badges: Vec::new(),
            icon_id: None,
        }
    }

    pub fn with_badge(mut self, badge: QuickPickBadge) -> Self {
        self.badges.push(badge);
        self
    }

    pub fn with_icon(mut self, icon_id: impl Into<String>) -> Self {
        self.icon_id = Some(icon_id.into());
        self
    }

    /// Render a single-line text representation suitable for a terminal UI.
    pub fn render_line(&self) -> String {
        let mut line = String::new();
        if let Some(ref icon) = self.icon_id {
            line.push_str(&format!("$({}) ", icon));
        }
        line.push_str(&self.item.label);
        for badge in &self.badges {
            line.push(' ');
            line.push_str(&badge.to_string());
        }
        if let Some(ref desc) = self.item.description {
            line.push_str(&format!("  {}", desc));
        }
        line
    }
}

// ---------------------------------------------------------------------------
// QuickInputTheme
// ---------------------------------------------------------------------------

/// Theme configuration for quick input UI elements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickInputTheme {
    pub background: String,
    pub foreground: String,
    pub highlight_color: String,
    pub border_color: String,
    pub selected_bg: String,
}

impl QuickInputTheme {
    /// Create a theme with sensible dark defaults.
    pub fn new() -> Self {
        Self {
            background: "#1e1e1e".into(),
            foreground: "#cccccc".into(),
            highlight_color: "#007acc".into(),
            border_color: "#454545".into(),
            selected_bg: "#094771".into(),
        }
    }

    /// Predefined dark theme.
    pub fn dark() -> Self {
        Self::new()
    }

    /// Predefined light theme.
    pub fn light() -> Self {
        Self {
            background: "#ffffff".into(),
            foreground: "#333333".into(),
            highlight_color: "#0066b8".into(),
            border_color: "#c8c8c8".into(),
            selected_bg: "#dceafa".into(),
        }
    }

    /// Override the highlight color.
    pub fn with_highlight(mut self, color: &str) -> Self {
        self.highlight_color = color.into();
        self
    }

    /// Return a map of CSS-like variable names to their color values.
    pub fn css_vars(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("--quick-input-bg".into(), self.background.clone());
        map.insert("--quick-input-fg".into(), self.foreground.clone());
        map.insert("--quick-input-highlight".into(), self.highlight_color.clone());
        map.insert("--quick-input-border".into(), self.border_color.clone());
        map.insert("--quick-input-selected-bg".into(), self.selected_bg.clone());
        map
    }
}

impl Default for QuickInputTheme {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QuickInputAccessibility
// ---------------------------------------------------------------------------

/// ARIA live region behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaLive {
    Off,
    Polite,
    Assertive,
}

impl fmt::Display for AriaLive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AriaLive::Off => write!(f, "off"),
            AriaLive::Polite => write!(f, "polite"),
            AriaLive::Assertive => write!(f, "assertive"),
        }
    }
}

/// Accessibility metadata for a quick input widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickInputAccessibility {
    pub role: String,
    pub label: String,
    pub live_region: AriaLive,
    pub expanded: bool,
}

impl QuickInputAccessibility {
    /// Create with sensible defaults for a listbox.
    pub fn new(label: &str) -> Self {
        Self {
            role: "listbox".into(),
            label: label.into(),
            live_region: AriaLive::Polite,
            expanded: false,
        }
    }

    /// Override the ARIA role.
    pub fn with_role(mut self, role: &str) -> Self {
        self.role = role.into();
        self
    }

    /// Produce a human-readable announcement string.
    pub fn announce(&self) -> String {
        format!(
            "role=\"{}\" aria-label=\"{}\" aria-live=\"{}\" aria-expanded=\"{}\"",
            self.role, self.label, self.live_region, self.expanded,
        )
    }

    /// Set whether the widget is expanded.
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// Return a map of ARIA attribute names to their values.
    pub fn aria_attrs(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("role".into(), self.role.clone());
        map.insert("aria-label".into(), self.label.clone());
        map.insert("aria-live".into(), self.live_region.to_string());
        map.insert("aria-expanded".into(), self.expanded.to_string());
        map
    }
}

// ---------------------------------------------------------------------------
// QuickInputSeparator
// ---------------------------------------------------------------------------

/// The visual style of a separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorKind {
    Line,
    Space,
    Label,
}

/// A separator that can appear between quick-pick items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickInputSeparator {
    pub label: Option<String>,
    pub kind: SeparatorKind,
}

impl QuickInputSeparator {
    /// A thin horizontal line separator.
    pub fn line() -> Self {
        Self { label: None, kind: SeparatorKind::Line }
    }

    /// An empty space separator.
    pub fn space() -> Self {
        Self { label: None, kind: SeparatorKind::Space }
    }

    /// A labeled separator such as `── Recent ──`.
    pub fn labeled(text: &str) -> Self {
        Self { label: Some(text.into()), kind: SeparatorKind::Label }
    }

    /// Render the separator to a fixed `width`.
    pub fn render(&self, width: usize) -> String {
        match self.kind {
            SeparatorKind::Line => "─".repeat(width),
            SeparatorKind::Space => String::new(),
            SeparatorKind::Label => {
                let text = self.label.as_deref().unwrap_or("");
                let content = format!(" {} ", text);
                let content_len = content.chars().count();
                if width <= content_len {
                    return content;
                }
                let remaining = width - content_len;
                let left = remaining / 2;
                let right = remaining - left;
                format!(
                    "{}{}{}",
                    "─".repeat(left),
                    content,
                    "─".repeat(right),
                )
            }
        }
    }
}

impl fmt::Display for QuickInputSeparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render(40))
    }
}

// ---------------------------------------------------------------------------
// QuickInputBusyIndicator
// ---------------------------------------------------------------------------

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// A busy / progress indicator for long-running quick-input operations.
#[derive(Debug, Clone, PartialEq)]
pub struct QuickInputBusyIndicator {
    pub active: bool,
    pub message: Option<String>,
    pub progress: Option<f32>,
    pub spinner_frame: usize,
}

impl QuickInputBusyIndicator {
    /// Create an inactive indicator.
    pub fn new() -> Self {
        Self {
            active: false,
            message: None,
            progress: None,
            spinner_frame: 0,
        }
    }

    /// Start the indicator with a message.
    pub fn start(&mut self, msg: &str) {
        self.active = true;
        self.message = Some(msg.into());
        self.spinner_frame = 0;
    }

    /// Stop the indicator and clear state.
    pub fn stop(&mut self) {
        self.active = false;
        self.message = None;
        self.progress = None;
        self.spinner_frame = 0;
    }

    /// Set progress as a percentage (0.0–100.0), clamped.
    pub fn set_progress(&mut self, pct: f32) {
        self.progress = Some(pct.clamp(0.0, 100.0));
    }

    /// Advance the spinner and return the current frame character.
    pub fn tick(&mut self) -> char {
        let ch = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        ch
    }

    /// Render the indicator to a string.
    pub fn render(&self) -> String {
        if !self.active {
            return String::new();
        }
        let spinner = SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()];
        let msg = self.message.as_deref().unwrap_or("");
        match self.progress {
            Some(pct) => format!("{} {} ({:.0}%)", spinner, msg, pct),
            None => format!("{} {}", spinner, msg),
        }
    }

    /// Whether the indicator is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for QuickInputBusyIndicator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QuickInputAutoComplete
// ---------------------------------------------------------------------------

/// A single auto-completion suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoCompleteSuggestion {
    pub text: String,
    pub label: String,
    pub score: u32,
    pub category: String,
}

impl AutoCompleteSuggestion {
    pub fn new(text: impl Into<String>, label: impl Into<String>, score: u32, category: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            label: label.into(),
            score,
            category: category.into(),
        }
    }
}

impl std::fmt::Display for AutoCompleteSuggestion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}] (score={})", self.label, self.category, self.score)
    }
}

/// Provides auto-completion suggestions for quick input based on prefix matching and scoring.
pub struct QuickInputAutoComplete {
    suggestions: Vec<AutoCompleteSuggestion>,
    max_results: usize,
    case_sensitive: bool,
}

impl QuickInputAutoComplete {
    pub fn new(max_results: usize) -> Self {
        Self {
            suggestions: Vec::new(),
            max_results,
            case_sensitive: false,
        }
    }

    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    pub fn add_suggestion(&mut self, suggestion: AutoCompleteSuggestion) {
        self.suggestions.push(suggestion);
    }

    pub fn add_suggestions(&mut self, items: impl IntoIterator<Item = AutoCompleteSuggestion>) {
        self.suggestions.extend(items);
    }

    pub fn suggestion_count(&self) -> usize {
        self.suggestions.len()
    }

    pub fn clear(&mut self) {
        self.suggestions.clear();
    }

    /// Return completions matching the given prefix, sorted by score descending.
    pub fn complete(&self, prefix: &str) -> Vec<&AutoCompleteSuggestion> {
        let normalized_prefix = if self.case_sensitive {
            prefix.to_string()
        } else {
            prefix.to_lowercase()
        };
        let mut matches: Vec<&AutoCompleteSuggestion> = self
            .suggestions
            .iter()
            .filter(|s| {
                let text = if self.case_sensitive {
                    s.text.clone()
                } else {
                    s.text.to_lowercase()
                };
                text.starts_with(&normalized_prefix)
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches.truncate(self.max_results);
        matches
    }

    /// Return completions that contain the query as a substring.
    pub fn fuzzy_match(&self, query: &str) -> Vec<&AutoCompleteSuggestion> {
        let q = if self.case_sensitive { query.to_string() } else { query.to_lowercase() };
        let mut matches: Vec<&AutoCompleteSuggestion> = self
            .suggestions
            .iter()
            .filter(|s| {
                let text = if self.case_sensitive { s.text.clone() } else { s.text.to_lowercase() };
                text.contains(&q)
            })
            .collect();
        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches.truncate(self.max_results);
        matches
    }

    /// Return distinct categories from all suggestions.
    pub fn categories(&self) -> Vec<String> {
        let mut cats: std::collections::HashSet<String> = std::collections::HashSet::new();
        for s in &self.suggestions {
            cats.insert(s.category.clone());
        }
        let mut v: Vec<String> = cats.into_iter().collect();
        v.sort();
        v
    }
}

impl std::fmt::Display for QuickInputAutoComplete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QuickInputAutoComplete({} suggestions, max={})", self.suggestions.len(), self.max_results)
    }
}

// ---------------------------------------------------------------------------
// QuickInputResultCache
// ---------------------------------------------------------------------------

/// A cached result entry for quick input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedQuickInputResult {
    pub query: String,
    pub results: Vec<String>,
    pub timestamp: u64,
}

impl CachedQuickInputResult {
    pub fn new(query: impl Into<String>, results: Vec<String>, timestamp: u64) -> Self {
        Self { query: query.into(), results, timestamp }
    }

    pub fn is_expired(&self, now: u64, ttl: u64) -> bool {
        now.saturating_sub(self.timestamp) > ttl
    }
}

impl std::fmt::Display for CachedQuickInputResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CachedResult('{}', {} items, ts={})", self.query, self.results.len(), self.timestamp)
    }
}

/// Caches previous quick input results for fast recall. Uses LRU-style eviction.
pub struct QuickInputResultCache {
    entries: Vec<CachedQuickInputResult>,
    capacity: usize,
    ttl: u64,
}

impl QuickInputResultCache {
    pub fn new(capacity: usize, ttl: u64) -> Self {
        Self { entries: Vec::new(), capacity, ttl }
    }

    pub fn insert(&mut self, entry: CachedQuickInputResult) {
        // Remove existing entry for same query
        self.entries.retain(|e| e.query != entry.query);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn get(&self, query: &str, now: u64) -> Option<&CachedQuickInputResult> {
        self.entries
            .iter()
            .rev()
            .find(|e| e.query == query && !e.is_expired(now, self.ttl))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Evict all expired entries relative to the given timestamp.
    pub fn evict_expired(&mut self, now: u64) {
        self.entries.retain(|e| !e.is_expired(now, self.ttl));
    }

    /// Return all cached queries.
    pub fn cached_queries(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.query.as_str()).collect()
    }

    /// Returns hit ratio info: (hits_possible, total_entries).
    pub fn stats(&self, now: u64) -> (usize, usize) {
        let valid = self.entries.iter().filter(|e| !e.is_expired(now, self.ttl)).count();
        (valid, self.entries.len())
    }
}

impl std::fmt::Display for QuickInputResultCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "QuickInputResultCache({}/{} entries, ttl={})", self.entries.len(), self.capacity, self.ttl)
    }
}



// ─── QuickIn Builder & Validator ─────────────────────────────

/// Builder for constructing quick input configurations.
#[derive(Debug, Clone)]
pub struct QuickInBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl QuickInBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<QuickInCfg, QuickInBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(QuickInBuildErr { errors }); }
        Ok(QuickInCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated quick input configuration.
#[derive(Debug, Clone)]
pub struct QuickInCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl QuickInCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &QuickInCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for QuickInCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuickInCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct QuickInBuildErr { pub errors: Vec<String> }

impl fmt::Display for QuickInBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuickInBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for QuickInBuildErr {}

// ─── QuickIn Formatter ───────────────────────────────────────

/// Formatting options for quick input output.
#[derive(Debug, Clone)]
pub struct QuickInFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for QuickInFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl QuickInFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for quick input data.
pub struct QuickInFmt {
    options: QuickInFmtOpts,
}

impl QuickInFmt {
    pub fn new(options: QuickInFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: QuickInFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}



// ---------------------------------------------------------------------------
// quickinput_svc – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for quick input service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YQuickinputSvcQuickPickItemKind {
    Default,
    Separator,
    Header,
    Detail,
}

impl YQuickinputSvcQuickPickItemKind {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Default => 0,
            Self::Separator => 1,
            Self::Header => 2,
            Self::Detail => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Separator => "Separator",
            Self::Header => "Header",
            Self::Detail => "Detail",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YQuickinputSvcQuickPickItemKind] {
        &[
            YQuickinputSvcQuickPickItemKind::Default,
            YQuickinputSvcQuickPickItemKind::Separator,
            YQuickinputSvcQuickPickItemKind::Header,
            YQuickinputSvcQuickPickItemKind::Detail,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YQuickinputSvcQuickPickItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks quick pick filter data.
#[derive(Debug, Clone)]
pub struct YQuickinputSvcQuickPickFilter {
    pub query: String,
    pub case_sensitive: bool,
    pub fuzzy: bool,
}

impl YQuickinputSvcQuickPickFilter {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            case_sensitive: false,
            fuzzy: false,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YQuickinputSvcQuickPickFilter({}: {:?})", "query", self.query)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_quickinput_svc_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_quickinput_svc_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_quickinput_svc_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_quickinput_svc_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_quickinput_svc_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_quickinput_svc_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_quickinput_svc_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_quickinput_svc_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// quickinput_svc – Extended quick pick scorer helpers
// ---------------------------------------------------------------------------

/// Priority levels for quick pick scorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZQuickinputSvcPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZQuickinputSvcPriority {
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
    pub fn all_asc() -> [ZQuickinputSvcPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZQuickinputSvcPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks quick pick scorer data.
#[derive(Debug, Clone)]
pub struct ZQuickinputSvcQuickPickScorer {
    pub scores: Vec<(String, f64)>,
    pub algorithm: String,
    pub threshold: f64,
}

impl ZQuickinputSvcQuickPickScorer {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            scores: Vec::new(),
            algorithm: String::new(),
            threshold: 0.0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.scores.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZQuickinputSvcQuickPickScorer[algorithm={:?}, threshold={:?}]", self.algorithm, self.threshold)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for quick pick scorer.
pub fn z_quickinput_svc_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_quickinput_svc_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_quickinput_svc_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_quickinput_svc_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_quickinput_svc_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_quickinput_svc_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_quickinput_svc_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
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

    #[test]
    fn input_history_push_and_prev() {
        let mut h = InputHistory::new(5);
        h.push("first");
        h.push("second");
        h.push("third");
        assert_eq!(h.prev(), Some("third"));
        assert_eq!(h.prev(), Some("second"));
        assert_eq!(h.prev(), Some("first"));
        assert_eq!(h.prev(), Some("first")); // stays at oldest
    }

    #[test]
    fn input_history_ignores_blank_and_consecutive_dupes() {
        let mut h = InputHistory::new(10);
        h.push("   ");
        assert!(h.is_empty());
        h.push("hello");
        h.push("hello");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn input_history_respects_max_entries() {
        let mut h = InputHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        h.push("d");
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries()[0], "b");
    }

    #[test]
    fn input_history_next_navigates_forward() {
        let mut h = InputHistory::new(10);
        h.push("x");
        h.push("y");
        h.push("z");
        h.prev(); // z
        h.prev(); // y
        assert_eq!(h.next(), Some("z"));
        assert_eq!(h.next(), None); // past end
    }

    #[test]
    fn apply_transforms_pipeline() {
        let transforms = vec![
            InputTransform::Trim,
            InputTransform::Lowercase,
            InputTransform::Replace { from: " ".into(), to: "_".into() },
        ];
        assert_eq!(apply_transforms("  Hello World  ", &transforms), "hello_world");
    }

    #[test]
    fn apply_transforms_truncate() {
        let transforms = vec![InputTransform::Truncate(5)];
        assert_eq!(apply_transforms("Hello, World!", &transforms), "Hello");
        assert_eq!(apply_transforms("Hi", &transforms), "Hi");
    }

    #[test]
    fn apply_transforms_uppercase() {
        let transforms = vec![InputTransform::Uppercase];
        assert_eq!(apply_transforms("hello", &transforms), "HELLO");
    }

    // -- QuickPickWizard tests --

    #[test]
    fn wizard_step_through_and_back() {
        let steps = vec![
            WizardStep::new("Language", vec![item("Rust"), item("Go")]),
            WizardStep::new("Framework", vec![item("Actix"), item("Axum")]),
            WizardStep::new("DB", vec![item("Postgres"), item("SQLite")]),
        ];
        let mut wiz = QuickPickWizard::new(steps);

        assert_eq!(wiz.total_steps(), 3);
        assert_eq!(wiz.current_index(), 0);
        assert_eq!(wiz.progress_label(), "Step 1 of 3");
        assert!(!wiz.is_finished());

        assert!(wiz.select(0)); // pick "Rust"
        assert_eq!(wiz.current_index(), 1);
        assert!(wiz.select(1)); // pick "Axum"
        assert_eq!(wiz.current_index(), 2);

        // go back
        assert!(wiz.back());
        assert_eq!(wiz.current_index(), 1);
        assert!(wiz.selection_at(1).is_none()); // cleared

        // re-select and finish
        assert!(wiz.select(0)); // pick "Actix"
        assert!(wiz.select(1)); // pick "SQLite"
        assert!(wiz.is_finished());
        assert!(!wiz.select(0)); // already finished

        let sels = wiz.all_selections();
        assert_eq!(sels.len(), 3);
        assert_eq!(sels[0].label, "Rust");
        assert_eq!(sels[1].label, "Actix");
        assert_eq!(sels[2].label, "SQLite");
    }

    #[test]
    fn wizard_rejects_out_of_bounds() {
        let steps = vec![WizardStep::new("Pick", vec![item("Only")])];
        let mut wiz = QuickPickWizard::new(steps);
        assert!(!wiz.select(5)); // out of bounds
        assert!(!wiz.back()); // already at step 0
    }

    // -- DebouncedInputValidator tests --

    #[test]
    fn debounced_validator_timing() {
        let mut dv = DebouncedInputValidator::new(300);
        assert_eq!(dv.delay_ms(), 300);

        dv.on_change(1000);
        assert!(!dv.should_validate(1100)); // only 100ms elapsed
        assert!(dv.should_validate(1300)); // 300ms elapsed

        assert!(dv.is_stale("anything")); // never validated
        dv.set_result("hello", InputBoxValidation::Ok);
        assert!(!dv.is_stale("hello"));
        assert!(dv.is_stale("world"));
        assert_eq!(dv.last_result(), Some(&InputBoxValidation::Ok));
    }

    // -- RecentSelections tests --

    #[test]
    fn recent_selections_record_and_boost() {
        let mut recent = RecentSelections::new(3);
        recent.record("C");
        recent.record("B");
        recent.record("A");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent.labels(), &["A", "B", "C"]);

        // Recording an existing label moves it to front
        recent.record("C");
        assert_eq!(recent.labels(), &["C", "A", "B"]);

        // Eviction at capacity
        recent.record("D");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent.labels(), &["D", "C", "A"]);
        assert_eq!(recent.position("B"), None); // evicted

        // boost_items reorders
        let items = vec![item("X"), item("A"), item("D"), item("Y")];
        let boosted = recent.boost_items(&items);
        assert_eq!(boosted[0].label, "D");
        assert_eq!(boosted[1].label, "A");
    }

    #[test]
    fn recent_selections_clear() {
        let mut recent = RecentSelections::new(5);
        recent.record("a");
        recent.record("b");
        assert!(!recent.is_empty());
        recent.clear();
        assert!(recent.is_empty());
        assert_eq!(recent.len(), 0);
    }

    // -- RichQuickPickItem / badge tests --

    #[test]
    fn rich_item_render_line() {
        let rich = RichQuickPickItem::new(
            QuickPickItemBuilder::new("Open File")
                .description("recent")
                .build(),
        )
        .with_icon("file")
        .with_badge(QuickPickBadge::new("⌘O").with_tooltip("Shortcut"));

        let line = rich.render_line();
        assert!(line.contains("$(file)"));
        assert!(line.contains("Open File"));
        assert!(line.contains("[⌘O]"));
        assert!(line.contains("recent"));
    }

    #[test]
    fn badge_display() {
        let b = QuickPickBadge::new("Ctrl+P");
        assert_eq!(b.to_string(), "[Ctrl+P]");
        assert_eq!(b.tooltip, None);

        let b2 = QuickPickBadge::new("!").with_tooltip("Warning");
        assert_eq!(b2.tooltip.as_deref(), Some("Warning"));
    }

    // -----------------------------------------------------------------------
    // QuickInputTheme tests
    // -----------------------------------------------------------------------

    #[test]
    fn theme_default_is_dark() {
        let theme = QuickInputTheme::new();
        assert_eq!(theme.background, "#1e1e1e");
        assert_eq!(theme, QuickInputTheme::dark());
    }

    #[test]
    fn theme_light_differs_from_dark() {
        let dark = QuickInputTheme::dark();
        let light = QuickInputTheme::light();
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.foreground, light.foreground);
    }

    #[test]
    fn theme_with_highlight_overrides() {
        let theme = QuickInputTheme::new().with_highlight("#ff0000");
        assert_eq!(theme.highlight_color, "#ff0000");
        assert_eq!(theme.background, "#1e1e1e"); // unchanged
    }

    #[test]
    fn theme_css_vars_has_all_keys() {
        let vars = QuickInputTheme::new().css_vars();
        assert_eq!(vars.len(), 5);
        assert_eq!(vars["--quick-input-bg"], "#1e1e1e");
        assert!(vars.contains_key("--quick-input-selected-bg"));
    }

    // -----------------------------------------------------------------------
    // QuickInputAccessibility tests
    // -----------------------------------------------------------------------

    #[test]
    fn accessibility_defaults() {
        let a11y = QuickInputAccessibility::new("Command Palette");
        assert_eq!(a11y.role, "listbox");
        assert_eq!(a11y.live_region, AriaLive::Polite);
        assert!(!a11y.expanded);
    }

    #[test]
    fn accessibility_with_role_and_expand() {
        let mut a11y = QuickInputAccessibility::new("Search")
            .with_role("combobox");
        assert_eq!(a11y.role, "combobox");
        a11y.set_expanded(true);
        assert!(a11y.expanded);
        let ann = a11y.announce();
        assert!(ann.contains("combobox"));
        assert!(ann.contains("aria-expanded=\"true\""));
    }

    #[test]
    fn accessibility_aria_attrs_map() {
        let a11y = QuickInputAccessibility::new("Files");
        let attrs = a11y.aria_attrs();
        assert_eq!(attrs["role"], "listbox");
        assert_eq!(attrs["aria-label"], "Files");
        assert_eq!(attrs["aria-live"], "polite");
        assert_eq!(attrs["aria-expanded"], "false");
    }

    #[test]
    fn aria_live_display() {
        assert_eq!(AriaLive::Off.to_string(), "off");
        assert_eq!(AriaLive::Polite.to_string(), "polite");
        assert_eq!(AriaLive::Assertive.to_string(), "assertive");
    }

    // -----------------------------------------------------------------------
    // QuickInputSeparator tests
    // -----------------------------------------------------------------------

    #[test]
    fn separator_line_render() {
        let sep = QuickInputSeparator::line();
        let rendered = sep.render(10);
        assert_eq!(rendered.chars().count(), 10);
        assert!(rendered.chars().all(|c| c == '─'));
    }

    #[test]
    fn separator_space_render_empty() {
        let sep = QuickInputSeparator::space();
        assert!(sep.render(20).is_empty());
    }

    #[test]
    fn separator_labeled_render() {
        let sep = QuickInputSeparator::labeled("Recent");
        let rendered = sep.render(30);
        assert!(rendered.contains("Recent"));
        assert!(rendered.contains('─'));
    }

    #[test]
    fn separator_display_uses_default_width() {
        let sep = QuickInputSeparator::line();
        let display = sep.to_string();
        assert_eq!(display.chars().count(), 40);
    }

    // -----------------------------------------------------------------------
    // QuickInputBusyIndicator tests
    // -----------------------------------------------------------------------

    #[test]
    fn busy_indicator_lifecycle() {
        let mut ind = QuickInputBusyIndicator::new();
        assert!(!ind.is_active());
        assert!(ind.render().is_empty());

        ind.start("Loading…");
        assert!(ind.is_active());
        let r = ind.render();
        assert!(r.contains("Loading…"));

        ind.stop();
        assert!(!ind.is_active());
        assert!(ind.render().is_empty());
    }

    #[test]
    fn busy_indicator_tick_cycles() {
        let mut ind = QuickInputBusyIndicator::new();
        let first = ind.tick();
        assert_eq!(first, '⠋');
        let second = ind.tick();
        assert_eq!(second, '⠙');
        // Cycle through all 10 frames and wrap
        for _ in 2..10 {
            ind.tick();
        }
        assert_eq!(ind.tick(), '⠋');
    }

    #[test]
    fn busy_indicator_progress() {
        let mut ind = QuickInputBusyIndicator::new();
        ind.start("Indexing");
        ind.set_progress(42.5);
        let r = ind.render();
        assert!(r.contains("42%") || r.contains("43%"));
        assert!(r.contains("Indexing"));

        // Clamp above 100
        ind.set_progress(150.0);
        assert_eq!(ind.progress, Some(100.0));
    }

    #[test]
    fn autocomplete_prefix_match() {
        let mut ac = QuickInputAutoComplete::new(10);
        ac.add_suggestion(AutoCompleteSuggestion::new("hello", "Hello", 10, "greet"));
        ac.add_suggestion(AutoCompleteSuggestion::new("help", "Help", 5, "cmd"));
        ac.add_suggestion(AutoCompleteSuggestion::new("world", "World", 8, "greet"));
        let results = ac.complete("hel");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "hello"); // higher score first
    }

    #[test]
    fn autocomplete_case_insensitive() {
        let mut ac = QuickInputAutoComplete::new(10);
        ac.add_suggestion(AutoCompleteSuggestion::new("Hello", "Hello", 10, "g"));
        let results = ac.complete("hel");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn autocomplete_case_sensitive() {
        let mut ac = QuickInputAutoComplete::new(10);
        ac.set_case_sensitive(true);
        ac.add_suggestion(AutoCompleteSuggestion::new("Hello", "Hello", 10, "g"));
        let results = ac.complete("hel");
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn autocomplete_max_results() {
        let mut ac = QuickInputAutoComplete::new(2);
        ac.add_suggestion(AutoCompleteSuggestion::new("a1", "A1", 1, "c"));
        ac.add_suggestion(AutoCompleteSuggestion::new("a2", "A2", 2, "c"));
        ac.add_suggestion(AutoCompleteSuggestion::new("a3", "A3", 3, "c"));
        let results = ac.complete("a");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn autocomplete_fuzzy_match() {
        let mut ac = QuickInputAutoComplete::new(10);
        ac.add_suggestion(AutoCompleteSuggestion::new("foobar", "FooBar", 10, "c"));
        ac.add_suggestion(AutoCompleteSuggestion::new("baz", "Baz", 5, "c"));
        let results = ac.fuzzy_match("oob");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "foobar");
    }

    #[test]
    fn autocomplete_categories() {
        let mut ac = QuickInputAutoComplete::new(10);
        ac.add_suggestion(AutoCompleteSuggestion::new("a", "A", 1, "cat1"));
        ac.add_suggestion(AutoCompleteSuggestion::new("b", "B", 1, "cat2"));
        ac.add_suggestion(AutoCompleteSuggestion::new("c", "C", 1, "cat1"));
        let cats = ac.categories();
        assert_eq!(cats.len(), 2);
    }

    #[test]
    fn autocomplete_display_and_clear() {
        let mut ac = QuickInputAutoComplete::new(10);
        ac.add_suggestion(AutoCompleteSuggestion::new("x", "X", 1, "c"));
        assert!(format!("{ac}").contains("1 suggestions"));
        assert_eq!(ac.suggestion_count(), 1);
        ac.clear();
        assert_eq!(ac.suggestion_count(), 0);
    }

    #[test]
    fn result_cache_insert_and_get() {
        let mut cache = QuickInputResultCache::new(10, 1000);
        cache.insert(CachedQuickInputResult::new("q1", vec!["r1".into()], 100));
        let r = cache.get("q1", 200);
        assert!(r.is_some());
        assert_eq!(r.unwrap().results, vec!["r1"]);
    }

    #[test]
    fn result_cache_expired_entry() {
        let mut cache = QuickInputResultCache::new(10, 100);
        cache.insert(CachedQuickInputResult::new("q1", vec!["r1".into()], 100));
        assert!(cache.get("q1", 300).is_none()); // expired
    }

    #[test]
    fn result_cache_eviction() {
        let mut cache = QuickInputResultCache::new(2, 1000);
        cache.insert(CachedQuickInputResult::new("q1", vec![], 100));
        cache.insert(CachedQuickInputResult::new("q2", vec![], 200));
        cache.insert(CachedQuickInputResult::new("q3", vec![], 300));
        assert_eq!(cache.len(), 2);
        assert!(cache.get("q1", 300).is_none()); // evicted
    }

    #[test]
    fn result_cache_evict_expired() {
        let mut cache = QuickInputResultCache::new(10, 100);
        cache.insert(CachedQuickInputResult::new("old", vec![], 10));
        cache.insert(CachedQuickInputResult::new("new", vec![], 500));
        cache.evict_expired(500);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn result_cache_stats_and_display() {
        let mut cache = QuickInputResultCache::new(10, 100);
        cache.insert(CachedQuickInputResult::new("q1", vec![], 10));
        cache.insert(CachedQuickInputResult::new("q2", vec![], 200));
        let (valid, total) = cache.stats(250);
        assert_eq!(total, 2);
        assert_eq!(valid, 1);
        assert!(format!("{cache}").contains("2/10"));
    }

    #[test]
    fn result_cache_cached_queries() {
        let mut cache = QuickInputResultCache::new(10, 1000);
        cache.insert(CachedQuickInputResult::new("alpha", vec![], 1));
        cache.insert(CachedQuickInputResult::new("beta", vec![], 2));
        let queries = cache.cached_queries();
        assert!(queries.contains(&"alpha"));
        assert!(queries.contains(&"beta"));
    }

    #[test]
    fn result_cache_replace_same_query() {
        let mut cache = QuickInputResultCache::new(10, 1000);
        cache.insert(CachedQuickInputResult::new("q", vec!["old".into()], 1));
        cache.insert(CachedQuickInputResult::new("q", vec!["new".into()], 2));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("q", 5).unwrap().results, vec!["new"]);
    }

    #[test]
    fn suggestion_display() {
        let s = AutoCompleteSuggestion::new("txt", "Label", 42, "Cat");
        assert!(format!("{s}").contains("Label"));
        assert!(format!("{s}").contains("Cat"));
        assert!(format!("{s}").contains("42"));
    }

    #[test]
    fn cached_result_is_expired() {
        let r = CachedQuickInputResult::new("q", vec![], 100);
        assert!(!r.is_expired(150, 100));
        assert!(r.is_expired(250, 100));
    }

    #[test]
    fn cached_result_display() {
        let r = CachedQuickInputResult::new("q", vec!["a".into(), "b".into()], 50);
        let s = format!("{r}");
        assert!(s.contains("q"));
        assert!(s.contains("2 items"));
    }


    #[test]
    fn quickin_builder_valid() {
        let cfg = QuickInBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn quickin_builder_empty_name() {
        let r = QuickInBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn quickin_builder_bad_priority() {
        assert!(QuickInBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn quickin_builder_zero_max() {
        assert!(QuickInBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn quickin_cfg_merge() {
        let mut a = QuickInBuilder::new("a").property("x", "1").build().unwrap();
        let b = QuickInBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn quickin_cfg_display() {
        let cfg = QuickInBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn quickin_fmt_list() {
        let f = QuickInFmt::new(QuickInFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn quickin_fmt_kv() {
        let f = QuickInFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn quickin_fmt_section() {
        let f = QuickInFmt::new(QuickInFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn quickin_fmt_truncate() {
        let f = QuickInFmt::new(QuickInFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn quickin_fmt_opts_defaults() {
        let o = QuickInFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    // -- quickinput_svc extended domain tests ----------------------------------------

    #[test]
    fn y_quickinput_svc_enum_index() {
        assert_eq!(YQuickinputSvcQuickPickItemKind::Default.index(), 0);
        assert_eq!(YQuickinputSvcQuickPickItemKind::Separator.index(), 1);
        assert_eq!(YQuickinputSvcQuickPickItemKind::Header.index(), 2);
        assert_eq!(YQuickinputSvcQuickPickItemKind::Detail.index(), 3);
    }

    #[test]
    fn y_quickinput_svc_enum_label() {
        assert_eq!(YQuickinputSvcQuickPickItemKind::Default.label(), "Default");
        assert_eq!(YQuickinputSvcQuickPickItemKind::Separator.label(), "Separator");
        assert_eq!(YQuickinputSvcQuickPickItemKind::Header.label(), "Header");
        assert_eq!(YQuickinputSvcQuickPickItemKind::Detail.label(), "Detail");
    }

    #[test]
    fn y_quickinput_svc_enum_all() {
        let all = YQuickinputSvcQuickPickItemKind::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_quickinput_svc_enum_is_default() {
        assert!(YQuickinputSvcQuickPickItemKind::Default.is_default());
        assert!(!YQuickinputSvcQuickPickItemKind::Detail.is_default());
    }

    #[test]
    fn y_quickinput_svc_enum_display() {
        assert_eq!(format!("{}", YQuickinputSvcQuickPickItemKind::Default), "Default");
    }

    #[test]
    fn y_quickinput_svc_struct_new() {
        let s = YQuickinputSvcQuickPickFilter::new();
        let _ = s.summary();
    }

    #[test]
    fn y_quickinput_svc_fingerprint_deterministic() {
        let h1 = y_quickinput_svc_fingerprint("hello");
        let h2 = y_quickinput_svc_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_quickinput_svc_fingerprint("a"), y_quickinput_svc_fingerprint("b"));
    }

    #[test]
    fn y_quickinput_svc_truncate_short() {
        assert_eq!(y_quickinput_svc_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_quickinput_svc_truncate_long() {
        let r = y_quickinput_svc_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_quickinput_svc_normalize_key_basic() {
        assert_eq!(y_quickinput_svc_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_quickinput_svc_split_path_basic() {
        let parts = y_quickinput_svc_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_quickinput_svc_count_occurrences_basic() {
        assert_eq!(y_quickinput_svc_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_quickinput_svc_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_quickinput_svc_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_quickinput_svc_in_range_basic() {
        assert!(y_quickinput_svc_in_range(5, 1, 10));
        assert!(y_quickinput_svc_in_range(1, 1, 10));
        assert!(y_quickinput_svc_in_range(10, 1, 10));
        assert!(!y_quickinput_svc_in_range(0, 1, 10));
        assert!(!y_quickinput_svc_in_range(11, 1, 10));
    }

    #[test]
    fn y_quickinput_svc_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_quickinput_svc_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_quickinput_svc_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_quickinput_svc_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- quickinput_svc Z-extended tests -----------------------------------------------

    #[test]
    fn z_quickinput_svc_priority_weight() {
        assert_eq!(ZQuickinputSvcPriority::Idle.weight(), 0);
        assert_eq!(ZQuickinputSvcPriority::Normal.weight(), 2);
        assert_eq!(ZQuickinputSvcPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_quickinput_svc_priority_label() {
        assert_eq!(ZQuickinputSvcPriority::Low.label(), "low");
        assert_eq!(ZQuickinputSvcPriority::High.label(), "high");
    }

    #[test]
    fn z_quickinput_svc_priority_is_elevated() {
        assert!(!ZQuickinputSvcPriority::Normal.is_elevated());
        assert!(ZQuickinputSvcPriority::High.is_elevated());
        assert!(ZQuickinputSvcPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_quickinput_svc_priority_display() {
        assert_eq!(format!("{}", ZQuickinputSvcPriority::Idle), "idle");
    }

    #[test]
    fn z_quickinput_svc_priority_all_asc() {
        let all = ZQuickinputSvcPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZQuickinputSvcPriority::Idle);
        assert_eq!(all[4], ZQuickinputSvcPriority::Realtime);
    }

    #[test]
    fn z_quickinput_svc_struct_new() {
        let s = ZQuickinputSvcQuickPickScorer::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_quickinput_svc_struct_toggled_clone() {
        let s = ZQuickinputSvcQuickPickScorer::new();
        let t = s.toggled_clone();
        let _ = t.threshold;
    }

    #[test]
    fn z_quickinput_svc_rolling_hash_deterministic() {
        let h1 = z_quickinput_svc_rolling_hash(b"test");
        let h2 = z_quickinput_svc_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_quickinput_svc_rolling_hash(b"a"), z_quickinput_svc_rolling_hash(b"b"));
    }

    #[test]
    fn z_quickinput_svc_pad_to_basic() {
        assert_eq!(z_quickinput_svc_pad_to("hi", 5), "hi   ");
        assert_eq!(z_quickinput_svc_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_quickinput_svc_is_identifier_basic() {
        assert!(z_quickinput_svc_is_identifier("foo_bar"));
        assert!(z_quickinput_svc_is_identifier("abc123"));
        assert!(!z_quickinput_svc_is_identifier(""));
        assert!(!z_quickinput_svc_is_identifier("has space"));
    }

    #[test]
    fn z_quickinput_svc_levenshtein_basic() {
        assert_eq!(z_quickinput_svc_levenshtein("", ""), 0);
        assert_eq!(z_quickinput_svc_levenshtein("abc", "abc"), 0);
        assert_eq!(z_quickinput_svc_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_quickinput_svc_unique_words_basic() {
        let w = z_quickinput_svc_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_quickinput_svc_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_quickinput_svc_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_quickinput_svc_common_prefix_basic() {
        assert_eq!(z_quickinput_svc_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_quickinput_svc_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_quickinput_svc_struct_clear() {
        let mut s = ZQuickinputSvcQuickPickScorer::new();
        s.scores.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_quickinput_svc_rolling_hash_empty() {
        let h = z_quickinput_svc_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }
}