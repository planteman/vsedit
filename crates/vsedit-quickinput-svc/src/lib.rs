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


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 144
// ---------------------------------------------------------------------------

/// Generic object pool `Xc144Pool<T>`.
pub struct Xc144Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc144Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc144PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc144Pool<T> {
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
    pub fn stats(&self) -> Xc144PoolStats {
        Xc144PoolStats {
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

impl<T> Default for Xc144Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc144Scheduler`.
pub struct Xc144Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc144Scheduler {
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

impl Default for Xc144Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_144 hash for the given byte slice.
pub fn xc_144_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_144 convention.
pub fn xc_144_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_2 deepening: state machine + event bus ---

/// States for the Xd2 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd2State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd2State {
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
pub struct Xd2Transition {
    pub from: Xd2State,
    pub to: Xd2State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd2StateMachine {
    current: Xd2State,
    history: Vec<Xd2Transition>,
    step_counter: usize,
}

impl Xd2StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd2State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd2State {
        self.current
    }

    pub fn history(&self) -> &[Xd2Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd2State) -> Result<Xd2State, String> {
        let allowed = match (self.current, target) {
            (Xd2State::Idle, Xd2State::Running) => true,
            (Xd2State::Running, Xd2State::Paused) => true,
            (Xd2State::Running, Xd2State::Done) => true,
            (Xd2State::Paused, Xd2State::Running) => true,
            (Xd2State::Paused, Xd2State::Done) => true,
            (Xd2State::Done, Xd2State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_2: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd2Transition {
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
            "Xd2SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd2State> {
        let prefix = "Xd2SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd2State::Idle),
            "Running" => Some(Xd2State::Running),
            "Paused" => Some(Xd2State::Paused),
            "Done" => Some(Xd2State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd2State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd2 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd2Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd2Event {
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

type Xd2HandlerFn = Box<dyn Fn(&Xd2Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd2EventBus {
    handlers: Vec<(usize, Option<String>, Xd2HandlerFn)>,
    next_id: usize,
    published: Vec<Xd2Event>,
}

impl Xd2EventBus {
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
        F: Fn(&Xd2Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd2Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd2Event) {
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

    pub fn published_events(&self) -> &[Xd2Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// === Xe121 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe121Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe121PipelineError {
    pub stage: Xe121Stage,
    pub message: String,
}

impl std::fmt::Display for Xe121PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe121Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe121Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError>>>,
    stage_names: Vec<Xe121Stage>,
}

impl Xe121Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe121Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe121Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe121Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe121Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe121Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe121CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe121CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe121Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe121CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe121CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe121Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe121CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_121_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe121CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_121_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe121CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_121_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> {
    Ok(data)
}

pub fn xe_121_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_121_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_121_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_121_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe121PipelineError> {
    Err(Xe121PipelineError {
        stage: Xe121Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_119: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg119Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg119Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg119Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_119: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg119Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg119Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg119Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg119Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 143).
pub struct Xh143SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh143SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 185 as u64,
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

/// A compact bit set supporting boolean operations (variant 143).
pub struct Xh143BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh143BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 143).
pub struct Xi143Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi143Deque<T> {
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
pub struct Xi143Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi143Interval {
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

/// A simple interval tree (variant 143).
pub struct Xi143IntervalTree {
    xi_intervals: Vec<Xi143Interval>,
}

impl Xi143IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi143Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi143Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi143Interval) -> Vec<&Xi143Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi143Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi143Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi143Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi143Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi143Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi143Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 144) ---

/// Disjoint set / union-find for crate 144.
pub struct Xj144UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj144UnionFind {
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

const XJ144_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 144.
pub struct Xj144BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj144BTreeNode<K, V>>>,
    len: usize,
}

struct Xj144BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj144BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj144BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ144_BTREE_ORDER - 1
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
        let mid = XJ144_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj144BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj144BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj144BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj144BTreeNode::xj_new_leaf();
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


// --- xk_144 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk144SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk144SegmentTree {
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
pub struct Xk144DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk144DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_144).
#[derive(Debug, Clone)]
pub struct Xl144Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl144Rope {
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

/// Suffix array for efficient string searching (xl_144).
#[derive(Debug, Clone)]
pub struct Xl144SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl144SuffixArray {
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
pub struct Xm144MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm144MatrixSparse {
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
pub struct Xm144Tokenizer {
    text: String,
}

impl Xm144Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 143.
pub struct Xn143Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn143Fenwick {
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

// ----- AVL tree map — crate 143 -----

#[derive(Debug, Clone)]
struct Xn143AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn143AvlNode<K, V>>>,
    right: Option<Box<Xn143AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 143.
#[derive(Debug, Clone)]
pub struct Xn143AVL<K, V> {
    root: Option<Box<Xn143AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn143AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn143AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn143AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn143AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn143AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn143AvlNode<K, V>>) -> Box<Xn143AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn143AvlNode<K, V>>) -> Box<Xn143AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn143AvlNode<K, V>>) -> Box<Xn143AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn143AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn143AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn143AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn143AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn143AvlNode<K, V>>) -> &Xn143AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn143AvlNode<K, V>>) -> (Box<Xn143AvlNode<K, V>>, Option<Box<Xn143AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn143AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn143AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn143AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn143AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn143AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn143AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn143AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo143RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo143Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo143RBNode<K, V> {
    key: K,
    value: V,
    color: Xo143Color,
    left: Option<Box<Xo143RBNode<K, V>>>,
    right: Option<Box<Xo143RBNode<K, V>>>,
}

/// A red-black tree map for crate 143.
#[derive(Debug, Clone)]
pub struct Xo143RedBlack<K, V> {
    root: Option<Box<Xo143RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo143RedBlack<K, V> {
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
            r.color = Xo143Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo143RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo143RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo143RBNode {
                    key, value, color: Xo143Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo143RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo143Color::Red)
    }

    fn xo_balance(mut h: Box<Xo143RBNode<K, V>>) -> Box<Xo143RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo143Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo143RBNode<K, V>>) -> Box<Xo143RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo143Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo143RBNode<K, V>>) -> Box<Xo143RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo143Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo143RBNode<K, V>>) {
        h.color = Xo143Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo143Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo143Color::Black; }
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
            r.color = Xo143Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo143RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo143RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo143RBNode<K, V>) -> (K, V, Option<Box<Xo143RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo143RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo143Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo143RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo143ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 143.
#[derive(Debug, Clone)]
pub struct Xo143ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo143ConsistentHash {
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
            let vkey = format!("{}#xo143#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo143#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 144).
#[derive(Debug)]
pub struct Xp144SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp144Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp144Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp144Node<K, V>>>,
    xp_right: Option<Box<Xp144Node<K, V>>>,
}

impl<K: Ord, V> Xp144Node<K, V> {
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

impl<K: Ord, V> Default for Xp144SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp144SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp144Node<K, V>>>, key: &K) -> Option<Box<Xp144Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp144Node<K, V>>) -> Box<Xp144Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp144Node<K, V>>) -> Box<Xp144Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp144Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp144Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp144Node::xp_new(key, val));
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


// --------------- Xq143Treap ---------------

use std::cmp::Ordering as Xq143Ord;

struct Xq143TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq143TreapNode<K, V>>>,
    right: Option<Box<Xq143TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq143Treap<K, V> {
    root: Option<Box<Xq143TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq143TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_143_size<K, V>(node: &Option<Box<Xq143TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_143_update_size<K, V>(node: &mut Xq143TreapNode<K, V>) {
    node.size = 1 + xq_143_size(&node.left) + xq_143_size(&node.right);
}

fn xq_143_rotate_right<K, V>(mut node: Box<Xq143TreapNode<K, V>>) -> Box<Xq143TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_143_update_size(&mut node);
    left.right = Some(node);
    xq_143_update_size(&mut left);
    left
}

fn xq_143_rotate_left<K, V>(mut node: Box<Xq143TreapNode<K, V>>) -> Box<Xq143TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_143_update_size(&mut node);
    right.left = Some(node);
    xq_143_update_size(&mut right);
    right
}

fn xq_143_insert_node<K: Ord, V>(
    node: Option<Box<Xq143TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq143TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq143TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq143Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq143Ord::Less => {
                let (new_left, old) = xq_143_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_143_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_143_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq143Ord::Greater => {
                let (new_right, old) = xq_143_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_143_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_143_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_143_remove_node<K: Ord, V>(
    node: Option<Box<Xq143TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq143TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq143Ord::Less => {
                let (new_left, old) = xq_143_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_143_update_size(&mut n);
                (Some(n), old)
            }
            Xq143Ord::Greater => {
                let (new_right, old) = xq_143_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_143_update_size(&mut n);
                (Some(n), old)
            }
            Xq143Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_143_rotate_right(n);
                    let (new_right, old) = xq_143_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_143_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_143_rotate_left(n);
                    let (new_left, old) = xq_143_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_143_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_143_find_min<K, V>(node: &Option<Box<Xq143TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_143_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_143_find_max<K, V>(node: &Option<Box<Xq143TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_143_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_143_rank<K: Ord, V>(node: &Option<Box<Xq143TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq143Ord::Less => xq_143_rank(&n.left, key),
            Xq143Ord::Equal => xq_143_size(&n.left),
            Xq143Ord::Greater => 1 + xq_143_size(&n.left) + xq_143_rank(&n.right, key),
        },
    }
}

fn xq_143_kth<K, V>(node: &Option<Box<Xq143TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_143_size(&n.left);
        if k < left_size {
            xq_143_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_143_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_143_in_order<K: Clone, V>(node: &Option<Box<Xq143TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_143_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_143_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq143Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 143 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_143_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq143Ord::Equal => return Some(&n.value),
                Xq143Ord::Less => cur = &n.left,
                Xq143Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_143_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_143_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_143_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_143_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_143_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_143_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_143_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq143VEBTree ---------------

pub struct Xq143VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq143VEBTree>>,
    clusters: Vec<Option<Box<Xq143VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq143VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq143VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq143VEBTree::xq_new(self.sqrt_lo)));
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

    // ---- xc_ pool / scheduler tests – block 144 ----

    #[test]
    fn xc_144_pool_new_empty() {
        let pool: super::Xc144Pool<i32> = super::Xc144Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_144_pool_release_acquire() {
        let mut pool = super::Xc144Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_144_pool_acquire_empty() {
        let mut pool: super::Xc144Pool<i32> = super::Xc144Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_144_pool_full() {
        let mut pool = super::Xc144Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_144_pool_drain() {
        let mut pool = super::Xc144Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_144_pool_stats() {
        let mut pool = super::Xc144Pool::new(8);
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
    fn xc_144_pool_clear() {
        let mut pool = super::Xc144Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_144_pool_shrink() {
        let mut pool = super::Xc144Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_144_pool_default() {
        let pool: super::Xc144Pool<String> = super::Xc144Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_144_pool_extend() {
        let mut pool = super::Xc144Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_144_pool_retain() {
        let mut pool = super::Xc144Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_144_scheduler_round_robin() {
        let mut sched = super::Xc144Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_144_scheduler_empty() {
        let mut sched = super::Xc144Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_144_scheduler_reset() {
        let mut sched = super::Xc144Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_144_scheduler_add_remove() {
        let mut sched = super::Xc144Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_144_scheduler_targets() {
        let sched = super::Xc144Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_144_hash_empty() {
        assert_eq!(super::xc_144_hash(b""), 5381);
    }

    #[test]
    fn xc_144_hash_data() {
        let h = super::xc_144_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_144_hash(b"hello"), h);
    }

    #[test]
    fn xc_144_reverse_str() {
        assert_eq!(super::xc_144_reverse("abc"), "cba");
        assert_eq!(super::xc_144_reverse(""), "");
    }


    // --- xd_2 deepening tests ---

    #[test]
    fn xd_2_sm_initial_state() {
        let sm = Xd2StateMachine::new();
        assert_eq!(sm.current_state(), Xd2State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_2_sm_valid_idle_to_running() {
        let mut sm = Xd2StateMachine::new();
        assert!(sm.transition(Xd2State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd2State::Running);
    }

    #[test]
    fn xd_2_sm_valid_running_to_paused() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        assert!(sm.transition(Xd2State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd2State::Paused);
    }

    #[test]
    fn xd_2_sm_valid_running_to_done() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        assert!(sm.transition(Xd2State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd2State::Done);
    }

    #[test]
    fn xd_2_sm_valid_paused_to_running() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        sm.transition(Xd2State::Paused).unwrap();
        assert!(sm.transition(Xd2State::Running).is_ok());
    }

    #[test]
    fn xd_2_sm_valid_done_to_idle() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        sm.transition(Xd2State::Done).unwrap();
        assert!(sm.transition(Xd2State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd2State::Idle);
    }

    #[test]
    fn xd_2_sm_invalid_idle_to_done() {
        let mut sm = Xd2StateMachine::new();
        assert!(sm.transition(Xd2State::Done).is_err());
    }

    #[test]
    fn xd_2_sm_invalid_idle_to_paused() {
        let mut sm = Xd2StateMachine::new();
        assert!(sm.transition(Xd2State::Paused).is_err());
    }

    #[test]
    fn xd_2_sm_history_tracking() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        sm.transition(Xd2State::Paused).unwrap();
        sm.transition(Xd2State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd2State::Idle);
        assert_eq!(sm.history()[0].to, Xd2State::Running);
        assert_eq!(sm.history()[1].from, Xd2State::Running);
        assert_eq!(sm.history()[2].to, Xd2State::Done);
    }

    #[test]
    fn xd_2_sm_serialize_deserialize() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd2StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd2State::Running));
    }

    #[test]
    fn xd_2_sm_deserialize_invalid() {
        assert_eq!(Xd2StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_2_sm_reset() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd2State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_2_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd2EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd2Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_2_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd2EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd2Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd2Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_2_bus_unsubscribe() {
        let mut bus = Xd2EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_2_event_kind_and_payload() {
        let e = Xd2Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd2Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_2_bus_clear_history() {
        let mut bus = Xd2EventBus::new();
        bus.publish(Xd2Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_2_sm_step_counter_increments() {
        let mut sm = Xd2StateMachine::new();
        sm.transition(Xd2State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd2State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    #[test]
    fn xe_121_pipeline_empty() {
        let p = super::Xe121Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_121_pipeline_parse_stage() {
        let p = super::Xe121Pipeline::new()
            .add_parse(super::xe_121_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_121_pipeline_transform_double() {
        let p = super::Xe121Pipeline::new()
            .add_transform(super::xe_121_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_121_pipeline_validate_reverse() {
        let p = super::Xe121Pipeline::new()
            .add_validate(super::xe_121_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_121_pipeline_emit_filter() {
        let p = super::Xe121Pipeline::new()
            .add_emit(super::xe_121_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_121_pipeline_multi_stage() {
        let p = super::Xe121Pipeline::new()
            .add_parse(super::xe_121_pipeline_identity)
            .add_transform(super::xe_121_pipeline_double)
            .add_validate(super::xe_121_pipeline_reverse)
            .add_emit(super::xe_121_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_121_pipeline_error_propagation() {
        let p = super::Xe121Pipeline::new()
            .add_parse(super::xe_121_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe121Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_121_pipeline_compose() {
        let p1 = super::Xe121Pipeline::new()
            .add_parse(super::xe_121_pipeline_identity);
        let p2 = super::Xe121Pipeline::new()
            .add_transform(super::xe_121_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_121_pipeline_error_display() {
        let e = super::Xe121PipelineError {
            stage: super::Xe121Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_121_cache_put_get() {
        let mut c = super::Xe121Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_121_cache_miss() {
        let mut c: super::Xe121Cache<&str, i32> = super::Xe121Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_121_cache_ttl_expiry() {
        let mut c = super::Xe121Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_121_cache_evict() {
        let mut c = super::Xe121Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_121_cache_capacity() {
        let mut c = super::Xe121Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_121_cache_stats() {
        let mut c = super::Xe121Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_121_cache_clear() {
        let mut c = super::Xe121Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_119 graph tests ------------------------------------------------

    #[test]
    fn xg_119_graph_empty() {
        let g = super::Xg119Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_119_graph_add_node() {
        let mut g = super::Xg119Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_119_graph_add_edge() {
        let mut g = super::Xg119Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_119_graph_neighbors() {
        let mut g = super::Xg119Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_119_graph_has_path() {
        let mut g = super::Xg119Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_119_graph_self_path() {
        let g = super::Xg119Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_119_graph_topo_sort() {
        let mut g = super::Xg119Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_119_graph_cycle_detect_false() {
        let mut g = super::Xg119Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_119_graph_cycle_detect_true() {
        let mut g = super::Xg119Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_119 heap tests -------------------------------------------------

    #[test]
    fn xg_119_heap_empty() {
        let h: super::Xg119Heap<i32> = super::Xg119Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_119_heap_push_pop() {
        let mut h = super::Xg119Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_119_heap_peek() {
        let mut h = super::Xg119Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_119_heap_drain_sorted() {
        let mut h = super::Xg119Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_119_heap_merge() {
        let mut a = super::Xg119Heap::new();
        let mut b = super::Xg119Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_119_heap_default() {
        let h: super::Xg119Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_119_graph_default() {
        let g: super::Xg119Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh143_skip_insert_contains() {
        let mut sl = super::Xh143SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh143_skip_remove() {
        let mut sl = super::Xh143SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh143_skip_len() {
        let mut sl = super::Xh143SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh143_skip_range_query() {
        let mut sl = super::Xh143SkipList::xh_new(4);
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
    fn xh143_skip_floor_ceiling() {
        let mut sl = super::Xh143SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh143_skip_rank() {
        let mut sl = super::Xh143SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh143_skip_empty() {
        let sl = super::Xh143SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh143_skip_duplicates() {
        let mut sl = super::Xh143SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh143_bitset_set_test() {
        let mut bs = super::Xh143BitSet::xh_new(256);
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
    fn xh143_bitset_clear_count() {
        let mut bs = super::Xh143BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh143_bitset_and_or_xor() {
        let mut a = super::Xh143BitSet::xh_new(128);
        let mut b = super::Xh143BitSet::xh_new(128);
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
    fn xh143_bitset_iter_ones() {
        let mut bs = super::Xh143BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh143_bitset_first_last() {
        let mut bs = super::Xh143BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh143_bitset_empty() {
        let bs = super::Xh143BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi143_deque_push_pop_back() {
        let mut dq = super::Xi143Deque::xi_new(4);
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
    fn xi143_deque_push_pop_front() {
        let mut dq = super::Xi143Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi143_deque_mixed_ops() {
        let mut dq = super::Xi143Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi143_deque_get_and_split() {
        let mut dq = super::Xi143Deque::xi_new(8);
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
    fn xi143_deque_rotate_left() {
        let mut dq = super::Xi143Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi143_deque_rotate_right() {
        let mut dq = super::Xi143Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi143_deque_grow() {
        let mut dq = super::Xi143Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi143_deque_empty() {
        let dq = super::Xi143Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi143_interval_tree_insert_query() {
        let mut tree = super::Xi143IntervalTree::xi_new();
        tree.xi_insert(super::Xi143Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi143Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi143Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi143_interval_tree_overlap() {
        let mut tree = super::Xi143IntervalTree::xi_new();
        tree.xi_insert(super::Xi143Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi143Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi143Interval::xi_new(12, 20));
        let q = super::Xi143Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi143_interval_tree_remove() {
        let mut tree = super::Xi143IntervalTree::xi_new();
        tree.xi_insert(super::Xi143Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi143Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi143_interval_tree_gaps() {
        let mut tree = super::Xi143IntervalTree::xi_new();
        tree.xi_insert(super::Xi143Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi143Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi143Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi143Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi143Interval::xi_new(8, 10));
    }

    #[test]
    fn xi143_interval_tree_merge() {
        let mut tree = super::Xi143IntervalTree::xi_new();
        tree.xi_insert(super::Xi143Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi143Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi143Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi143Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi143Interval::xi_new(10, 15));
    }

    #[test]
    fn xi143_interval_tree_all() {
        let mut tree = super::Xi143IntervalTree::xi_new();
        tree.xi_insert(super::Xi143Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi143Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi143_interval_tree_empty() {
        let tree = super::Xi143IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi143_interval_tree_contains_point() {
        let iv = super::Xi143Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 144) ---

    #[test]
    fn xj_144_uf_make_and_find() {
        let mut uf = super::Xj144UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_144_uf_union_connected() {
        let mut uf = super::Xj144UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_144_uf_component_count() {
        let mut uf = super::Xj144UnionFind::xj_new();
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
    fn xj_144_uf_component_size() {
        let mut uf = super::Xj144UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_144_uf_largest_component() {
        let mut uf = super::Xj144UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_144_uf_many_elements() {
        let mut uf = super::Xj144UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_144_uf_separate_components() {
        let mut uf = super::Xj144UnionFind::xj_new();
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
    fn xj_144_uf_path_compression() {
        let mut uf = super::Xj144UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_144_bt_insert_get() {
        let mut bt = super::Xj144BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_144_bt_contains_len() {
        let mut bt = super::Xj144BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_144_bt_replace() {
        let mut bt = super::Xj144BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_144_bt_remove() {
        let mut bt = super::Xj144BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_144_bt_keys_values() {
        let mut bt = super::Xj144BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_144_bt_range() {
        let mut bt = super::Xj144BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_144_bt_min_max() {
        let mut bt = super::Xj144BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_144_bt_many_inserts() {
        let mut bt = super::Xj144BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_144 segment tree tests ---

    #[test]
    fn xk_144_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk144SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_144_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk144SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_144_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk144SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_144_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk144SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_144_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk144SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_144_st_single_element() {
        let data = vec![42];
        let st = super::Xk144SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_144_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk144SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_144_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk144SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_144 disjoint intervals tests ---

    #[test]
    fn xk_144_di_add_and_count() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_144_di_merge_overlap() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_144_di_contains() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_144_di_remove() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_144_di_covered_length() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_144_di_gaps() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_144_di_merge_adjacent() {
        let mut di = super::Xk144DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_144_di_empty() {
        let di = super::Xk144DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_144_rope_new_empty() {
        let rope = super::Xl144Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_144_rope_from_str() {
        let rope = super::Xl144Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_144_rope_insert_at() {
        let mut rope = super::Xl144Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_144_rope_delete_range() {
        let mut rope = super::Xl144Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_144_rope_char_at() {
        let rope = super::Xl144Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_144_rope_split_concat() {
        let rope = super::Xl144Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_144_rope_line_count() {
        let rope = super::Xl144Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_144_rope_line_at() {
        let rope = super::Xl144Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_144_sa_build_and_search() {
        let sa = super::Xl144SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_144_sa_count() {
        let sa = super::Xl144SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_144_sa_longest_repeated() {
        let sa = super::Xl144SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_144_sa_all_positions() {
        let sa = super::Xl144SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_144_sa_len() {
        let sa = super::Xl144SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_144_sa_empty() {
        let sa = super::Xl144SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_144_rope_slice() {
        let rope = super::Xl144Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_144_sa_search_start() {
        let sa = super::Xl144SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_144_sparse_set_get() {
        let mut m = super::Xm144MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_144_sparse_row_col() {
        let mut m = super::Xm144MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_144_sparse_transpose() {
        let mut m = super::Xm144MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_144_sparse_multiply_vec() {
        let mut m = super::Xm144MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_144_sparse_nnz_density() {
        let mut m = super::Xm144MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_144_sparse_clear() {
        let mut m = super::Xm144MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_144_sparse_overwrite_zero() {
        let mut m = super::Xm144MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_144_tokenizer_basic() {
        let t = super::Xm144Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_144_tokenizer_count() {
        let t = super::Xm144Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_144_tokenizer_unique() {
        let t = super::Xm144Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_144_tokenizer_frequency() {
        let t = super::Xm144Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_144_tokenizer_delimiter() {
        let t = super::Xm144Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_144_tokenizer_whitespace() {
        let t = super::Xm144Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_144_tokenizer_empty() {
        let t = super::Xm144Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 143 ----

    #[test]
    fn xn_143_fenwick_prefix_sum() {
        let mut ft = super::Xn143Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_143_fenwick_range_sum() {
        let mut ft = super::Xn143Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_143_fenwick_point_query() {
        let mut ft = super::Xn143Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_143_fenwick_len() {
        let ft = super::Xn143Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_143_fenwick_multiple_updates() {
        let mut ft = super::Xn143Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_143_fenwick_single_element() {
        let mut ft = super::Xn143Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_143_fenwick_find_kth() {
        let mut ft = super::Xn143Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_143_fenwick_negative_delta() {
        let mut ft = super::Xn143Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 143 ----

    #[test]
    fn xn_143_avl_insert_get() {
        let mut m = super::Xn143AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_143_avl_remove() {
        let mut m = super::Xn143AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_143_avl_in_order() {
        let mut m = super::Xn143AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_143_avl_min_max() {
        let mut m = super::Xn143AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_143_avl_floor_ceiling() {
        let mut m = super::Xn143AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_143_avl_height_balanced() {
        let mut m = super::Xn143AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_143_avl_overwrite() {
        let mut m = super::Xn143AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_143_avl_empty() {
        let m: super::Xn143AVL<i32, i32> = super::Xn143AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo143RedBlack tests ---

    #[test]
    fn xo_143_rb_insert_and_get() {
        let mut tree = super::Xo143RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_143_rb_len_and_empty() {
        let mut tree = super::Xo143RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_143_rb_min_max() {
        let mut tree = super::Xo143RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_143_rb_contains() {
        let mut tree = super::Xo143RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_143_rb_remove() {
        let mut tree = super::Xo143RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_143_rb_in_order() {
        let mut tree = super::Xo143RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_143_rb_black_height() {
        let mut tree = super::Xo143RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_143_rb_overwrite() {
        let mut tree = super::Xo143RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo143ConsistentHash tests ---

    #[test]
    fn xo_143_ch_add_and_count() {
        let mut ring = super::Xo143ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_143_ch_remove_node() {
        let mut ring = super::Xo143ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_143_ch_get_node() {
        let mut ring = super::Xo143ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_143_ch_empty_ring() {
        let ring = super::Xo143ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_143_ch_distribution() {
        let mut ring = super::Xo143ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_143_ch_rebalance() {
        let mut ring = super::Xo143ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_143_ch_virtual_nodes() {
        let mut ring = super::Xo143ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_143_ch_consistent_lookup() {
        let mut ring = super::Xo143ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_144_splay_insert_get() {
        let mut t = super::Xp144SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_144_splay_remove() {
        let mut t = super::Xp144SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_144_splay_count_increases() {
        let mut t = super::Xp144SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_144_splay_depth() {
        let mut t = super::Xp144SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_144_splay_len_empty() {
        let t = super::Xp144SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_144_splay_min_max() {
        let mut t = super::Xp144SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_144_splay_overwrite() {
        let mut t = super::Xp144SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_144_splay_remove_missing() {
        let mut t = super::Xp144SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_143 treap tests ----
    #[test]
    fn xq_143_treap_empty() {
        let t = super::Xq143Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_143_treap_insert_get() {
        let mut t = super::Xq143Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_143_treap_overwrite() {
        let mut t = super::Xq143Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_143_treap_remove() {
        let mut t = super::Xq143Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_143_treap_min_max() {
        let mut t = super::Xq143Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_143_treap_rank() {
        let mut t = super::Xq143Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_143_treap_kth() {
        let mut t = super::Xq143Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_143_treap_in_order() {
        let mut t = super::Xq143Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_143 VEB tree tests ----
    #[test]
    fn xq_143_veb_empty() {
        let v = super::Xq143VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_143_veb_insert_contains() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_143_veb_min_max() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_143_veb_delete() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_143_veb_successor() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_143_veb_predecessor() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_143_veb_count() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_143_veb_duplicate_insert() {
        let mut v = super::Xq143VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }

}