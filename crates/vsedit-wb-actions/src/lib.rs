//! Command/action execution.

use std::fmt;

/// Category for grouping actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionCategory {
    View,
    Edit,
    File,
    Selection,
    Terminal,
    Help,
    Debug,
    Source,
}

impl fmt::Display for ActionCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionCategory::View => write!(f, "View"),
            ActionCategory::Edit => write!(f, "Edit"),
            ActionCategory::File => write!(f, "File"),
            ActionCategory::Selection => write!(f, "Selection"),
            ActionCategory::Terminal => write!(f, "Terminal"),
            ActionCategory::Help => write!(f, "Help"),
            ActionCategory::Debug => write!(f, "Debug"),
            ActionCategory::Source => write!(f, "Source"),
        }
    }
}

/// A registered action that can be executed.
#[derive(Debug, Clone)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub category: ActionCategory,
    pub keybinding: Option<String>,
    pub precondition: Option<String>,
    pub enabled: bool,
}

impl Action {
    /// Start building a new action with the required fields.
    pub fn builder(id: impl Into<String>, label: impl Into<String>, category: ActionCategory) -> ActionBuilder {
        ActionBuilder {
            id: id.into(),
            label: label.into(),
            category,
            keybinding: None,
            precondition: None,
            enabled: true,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.keybinding {
            Some(kb) => write!(f, "{} ({}) [{}]", self.label, self.id, kb),
            None => write!(f, "{} ({})", self.label, self.id),
        }
    }
}

/// Builder for constructing [`Action`] instances.
pub struct ActionBuilder {
    id: String,
    label: String,
    category: ActionCategory,
    keybinding: Option<String>,
    precondition: Option<String>,
    enabled: bool,
}

impl ActionBuilder {
    pub fn keybinding(mut self, keybinding: impl Into<String>) -> Self {
        self.keybinding = Some(keybinding.into());
        self
    }

    pub fn precondition(mut self, precondition: impl Into<String>) -> Self {
        self.precondition = Some(precondition.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn build(self) -> Action {
        Action {
            id: self.id,
            label: self.label,
            category: self.category,
            keybinding: self.keybinding,
            precondition: self.precondition,
            enabled: self.enabled,
        }
    }
}

/// Registry of all available actions.
pub struct ActionRegistry {
    actions: Vec<Action>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn register(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        let len = self.actions.len();
        self.actions.retain(|a| a.id != id);
        self.actions.len() != len
    }

    pub fn get_action(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    pub fn get_by_category(&self, category: ActionCategory) -> Vec<&Action> {
        self.actions
            .iter()
            .filter(|a| a.category == category)
            .collect()
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if let Some(action) = self.actions.iter_mut().find(|a| a.id == id) {
            action.enabled = enabled;
        }
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    /// Search actions by id or label (case-insensitive).
    pub fn find_actions(&self, query: &str) -> Vec<&Action> {
        let q = query.to_lowercase();
        self.actions
            .iter()
            .filter(|a| a.id.to_lowercase().contains(&q) || a.label.to_lowercase().contains(&q))
            .collect()
    }

    /// Return all enabled actions.
    pub fn get_enabled_actions(&self) -> Vec<&Action> {
        self.actions.iter().filter(|a| a.enabled).collect()
    }

    /// Return all disabled actions.
    pub fn get_disabled_actions(&self) -> Vec<&Action> {
        self.actions.iter().filter(|a| !a.enabled).collect()
    }

    /// Check whether an action with the given id exists.
    pub fn has_action(&self, id: &str) -> bool {
        self.actions.iter().any(|a| a.id == id)
    }

    /// Find an action by its keybinding.
    pub fn get_by_keybinding(&self, keybinding: &str) -> Option<&Action> {
        self.actions
            .iter()
            .find(|a| a.keybinding.as_deref() == Some(keybinding))
    }

    /// Remove all actions belonging to a category.
    pub fn clear_category(&mut self, category: ActionCategory) {
        self.actions.retain(|a| a.category != category);
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors related to action operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    NotFound(String),
    Disabled(String),
    DuplicateId(String),
    PreconditionFailed(String),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "action not found: {id}"),
            Self::Disabled(id) => write!(f, "action is disabled: {id}"),
            Self::DuplicateId(id) => write!(f, "duplicate action id: {id}"),
            Self::PreconditionFailed(msg) => write!(f, "precondition failed: {msg}"),
        }
    }
}

impl std::error::Error for ActionError {}

impl Action {
    /// Check if this action has a keybinding assigned.
    pub fn has_keybinding(&self) -> bool {
        self.keybinding.is_some()
    }

    /// Check if this action has a precondition set.
    pub fn has_precondition(&self) -> bool {
        self.precondition.is_some()
    }

    /// Create a clone of this action with a new ID.
    pub fn clone_with_id(&self, new_id: impl Into<String>) -> Self {
        Action {
            id: new_id.into(),
            label: self.label.clone(),
            category: self.category,
            keybinding: self.keybinding.clone(),
            precondition: self.precondition.clone(),
            enabled: self.enabled,
        }
    }
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl ActionRegistry {
    /// Register an action, returning an error if an action with the same ID exists.
    pub fn try_register(&mut self, action: Action) -> Result<(), ActionError> {
        if self.has_action(&action.id) {
            return Err(ActionError::DuplicateId(action.id));
        }
        self.register(action);
        Ok(())
    }

    /// Execute an action by ID: checks it exists and is enabled.
    pub fn try_execute(&self, id: &str) -> Result<&Action, ActionError> {
        let action = self
            .get_action(id)
            .ok_or_else(|| ActionError::NotFound(id.to_string()))?;
        if !action.enabled {
            return Err(ActionError::Disabled(id.to_string()));
        }
        Ok(action)
    }

    /// Return all registered action IDs.
    pub fn action_ids(&self) -> Vec<&str> {
        self.actions.iter().map(|a| a.id.as_str()).collect()
    }

    /// Return all unique categories present in the registry.
    pub fn unique_categories(&self) -> Vec<ActionCategory> {
        let mut cats: Vec<ActionCategory> = Vec::new();
        for action in &self.actions {
            if !cats.contains(&action.category) {
                cats.push(action.category);
            }
        }
        cats
    }

    /// Count actions in a specific category.
    pub fn count_by_category(&self, category: ActionCategory) -> usize {
        self.actions
            .iter()
            .filter(|a| a.category == category)
            .count()
    }

    /// Find all actions that have keybindings.
    pub fn actions_with_keybindings(&self) -> Vec<&Action> {
        self.actions
            .iter()
            .filter(|a| a.has_keybinding())
            .collect()
    }

    /// Find all actions without keybindings.
    pub fn actions_without_keybindings(&self) -> Vec<&Action> {
        self.actions
            .iter()
            .filter(|a| !a.has_keybinding())
            .collect()
    }

    /// Rename an action's label. Returns whether the action was found.
    pub fn rename_action(&mut self, id: &str, new_label: impl Into<String>) -> bool {
        if let Some(action) = self.actions.iter_mut().find(|a| a.id == id) {
            action.label = new_label.into();
            true
        } else {
            false
        }
    }

    /// Set or update the keybinding for an action.
    pub fn set_keybinding(&mut self, id: &str, keybinding: impl Into<String>) -> bool {
        if let Some(action) = self.actions.iter_mut().find(|a| a.id == id) {
            action.keybinding = Some(keybinding.into());
            true
        } else {
            false
        }
    }

    /// Remove the keybinding from an action.
    pub fn clear_keybinding(&mut self, id: &str) -> bool {
        if let Some(action) = self.actions.iter_mut().find(|a| a.id == id) {
            action.keybinding = None;
            true
        } else {
            false
        }
    }

    /// Get a summary of the registry for display purposes.
    pub fn summary(&self) -> String {
        format!(
            "{} actions ({} enabled, {} with keybindings, {} categories)",
            self.action_count(),
            self.get_enabled_actions().len(),
            self.actions_with_keybindings().len(),
            self.unique_categories().len(),
        )
    }
}

impl fmt::Display for ActionRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActionRegistry({} actions)", self.action_count())
    }
}

/// Accumulated statistics for wb-actions operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbActionsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbActionsStats {
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
    pub fn merge(&mut self, other: &WbActionsStats) {
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

impl Default for WbActionsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbActionsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbActionsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-actions.
#[derive(Debug, Clone)]
pub struct WbActionsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbActionsValidator {
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

impl Default for WbActionsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Action bar orientation
// ---------------------------------------------------------------------------

/// Layout orientation for an action bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionBarOrientation {
    Horizontal,
    Vertical,
}

impl fmt::Display for ActionBarOrientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionBarOrientation::Horizontal => write!(f, "horizontal"),
            ActionBarOrientation::Vertical => write!(f, "vertical"),
        }
    }
}

/// Configuration for an action bar's layout and behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBarConfig {
    pub orientation: ActionBarOrientation,
    pub max_visible: usize,
    pub show_labels: bool,
    pub icon_size: u16,
    items: Vec<ActionBarItem>,
}

/// An item in the action bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionBarItem {
    pub action_id: String,
    pub icon: String,
    pub label: String,
    pub order: i32,
    pub visible: bool,
}

impl ActionBarItem {
    pub fn new(action_id: impl Into<String>, icon: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            action_id: action_id.into(),
            icon: icon.into(),
            label: label.into(),
            order: 0,
            visible: true,
        }
    }

    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

impl ActionBarConfig {
    pub fn new(orientation: ActionBarOrientation) -> Self {
        Self {
            orientation,
            max_visible: 8,
            show_labels: true,
            icon_size: 16,
            items: Vec::new(),
        }
    }

    pub fn with_max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    pub fn with_show_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    pub fn add_item(&mut self, item: ActionBarItem) {
        self.items.push(item);
    }

    /// Return visible items sorted by order, capped at max_visible.
    pub fn visible_items(&self) -> Vec<&ActionBarItem> {
        let mut items: Vec<_> = self.items.iter().filter(|i| i.visible).collect();
        items.sort_by_key(|i| i.order);
        items.truncate(self.max_visible);
        items
    }

    /// Return items that didn't fit in the visible set (overflow menu).
    pub fn overflow_items(&self) -> Vec<&ActionBarItem> {
        let mut items: Vec<_> = self.items.iter().filter(|i| i.visible).collect();
        items.sort_by_key(|i| i.order);
        if items.len() > self.max_visible {
            items.split_off(self.max_visible)
        } else {
            Vec::new()
        }
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

// ---------------------------------------------------------------------------
// Action iterator
// ---------------------------------------------------------------------------

/// Iterator over actions filtered by category.
pub struct ActionCategoryIter<'a> {
    actions: &'a [Action],
    category: ActionCategory,
    index: usize,
}

impl<'a> ActionCategoryIter<'a> {
    pub fn new(actions: &'a [Action], category: ActionCategory) -> Self {
        Self { actions, category, index: 0 }
    }
}

impl<'a> Iterator for ActionCategoryIter<'a> {
    type Item = &'a Action;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.actions.len() {
            let action = &self.actions[self.index];
            self.index += 1;
            if action.category == self.category {
                return Some(action);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// ActionCategory helpers
// ---------------------------------------------------------------------------

impl ActionCategory {
    /// Returns all category variants.
    pub fn all() -> &'static [ActionCategory] {
        &[
            ActionCategory::View,
            ActionCategory::Edit,
            ActionCategory::File,
            ActionCategory::Selection,
            ActionCategory::Terminal,
            ActionCategory::Help,
            ActionCategory::Debug,
            ActionCategory::Source,
        ]
    }

    /// Parse a category from a string.
    pub fn from_str_opt(s: &str) -> Option<ActionCategory> {
        match s.to_lowercase().as_str() {
            "view" => Some(ActionCategory::View),
            "edit" => Some(ActionCategory::Edit),
            "file" => Some(ActionCategory::File),
            "selection" => Some(ActionCategory::Selection),
            "terminal" => Some(ActionCategory::Terminal),
            "help" => Some(ActionCategory::Help),
            "debug" => Some(ActionCategory::Debug),
            "source" => Some(ActionCategory::Source),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Action search helpers
// ---------------------------------------------------------------------------

/// Fuzzy-match score for searching actions by label.
pub fn action_match_score(action: &Action, query: &str) -> Option<u32> {
    let label_lower = action.label.to_lowercase();
    let query_lower = query.to_lowercase();

    if label_lower == query_lower {
        return Some(100);
    }
    if label_lower.starts_with(&query_lower) {
        return Some(80);
    }
    if label_lower.contains(&query_lower) {
        return Some(60);
    }
    // Check if all query chars appear in order
    let mut label_chars = label_lower.chars();
    let mut matched = 0u32;
    for qc in query_lower.chars() {
        if label_chars.any(|lc| lc == qc) {
            matched += 1;
        } else {
            return None;
        }
    }
    Some(matched * 10)
}

/// Search actions by label query, returning matches sorted by score.
pub fn search_actions(actions: &[Action], query: &str) -> Vec<(usize, u32)> {
    let mut results: Vec<(usize, u32)> = actions.iter()
        .enumerate()
        .filter_map(|(i, a)| action_match_score(a, query).map(|s| (i, s)))
        .collect();
    results.sort_by(|a, b| b.1.cmp(&a.1));
    results
}

/// Groups actions by category, returning a map from category display name to count.
pub fn group_actions_by_category(actions: &[Action]) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for action in actions {
        *map.entry(format!("{}", action.category)).or_insert(0) += 1;
    }
    map
}

/// Validates an action ID format (must be non-empty, lowercase with dots/dashes).
pub fn validate_action_id(id: &str) -> Result<(), ActionError> {
    if id.is_empty() {
        return Err(ActionError::NotFound(id.to_string()));
    }
    if !id.chars().all(|c| c.is_ascii_lowercase() || c == '.' || c == '-' || c == '_') {
        return Err(ActionError::PreconditionFailed(format!("invalid action id: {id}")));
    }
    Ok(())
}

/// Returns actions that have keybindings assigned.
pub fn actions_with_keybindings(actions: &[Action]) -> Vec<&Action> {
    actions.iter().filter(|a| a.keybinding.is_some()).collect()
}

// ---------------------------------------------------------------------------
// ActionExecutionResult
// ---------------------------------------------------------------------------

/// Result of executing an action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionExecutionResult {
    Success,
    Failure(String),
    Cancelled,
    NotFound(String),
}

impl ActionExecutionResult {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failure(_))
    }
}

impl fmt::Display for ActionExecutionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Failure(msg) => write!(f, "failed: {msg}"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::NotFound(id) => write!(f, "action '{id}' not found"),
        }
    }
}

// ---------------------------------------------------------------------------
// ActionHistory
// ---------------------------------------------------------------------------

/// An entry in the action history.
#[derive(Debug, Clone)]
pub struct ActionHistoryEntry {
    pub action_id: String,
    pub result: ActionExecutionResult,
}

/// Tracks recently executed actions.
#[derive(Debug, Clone)]
pub struct ActionHistory {
    entries: Vec<ActionHistoryEntry>,
    max_size: usize,
}

impl ActionHistory {
    pub fn new(max_size: usize) -> Self {
        Self { entries: Vec::new(), max_size }
    }

    pub fn push(&mut self, action_id: impl Into<String>, result: ActionExecutionResult) {
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(ActionHistoryEntry {
            action_id: action_id.into(),
            result,
        });
    }

    pub fn last(&self) -> Option<&ActionHistoryEntry> {
        self.entries.last()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActionHistoryEntry> {
        self.entries.iter()
    }

    pub fn success_count(&self) -> usize {
        self.entries.iter().filter(|e| e.result.is_success()).count()
    }
}

impl fmt::Display for ActionHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActionHistory({} entries, {} successes)", self.len(), self.success_count())
    }
}

impl ActionHistory {
    /// Return the number of failed entries.
    pub fn failure_count(&self) -> usize {
        self.entries.iter().filter(|e| e.result.is_failure()).count()
    }

    /// Return the number of cancelled entries.
    pub fn cancelled_count(&self) -> usize {
        self.entries.iter().filter(|e| matches!(e.result, ActionExecutionResult::Cancelled)).count()
    }

    /// Return the most recently executed action ID, if any.
    pub fn last_action_id(&self) -> Option<&str> {
        self.last().map(|e| e.action_id.as_str())
    }

    /// Return true if the history contains an entry for the given action ID.
    pub fn contains_action(&self, action_id: &str) -> bool {
        self.entries.iter().any(|e| e.action_id == action_id)
    }

    /// Return success rate as a fraction (0.0 to 1.0).
    pub fn success_rate(&self) -> f64 {
        if self.entries.is_empty() { return 0.0; }
        self.success_count() as f64 / self.len() as f64
    }
}

impl ActionRegistry {
    /// Return all action labels as a vector.
    pub fn action_labels(&self) -> Vec<&str> {
        self.actions.iter().map(|a| a.label.as_str()).collect()
    }

    /// Return actions that match a precondition substring (case-insensitive).
    pub fn find_by_precondition(&self, query: &str) -> Vec<&Action> {
        let q = query.to_lowercase();
        self.actions.iter().filter(|a| {
            a.precondition.as_ref().map_or(false, |p| p.to_lowercase().contains(&q))
        }).collect()
    }

    /// Disable all actions in a given category. Returns how many were disabled.
    pub fn disable_category(&mut self, category: ActionCategory) -> usize {
        let mut count = 0;
        for action in &mut self.actions {
            if action.category == category && action.enabled {
                action.enabled = false;
                count += 1;
            }
        }
        count
    }

    /// Enable all actions in the registry. Returns how many were newly enabled.
    pub fn enable_all(&mut self) -> usize {
        let mut count = 0;
        for action in &mut self.actions {
            if !action.enabled {
                action.enabled = true;
                count += 1;
            }
        }
        count
    }
}

/// Filter actions by whether they are enabled.
pub fn partition_actions(actions: &[Action]) -> (Vec<&Action>, Vec<&Action>) {
    let enabled: Vec<&Action> = actions.iter().filter(|a| a.enabled).collect();
    let disabled: Vec<&Action> = actions.iter().filter(|a| !a.enabled).collect();
    (enabled, disabled)
}


// ---------------------------------------------------------------------------
// Action search and grouping utilities
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Group actions by their category name, returning a map from category string to actions.
pub fn group_by_category_name(actions: &[Action]) -> HashMap<String, Vec<&Action>> {
    let mut groups: HashMap<String, Vec<&Action>> = HashMap::new();
    for action in actions {
        groups.entry(action.category.to_string()).or_default().push(action);
    }
    groups
}

/// Search actions by a case-insensitive query matching label or id. Returns references.
pub fn find_actions_matching<'a>(actions: &'a [Action], query: &str) -> Vec<&'a Action> {
    let q = query.to_lowercase();
    actions
        .iter()
        .filter(|a| a.label.to_lowercase().contains(&q) || a.id.to_lowercase().contains(&q))
        .collect()
}

/// Return actions sorted by label alphabetically.
pub fn sort_by_label(actions: &mut [Action]) {
    actions.sort_by(|a, b| a.label.cmp(&b.label));
}

/// Return all distinct categories present in the given actions.
pub fn distinct_categories(actions: &[Action]) -> Vec<ActionCategory> {
    let mut cats: Vec<ActionCategory> = actions.iter().map(|a| a.category).collect();
    cats.sort_by_key(|c| format!("{c}"));
    cats.dedup();
    cats
}

/// Count actions with keybindings vs. without.
pub fn keybinding_stats(actions: &[Action]) -> (usize, usize) {
    let with = actions.iter().filter(|a| a.keybinding.is_some()).count();
    (with, actions.len() - with)
}

/// Produce a multi-line summary string of actions grouped by category.
pub fn format_action_list(actions: &[Action]) -> String {
    let mut groups: HashMap<String, Vec<&Action>> = HashMap::new();
    for action in actions {
        groups
            .entry(action.category.to_string())
            .or_default()
            .push(action);
    }
    let mut keys: Vec<&String> = groups.keys().collect();
    keys.sort();
    let mut out = String::new();
    for key in keys {
        out.push_str(&format!("[{}]\n", key));
        for a in &groups[key] {
            out.push_str(&format!("  {}\n", a));
        }
    }
    out
}

/// Create a duplicate of an action with a new id and label prefix.
pub fn clone_action_with_prefix(action: &Action, prefix: &str) -> Action {
    Action {
        id: format!("{}.{}", prefix, action.id),
        label: format!("{}: {}", prefix, action.label),
        category: action.category,
        keybinding: action.keybinding.clone(),
        precondition: action.precondition.clone(),
        enabled: action.enabled,
    }
}

impl ActionRegistry {
    /// Return a summary of keybinding coverage.
    pub fn keybinding_coverage(&self) -> String {
        let (with, without) = keybinding_stats(&self.actions);
        format!("{} with keybindings, {} without", with, without)
    }
}


// ---------------------------------------------------------------------------
// Condition evaluation for action preconditions
// ---------------------------------------------------------------------------

/// Evaluates context-key conditions to determine whether an action should be
/// enabled. Conditions are expressions like `"editorTextFocus && !inDebugMode"`
/// where each token is a boolean context key optionally negated with `!`.
/// Tokens are combined with `&&` (all must be true) or `||` (any must be true).
/// Mixed operators are evaluated left-to-right (no precedence).
pub struct ActionConditionEvaluator {
    context: HashMap<String, bool>,
}

impl ActionConditionEvaluator {
    /// Create an evaluator with an empty context.
    pub fn new() -> Self {
        Self {
            context: HashMap::new(),
        }
    }

    /// Create an evaluator pre-populated with context keys.
    pub fn with_context(context: HashMap<String, bool>) -> Self {
        Self { context }
    }

    /// Set a single context key.
    pub fn set(&mut self, key: impl Into<String>, value: bool) {
        self.context.insert(key.into(), value);
    }

    /// Remove a context key. Returns the previous value if present.
    pub fn unset(&mut self, key: &str) -> Option<bool> {
        self.context.remove(key)
    }

    /// Get the value of a context key. Missing keys are treated as `false`.
    pub fn get(&self, key: &str) -> bool {
        self.context.get(key).copied().unwrap_or(false)
    }

    /// Evaluate a single token, handling `!` negation.
    fn eval_token(&self, token: &str) -> bool {
        let trimmed = token.trim();
        if let Some(rest) = trimmed.strip_prefix('!') {
            !self.get(rest.trim())
        } else {
            self.get(trimmed)
        }
    }

    /// Evaluate a full condition expression. Returns `true` for empty/blank
    /// expressions. Supports `&&` and `||` evaluated left-to-right.
    pub fn evaluate(&self, expression: &str) -> bool {
        let expr = expression.trim();
        if expr.is_empty() {
            return true;
        }

        // Split on `||` first – each segment is an OR-branch.
        if expr.contains("||") {
            return expr.split("||").any(|segment| self.evaluate_and_chain(segment));
        }

        self.evaluate_and_chain(expr)
    }

    /// Evaluate a chain of `&&`-separated tokens.
    fn evaluate_and_chain(&self, expr: &str) -> bool {
        expr.split("&&").all(|token| self.eval_token(token))
    }

    /// Evaluate an action's precondition. If the action has no precondition the
    /// result is `true`.
    pub fn action_enabled(&self, action: &Action) -> bool {
        match &action.precondition {
            Some(expr) => self.evaluate(expr),
            None => true,
        }
    }

    /// Return all context keys currently set.
    pub fn keys(&self) -> Vec<&str> {
        self.context.keys().map(|s| s.as_str()).collect()
    }

    /// Return the number of context keys.
    pub fn len(&self) -> usize {
        self.context.len()
    }

    /// Return `true` if no context keys are set.
    pub fn is_empty(&self) -> bool {
        self.context.is_empty()
    }
}

impl Default for ActionConditionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ActionConditionEvaluator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActionConditionEvaluator")
            .field("keys", &self.context.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Keybinding display label
// ---------------------------------------------------------------------------

/// Formats a keybinding string for platform-specific display.
///
/// Given a raw keybinding like `"Ctrl+Shift+P"`, this struct normalises
/// modifier order (Ctrl → Shift → Alt → Meta → key) and can produce a
/// compact display label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionKeybindingLabel {
    raw: String,
    parts: Vec<String>,
}

impl ActionKeybindingLabel {
    /// Parse a keybinding string (e.g. `"Ctrl+Shift+P"`).
    pub fn parse(raw: impl Into<String>) -> Self {
        let raw = raw.into();
        let parts: Vec<String> = raw.split('+').map(|s| s.trim().to_string()).collect();
        Self { raw, parts }
    }

    /// Return the original raw keybinding string.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Canonical modifier order index (lower = earlier).
    fn modifier_order(part: &str) -> usize {
        match part.to_lowercase().as_str() {
            "ctrl" => 0,
            "shift" => 1,
            "alt" => 2,
            "meta" | "cmd" | "super" => 3,
            _ => 4,
        }
    }

    /// Return a normalised label with modifiers in canonical order.
    pub fn normalised(&self) -> String {
        let mut sorted = self.parts.clone();
        sorted.sort_by_key(|p| Self::modifier_order(p));
        sorted.join("+")
    }

    /// Return a display label with platform-aware modifier symbols.
    /// On macOS-style: Ctrl→⌃, Shift→⇧, Alt→⌥, Meta/Cmd→⌘.
    pub fn display_mac(&self) -> String {
        let mut sorted = self.parts.clone();
        sorted.sort_by_key(|p| Self::modifier_order(p));
        sorted
            .iter()
            .map(|p| match p.to_lowercase().as_str() {
                "ctrl" => "⌃".to_string(),
                "shift" => "⇧".to_string(),
                "alt" => "⌥".to_string(),
                "meta" | "cmd" | "super" => "⌘".to_string(),
                other => other.to_uppercase(),
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Return a display label for Linux/Windows (modifiers spelled out).
    pub fn display_standard(&self) -> String {
        self.normalised()
    }

    /// Return the number of parts (modifiers + key).
    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    /// Check whether the keybinding contains a specific modifier (case-insensitive).
    pub fn has_modifier(&self, modifier: &str) -> bool {
        let lower = modifier.to_lowercase();
        self.parts.iter().any(|p| p.to_lowercase() == lower)
    }
}

impl fmt::Display for ActionKeybindingLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.normalised())
    }
}

// ---------------------------------------------------------------------------
// Category grouper
// ---------------------------------------------------------------------------

/// Groups actions by category and returns them in sorted order.
/// Supports filtering to only enabled actions.
pub struct ActionCategoryGrouper<'a> {
    actions: &'a [Action],
}

impl<'a> ActionCategoryGrouper<'a> {
    /// Create a grouper over a slice of actions.
    pub fn new(actions: &'a [Action]) -> Self {
        Self { actions }
    }

    /// Group all actions by category, sorted by category name, then by label.
    pub fn grouped(&self) -> Vec<(ActionCategory, Vec<&'a Action>)> {
        self.grouped_filtered(false)
    }

    /// Group only enabled actions by category, sorted by category name, then label.
    pub fn grouped_enabled(&self) -> Vec<(ActionCategory, Vec<&'a Action>)> {
        self.grouped_filtered(true)
    }

    fn grouped_filtered(&self, enabled_only: bool) -> Vec<(ActionCategory, Vec<&'a Action>)> {
        let mut map: HashMap<String, (ActionCategory, Vec<&'a Action>)> = HashMap::new();
        for action in self.actions {
            if enabled_only && !action.enabled {
                continue;
            }
            let key = action.category.to_string();
            map.entry(key)
                .or_insert_with(|| (action.category, Vec::new()))
                .1
                .push(action);
        }

        let mut groups: Vec<(ActionCategory, Vec<&'a Action>)> = map.into_values().collect();
        groups.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
        for (_, actions) in &mut groups {
            actions.sort_by(|a, b| a.label.cmp(&b.label));
        }
        groups
    }

    /// Return a flat sorted list of unique categories present.
    pub fn categories(&self) -> Vec<ActionCategory> {
        let mut cats: Vec<ActionCategory> = self
            .actions
            .iter()
            .map(|a| a.category)
            .collect::<Vec<_>>();
        cats.sort_by_key(|c| c.to_string());
        cats.dedup();
        cats
    }

    /// Return the number of non-empty groups.
    pub fn group_count(&self) -> usize {
        self.categories().len()
    }

    /// Return a formatted string showing categories and their action counts.
    pub fn summary(&self) -> String {
        let groups = self.grouped();
        groups
            .iter()
            .map(|(cat, actions)| format!("{}: {}", cat, actions.len()))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

// ---------------------------------------------------------------------------
// Fuzzy search
// ---------------------------------------------------------------------------

/// A scored fuzzy-search result.
#[derive(Debug, Clone)]
pub struct FuzzyMatch<'a> {
    /// The matched action.
    pub action: &'a Action,
    /// Higher is better.
    pub score: i32,
}

/// Perform a fuzzy search over actions by matching `query` characters in order
/// against both the action label and id. Returns results sorted by descending
/// score. A score of 0 or below means no match.
///
/// Scoring:
/// - Each matched character in sequence: +1
/// - Consecutive matched characters bonus: +2
/// - Prefix match (label or id starts with query): +5
/// - Exact substring match: +10
pub fn action_fuzzy_search<'a>(actions: &'a [Action], query: &str) -> Vec<FuzzyMatch<'a>> {
    if query.is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();
    let mut results: Vec<FuzzyMatch<'a>> = Vec::new();

    for action in actions {
        let label_score = fuzzy_score(&action.label, &query_lower);
        let id_score = fuzzy_score(&action.id, &query_lower);
        let best = label_score.max(id_score);
        if best > 0 {
            results.push(FuzzyMatch {
                action,
                score: best,
            });
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    results
}

/// Compute a fuzzy match score for `query` against `haystack`.
fn fuzzy_score(haystack: &str, query: &str) -> i32 {
    let hay_lower = haystack.to_lowercase();

    // Exact substring bonus
    let mut score: i32 = 0;
    if hay_lower.contains(query) {
        score += 10;
    }

    // Prefix bonus
    if hay_lower.starts_with(query) {
        score += 5;
    }

    // Sequential character matching
    let mut hay_chars = hay_lower.chars().peekable();
    let mut consecutive = 0i32;
    let mut matched = 0i32;
    let mut last_matched = false;

    for qc in query.chars() {
        let mut found = false;
        for hc in hay_chars.by_ref() {
            if hc == qc {
                found = true;
                matched += 1;
                if last_matched {
                    consecutive += 2;
                }
                last_matched = true;
                break;
            }
            last_matched = false;
        }
        if !found {
            // Query character not found – no match via sequential path.
            return score;
        }
    }

    score + matched + consecutive
}

// ---------------------------------------------------------------------------
// ActionPrecondition
// ---------------------------------------------------------------------------

/// Conditions that must be met before an action can execute.
#[derive(Debug, Clone)]
pub struct ActionPrecondition {
    required_context_keys: Vec<String>,
    forbidden_context_keys: Vec<String>,
    needs_focus: bool,
    needs_visible: bool,
}

impl ActionPrecondition {
    pub fn new() -> Self {
        Self {
            required_context_keys: Vec::new(),
            forbidden_context_keys: Vec::new(),
            needs_focus: false,
            needs_visible: false,
        }
    }

    pub fn require_key(mut self, key: &str) -> Self {
        self.required_context_keys.push(key.to_string());
        self
    }

    pub fn forbid_key(mut self, key: &str) -> Self {
        self.forbidden_context_keys.push(key.to_string());
        self
    }

    pub fn has_focus(mut self) -> Self {
        self.needs_focus = true;
        self
    }

    pub fn is_visible(mut self) -> Self {
        self.needs_visible = true;
        self
    }

    pub fn all_met(&self, context: &[&str], focused: bool, visible: bool) -> bool {
        if self.needs_focus && !focused {
            return false;
        }
        if self.needs_visible && !visible {
            return false;
        }
        for key in &self.required_context_keys {
            if !context.contains(&key.as_str()) {
                return false;
            }
        }
        for key in &self.forbidden_context_keys {
            if context.contains(&key.as_str()) {
                return false;
            }
        }
        true
    }

    pub fn any_met(&self, context: &[&str], focused: bool, visible: bool) -> bool {
        if self.needs_focus && focused {
            return true;
        }
        if self.needs_visible && visible {
            return true;
        }
        for key in &self.required_context_keys {
            if context.contains(&key.as_str()) {
                return true;
            }
        }
        false
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.needs_focus {
            parts.push("focused".to_string());
        }
        if self.needs_visible {
            parts.push("visible".to_string());
        }
        for k in &self.required_context_keys {
            parts.push(format!("require({k})"));
        }
        for k in &self.forbidden_context_keys {
            parts.push(format!("forbid({k})"));
        }
        parts.join(" && ")
    }
}

// ---------------------------------------------------------------------------
// ActionKeybindingHint
// ---------------------------------------------------------------------------

/// Format keybinding hints for display.
#[derive(Debug, Clone)]
pub struct ActionKeybindingHint {
    modifiers: Vec<String>,
    key: String,
    is_mac: bool,
}

impl ActionKeybindingHint {
    pub fn new(key: &str, is_mac: bool) -> Self {
        Self {
            modifiers: Vec::new(),
            key: key.to_string(),
            is_mac,
        }
    }

    pub fn ctrl(mut self) -> Self {
        self.modifiers.push(if self.is_mac { "⌘".into() } else { "Ctrl".into() });
        self
    }

    pub fn shift(mut self) -> Self {
        self.modifiers.push(if self.is_mac { "⇧".into() } else { "Shift".into() });
        self
    }

    pub fn alt(mut self) -> Self {
        self.modifiers.push(if self.is_mac { "⌥".into() } else { "Alt".into() });
        self
    }

    pub fn format_shortcut(&self) -> String {
        let sep = if self.is_mac { "" } else { "+" };
        let mut parts = self.modifiers.clone();
        parts.push(self.key.clone());
        parts.join(sep)
    }

    pub fn format_chord(first: &ActionKeybindingHint, second: &ActionKeybindingHint) -> String {
        format!("{} {}", first.format_shortcut(), second.format_shortcut())
    }

    pub fn human_readable(&self) -> String {
        format!("Press {}", self.format_shortcut())
    }

    pub fn is_multi_chord(&self) -> bool {
        self.modifiers.len() > 1
    }
}

// ---------------------------------------------------------------------------
// ActionCooldown
// ---------------------------------------------------------------------------

/// Per-action cooldown tracking.
#[derive(Debug, Clone)]
pub struct ActionCooldown {
    cooldown_ms: u64,
    last_executed: Option<u64>,
}

impl ActionCooldown {
    pub fn new(cooldown_ms: u64) -> Self {
        Self {
            cooldown_ms,
            last_executed: None,
        }
    }

    pub fn cooldown_ms(&self) -> u64 {
        self.cooldown_ms
    }

    pub fn last_executed(&self) -> Option<u64> {
        self.last_executed
    }

    pub fn can_execute_at(&self, now_ms: u64) -> bool {
        match self.last_executed {
            None => true,
            Some(last) => now_ms.saturating_sub(last) >= self.cooldown_ms,
        }
    }

    pub fn execute_at(&mut self, now_ms: u64) -> bool {
        if self.can_execute_at(now_ms) {
            self.last_executed = Some(now_ms);
            true
        } else {
            false
        }
    }

    pub fn remaining_ms(&self, now_ms: u64) -> u64 {
        match self.last_executed {
            None => 0,
            Some(last) => {
                let elapsed = now_ms.saturating_sub(last);
                self.cooldown_ms.saturating_sub(elapsed)
            }
        }
    }

    pub fn reset(&mut self) {
        self.last_executed = None;
    }
}



/// Workbench action configuration manager.
#[derive(Debug, Clone)]
pub struct WbActionsConfig {
    entries: Vec<WbActionsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench action entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbActionsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbActionsEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbActionsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbActionsEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbActionsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbActionsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbActionsEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbActionsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbActionsEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbActionsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Workbench action definitions — extended utilities (xe)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_act operations.
#[derive(Debug, Clone)]
pub struct XeMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XeMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for wb_act.
#[derive(Debug, Clone)]
pub struct XeRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XeRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for wb_act lookups.
#[derive(Debug, Clone)]
pub struct XeLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XeLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
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

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 21
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer21 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer21 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_21(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_21<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_21<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_21(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_21(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_action(id: &str, category: ActionCategory) -> Action {
        Action {
            id: id.to_string(),
            label: format!("Action {id}"),
            category,
            keybinding: None,
            precondition: None,
            enabled: true,
        }
    }

    #[test]
    fn register_and_query() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        reg.register(make_action("save", ActionCategory::File));
        reg.register(make_action("zoom", ActionCategory::View));
        assert_eq!(reg.action_count(), 3);
        assert_eq!(reg.get_by_category(ActionCategory::File).len(), 2);
        assert_eq!(reg.get_by_category(ActionCategory::View).len(), 1);
    }

    #[test]
    fn unregister_action() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        assert!(reg.unregister("open"));
        assert!(!reg.unregister("open"));
        assert_eq!(reg.action_count(), 0);
    }

    #[test]
    fn set_enabled() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        reg.set_enabled("open", false);
        assert!(!reg.get_action("open").unwrap().enabled);
    }

    #[test]
    fn builder_defaults() {
        let action = Action::builder("copy", "Copy", ActionCategory::Edit).build();
        assert_eq!(action.id, "copy");
        assert_eq!(action.label, "Copy");
        assert_eq!(action.category, ActionCategory::Edit);
        assert!(action.keybinding.is_none());
        assert!(action.precondition.is_none());
        assert!(action.enabled);
    }

    #[test]
    fn builder_full() {
        let action = Action::builder("paste", "Paste", ActionCategory::Edit)
            .keybinding("Ctrl+V")
            .precondition("editorFocus")
            .enabled(false)
            .build();
        assert_eq!(action.keybinding.as_deref(), Some("Ctrl+V"));
        assert_eq!(action.precondition.as_deref(), Some("editorFocus"));
        assert!(!action.enabled);
    }

    #[test]
    fn find_actions_case_insensitive() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("file.open", ActionCategory::File));
        reg.register(make_action("edit.undo", ActionCategory::Edit));
        let results = reg.find_actions("OPEN");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "file.open");
        // search by label
        let results = reg.find_actions("action edit");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn get_enabled_and_disabled() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::File));
        reg.set_enabled("b", false);
        assert_eq!(reg.get_enabled_actions().len(), 1);
        assert_eq!(reg.get_disabled_actions().len(), 1);
        assert_eq!(reg.get_disabled_actions()[0].id, "b");
    }

    #[test]
    fn has_action_check() {
        let mut reg = ActionRegistry::new();
        assert!(!reg.has_action("x"));
        reg.register(make_action("x", ActionCategory::Debug));
        assert!(reg.has_action("x"));
    }

    #[test]
    fn get_by_keybinding_lookup() {
        let mut reg = ActionRegistry::new();
        reg.register(
            Action::builder("save", "Save", ActionCategory::File)
                .keybinding("Ctrl+S")
                .build(),
        );
        reg.register(make_action("open", ActionCategory::File));
        assert_eq!(reg.get_by_keybinding("Ctrl+S").unwrap().id, "save");
        assert!(reg.get_by_keybinding("Ctrl+Z").is_none());
    }

    #[test]
    fn clear_category_removes_only_target() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::File));
        reg.register(make_action("c", ActionCategory::View));
        reg.clear_category(ActionCategory::File);
        assert_eq!(reg.action_count(), 1);
        assert_eq!(reg.get_action("c").unwrap().category, ActionCategory::View);
    }

    #[test]
    fn display_action_category() {
        assert_eq!(ActionCategory::File.to_string(), "File");
        assert_eq!(ActionCategory::Terminal.to_string(), "Terminal");
    }

    #[test]
    fn display_action_with_keybinding() {
        let action = Action::builder("save", "Save File", ActionCategory::File)
            .keybinding("Ctrl+S")
            .build();
        assert_eq!(action.to_string(), "Save File (save) [Ctrl+S]");
    }

    #[test]
    fn display_action_without_keybinding() {
        let action = Action::builder("help", "Show Help", ActionCategory::Help).build();
        assert_eq!(action.to_string(), "Show Help (help)");
    }

    #[test]
    fn action_has_keybinding() {
        let action = Action::builder("save", "Save", ActionCategory::File)
            .keybinding("Ctrl+S")
            .build();
        assert!(action.has_keybinding());
        let action2 = make_action("open", ActionCategory::File);
        assert!(!action2.has_keybinding());
    }

    #[test]
    fn action_has_precondition() {
        let action = Action::builder("paste", "Paste", ActionCategory::Edit)
            .precondition("editorFocus")
            .build();
        assert!(action.has_precondition());
        let action2 = make_action("open", ActionCategory::File);
        assert!(!action2.has_precondition());
    }

    #[test]
    fn action_clone_with_id() {
        let action = Action::builder("save", "Save", ActionCategory::File)
            .keybinding("Ctrl+S")
            .build();
        let cloned = action.clone_with_id("save_as");
        assert_eq!(cloned.id, "save_as");
        assert_eq!(cloned.label, "Save");
        assert_eq!(cloned.keybinding.as_deref(), Some("Ctrl+S"));
    }

    #[test]
    fn action_partial_eq_by_id() {
        let a1 = make_action("open", ActionCategory::File);
        let a2 = Action::builder("open", "Different Label", ActionCategory::Edit).build();
        assert_eq!(a1, a2);
    }

    #[test]
    fn try_register_duplicate() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        let result = reg.try_register(make_action("open", ActionCategory::Edit));
        assert_eq!(result, Err(ActionError::DuplicateId("open".to_string())));
    }

    #[test]
    fn try_register_success() {
        let mut reg = ActionRegistry::new();
        assert!(reg.try_register(make_action("open", ActionCategory::File)).is_ok());
        assert_eq!(reg.action_count(), 1);
    }

    #[test]
    fn try_execute_success() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        let action = reg.try_execute("open").unwrap();
        assert_eq!(action.id, "open");
    }

    #[test]
    fn try_execute_not_found() {
        let reg = ActionRegistry::new();
        assert_eq!(
            reg.try_execute("missing"),
            Err(ActionError::NotFound("missing".to_string()))
        );
    }

    #[test]
    fn try_execute_disabled() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        reg.set_enabled("open", false);
        assert_eq!(
            reg.try_execute("open"),
            Err(ActionError::Disabled("open".to_string()))
        );
    }

    #[test]
    fn action_ids_list() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::Edit));
        let ids = reg.action_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
    }

    #[test]
    fn unique_categories_list() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::File));
        reg.register(make_action("c", ActionCategory::Edit));
        let cats = reg.unique_categories();
        assert_eq!(cats.len(), 2);
    }

    #[test]
    fn count_by_category_tally() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::File));
        reg.register(make_action("c", ActionCategory::Edit));
        assert_eq!(reg.count_by_category(ActionCategory::File), 2);
        assert_eq!(reg.count_by_category(ActionCategory::Edit), 1);
        assert_eq!(reg.count_by_category(ActionCategory::Debug), 0);
    }

    #[test]
    fn actions_with_and_without_keybindings() {
        let mut reg = ActionRegistry::new();
        reg.register(
            Action::builder("save", "Save", ActionCategory::File)
                .keybinding("Ctrl+S")
                .build(),
        );
        reg.register(make_action("open", ActionCategory::File));
        assert_eq!(reg.actions_with_keybindings().len(), 1);
        assert_eq!(reg.actions_without_keybindings().len(), 1);
    }

    #[test]
    fn rename_action_label() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        assert!(reg.rename_action("open", "Open File"));
        assert_eq!(reg.get_action("open").unwrap().label, "Open File");
        assert!(!reg.rename_action("missing", "X"));
    }

    #[test]
    fn set_and_clear_keybinding() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("open", ActionCategory::File));
        assert!(reg.set_keybinding("open", "Ctrl+O"));
        assert_eq!(
            reg.get_action("open").unwrap().keybinding.as_deref(),
            Some("Ctrl+O")
        );
        assert!(reg.clear_keybinding("open"));
        assert!(reg.get_action("open").unwrap().keybinding.is_none());
    }

    #[test]
    fn registry_summary() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(
            Action::builder("b", "B", ActionCategory::Edit)
                .keybinding("Ctrl+B")
                .build(),
        );
        let summary = reg.summary();
        assert!(summary.contains("2 actions"));
        assert!(summary.contains("1 with keybindings"));
    }

    #[test]
    fn registry_display() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        let s = format!("{reg}");
        assert!(s.contains("1 actions"));
    }

    #[test]
    fn action_error_display_messages() {
        assert_eq!(
            ActionError::NotFound("x".to_string()).to_string(),
            "action not found: x"
        );
        assert_eq!(
            ActionError::Disabled("y".to_string()).to_string(),
            "action is disabled: y"
        );
        assert_eq!(
            ActionError::DuplicateId("z".to_string()).to_string(),
            "duplicate action id: z"
        );
        assert_eq!(
            ActionError::PreconditionFailed("nope".to_string()).to_string(),
            "precondition failed: nope"
        );
    }

    #[test]
    fn action_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(ActionError::NotFound("x".to_string()));
        assert_eq!(err.to_string(), "action not found: x");
    }

    #[test]
    fn wb_actions_stats_new_defaults() {
        let stats = WbActionsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_actions_stats_record_success() {
        let mut stats = WbActionsStats::new();
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
    fn wb_actions_stats_record_failure() {
        let mut stats = WbActionsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_actions_stats_reset() {
        let mut stats = WbActionsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_actions_stats_merge() {
        let mut a = WbActionsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbActionsStats::new();
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
    fn wb_actions_stats_display() {
        let mut stats = WbActionsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_actions_stats_default() {
        let stats = WbActionsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_actions_validator_accepts_valid_name() {
        let v = WbActionsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_actions_validator_rejects_empty() {
        let v = WbActionsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_actions_validator_rejects_too_long() {
        let v = WbActionsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_actions_validator_forbidden_prefix() {
        let v = WbActionsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_actions_validator_allowed_chars() {
        let v = WbActionsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_actions_validator_range() {
        let v = WbActionsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_actions_sanitize_removes_control() {
        let result = WbActionsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_actions_truncate_short_string() {
        assert_eq!(WbActionsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_actions_truncate_long_string() {
        let result = WbActionsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_actions_is_ascii_printable() {
        assert!(WbActionsValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbActionsValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- ActionBarOrientation --

    #[test]
    fn action_bar_orientation_display() {
        assert_eq!(format!("{}", ActionBarOrientation::Horizontal), "horizontal");
        assert_eq!(format!("{}", ActionBarOrientation::Vertical), "vertical");
    }

    #[test]
    fn action_bar_visible_items_sorted() {
        let mut bar = ActionBarConfig::new(ActionBarOrientation::Horizontal).with_max_visible(3);
        bar.add_item(ActionBarItem::new("c", "ic", "C").with_order(3));
        bar.add_item(ActionBarItem::new("a", "ia", "A").with_order(1));
        bar.add_item(ActionBarItem::new("b", "ib", "B").with_order(2));
        let vis = bar.visible_items();
        assert_eq!(vis.len(), 3);
        assert_eq!(vis[0].action_id, "a");
        assert_eq!(vis[1].action_id, "b");
        assert_eq!(vis[2].action_id, "c");
    }

    #[test]
    fn action_bar_overflow() {
        let mut bar = ActionBarConfig::new(ActionBarOrientation::Vertical).with_max_visible(2);
        bar.add_item(ActionBarItem::new("a", "i", "A").with_order(1));
        bar.add_item(ActionBarItem::new("b", "i", "B").with_order(2));
        bar.add_item(ActionBarItem::new("c", "i", "C").with_order(3));
        assert_eq!(bar.visible_items().len(), 2);
        assert_eq!(bar.overflow_items().len(), 1);
        assert_eq!(bar.overflow_items()[0].action_id, "c");
    }

    #[test]
    fn action_bar_hidden_items_excluded() {
        let mut bar = ActionBarConfig::new(ActionBarOrientation::Horizontal);
        bar.add_item(ActionBarItem::new("a", "i", "A"));
        bar.add_item(ActionBarItem::new("b", "i", "B").hidden());
        assert_eq!(bar.visible_items().len(), 1);
        assert_eq!(bar.item_count(), 2);
    }

    #[test]
    fn action_bar_item_builder() {
        let item = ActionBarItem::new("save", "💾", "Save").with_order(5);
        assert_eq!(item.action_id, "save");
        assert_eq!(item.order, 5);
        assert!(item.visible);
    }

    #[test]
    fn action_bar_config_defaults() {
        let bar = ActionBarConfig::new(ActionBarOrientation::Horizontal);
        assert_eq!(bar.orientation, ActionBarOrientation::Horizontal);
        assert_eq!(bar.max_visible, 8);
        assert!(bar.show_labels);
    }

    #[test]
    fn test_action_category_iter() {
        let actions = vec![
            Action { id: "a".into(), label: "Open".into(), category: ActionCategory::File, keybinding: None, precondition: None, enabled: true },
            Action { id: "b".into(), label: "Cut".into(), category: ActionCategory::Edit, keybinding: None, precondition: None, enabled: true },
            Action { id: "c".into(), label: "Save".into(), category: ActionCategory::File, keybinding: None, precondition: None, enabled: true },
        ];
        let file_actions: Vec<_> = ActionCategoryIter::new(&actions, ActionCategory::File).collect();
        assert_eq!(file_actions.len(), 2);
        assert_eq!(file_actions[0].label, "Open");
        assert_eq!(file_actions[1].label, "Save");
    }

    #[test]
    fn test_action_category_all() {
        assert_eq!(ActionCategory::all().len(), 8);
    }

    #[test]
    fn test_action_category_from_str_opt() {
        assert_eq!(ActionCategory::from_str_opt("edit"), Some(ActionCategory::Edit));
        assert_eq!(ActionCategory::from_str_opt("View"), Some(ActionCategory::View));
        assert_eq!(ActionCategory::from_str_opt("nope"), None);
    }

    #[test]
    fn test_action_match_score() {
        let action = Action { id: "x".into(), label: "Toggle Sidebar".into(), category: ActionCategory::View, keybinding: None, precondition: None, enabled: true };
        assert_eq!(action_match_score(&action, "Toggle Sidebar"), Some(100));
        assert_eq!(action_match_score(&action, "toggle"), Some(80));
        assert_eq!(action_match_score(&action, "sidebar"), Some(60));
        assert!(action_match_score(&action, "zzz").is_none());
    }

    #[test]
    fn test_search_actions() {
        let actions = vec![
            Action { id: "a".into(), label: "Open File".into(), category: ActionCategory::File, keybinding: None, precondition: None, enabled: true },
            Action { id: "b".into(), label: "Close All".into(), category: ActionCategory::File, keybinding: None, precondition: None, enabled: true },
        ];
        let results = search_actions(&actions, "open");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0);
    }

    #[test]
    fn test_group_actions_by_category() {
        let actions = vec![
            Action { id: "a".into(), label: "A".into(), category: ActionCategory::File, keybinding: None, precondition: None, enabled: true },
            Action { id: "b".into(), label: "B".into(), category: ActionCategory::Edit, keybinding: None, precondition: None, enabled: true },
            Action { id: "c".into(), label: "C".into(), category: ActionCategory::File, keybinding: None, precondition: None, enabled: true },
        ];
        let groups = group_actions_by_category(&actions);
        assert_eq!(groups["File"], 2);
        assert_eq!(groups["Edit"], 1);
    }

    #[test]
    fn test_validate_action_id() {
        assert!(validate_action_id("editor.fold").is_ok());
        assert!(validate_action_id("").is_err());
        assert!(validate_action_id("Has Caps").is_err());
    }

    #[test]
    fn test_actions_with_keybindings() {
        let actions = vec![
            Action { id: "a".into(), label: "A".into(), category: ActionCategory::File, keybinding: Some("ctrl+s".into()), precondition: None, enabled: true },
            Action { id: "b".into(), label: "B".into(), category: ActionCategory::Edit, keybinding: None, precondition: None, enabled: true },
        ];
        let with_kb = actions_with_keybindings(&actions);
        assert_eq!(with_kb.len(), 1);
        assert_eq!(with_kb[0].id, "a");
    }

    #[test]
    fn test_action_registry_operations() {
        let mut reg = ActionRegistry::new();
        assert_eq!(reg.action_count(), 0);
        reg.register(make_action("open", ActionCategory::File));
        reg.register(make_action("save", ActionCategory::File));
        reg.register(make_action("cut", ActionCategory::Edit));
        assert_eq!(reg.action_count(), 3);
        assert!(reg.get_action("open").is_some());
        assert_eq!(reg.get_by_category(ActionCategory::File).len(), 2);
        assert!(reg.unregister("cut"));
        assert_eq!(reg.action_count(), 2);
        assert!(!reg.unregister("missing"));
    }

    #[test]
    fn test_action_registry_find_by_keybinding() {
        let mut reg = ActionRegistry::new();
        let mut action = make_action("save", ActionCategory::File);
        action.keybinding = Some("ctrl+s".into());
        reg.register(action);
        assert!(reg.get_by_keybinding("ctrl+s").is_some());
        assert!(reg.get_by_keybinding("ctrl+x").is_none());
    }

    #[test]
    fn test_action_registry_has_action() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("x", ActionCategory::File));
        assert!(reg.has_action("x"));
        assert!(!reg.has_action("y"));
    }

    #[test]
    fn test_action_execution_result_display() {
        assert_eq!(format!("{}", ActionExecutionResult::Success), "success");
        assert!(ActionExecutionResult::Success.is_success());
        assert!(ActionExecutionResult::Failure("err".into()).is_failure());
        assert_eq!(format!("{}", ActionExecutionResult::NotFound("x".into())), "action 'x' not found");
    }

    #[test]
    fn test_action_history_operations() {
        let mut hist = ActionHistory::new(3);
        assert!(hist.is_empty());
        hist.push("open", ActionExecutionResult::Success);
        hist.push("save", ActionExecutionResult::Success);
        hist.push("cut", ActionExecutionResult::Failure("err".into()));
        assert_eq!(hist.len(), 3);
        assert_eq!(hist.success_count(), 2);
        assert_eq!(hist.last().unwrap().action_id, "cut");
        // Overflow causes oldest to be removed
        hist.push("paste", ActionExecutionResult::Success);
        assert_eq!(hist.len(), 3);
        assert!(format!("{hist}").contains("3 entries"));
    }

    #[test]
    fn test_action_history_clear() {
        let mut hist = ActionHistory::new(10);
        hist.push("a", ActionExecutionResult::Success);
        hist.push("b", ActionExecutionResult::Cancelled);
        hist.clear();
        assert!(hist.is_empty());
    }

    #[test]
    fn history_failure_and_cancelled_count() {
        let mut hist = ActionHistory::new(10);
        hist.push("a", ActionExecutionResult::Success);
        hist.push("b", ActionExecutionResult::Failure("err".into()));
        hist.push("c", ActionExecutionResult::Cancelled);
        hist.push("d", ActionExecutionResult::Failure("err2".into()));
        assert_eq!(hist.failure_count(), 2);
        assert_eq!(hist.cancelled_count(), 1);
    }

    #[test]
    fn history_last_action_id() {
        let mut hist = ActionHistory::new(10);
        assert_eq!(hist.last_action_id(), None);
        hist.push("my.action", ActionExecutionResult::Success);
        assert_eq!(hist.last_action_id(), Some("my.action"));
    }

    #[test]
    fn history_contains_action() {
        let mut hist = ActionHistory::new(10);
        hist.push("action.a", ActionExecutionResult::Success);
        assert!(hist.contains_action("action.a"));
        assert!(!hist.contains_action("action.b"));
    }

    #[test]
    fn history_success_rate() {
        let mut hist = ActionHistory::new(10);
        hist.push("a", ActionExecutionResult::Success);
        hist.push("b", ActionExecutionResult::Success);
        hist.push("c", ActionExecutionResult::Failure("err".into()));
        hist.push("d", ActionExecutionResult::Cancelled);
        assert!((hist.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_action_labels() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::Edit));
        let labels = reg.action_labels();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn registry_find_by_precondition() {
        let mut reg = ActionRegistry::new();
        let mut a = make_action("a", ActionCategory::File);
        a.precondition = Some("editorIsOpen".into());
        reg.register(a);
        reg.register(make_action("b", ActionCategory::Edit));
        let found = reg.find_by_precondition("editor");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn registry_disable_category_and_enable_all() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::File));
        reg.register(make_action("c", ActionCategory::Edit));
        let disabled = reg.disable_category(ActionCategory::File);
        assert_eq!(disabled, 2);
        assert_eq!(reg.get_disabled_actions().len(), 2);
        let enabled = reg.enable_all();
        assert_eq!(enabled, 2);
        assert_eq!(reg.get_enabled_actions().len(), 3);
    }

    #[test]
    fn partition_actions_splits() {
        let actions = vec![
            make_action("a", ActionCategory::File),
            {
                let mut a = make_action("b", ActionCategory::Edit);
                a.enabled = false;
                a
            },
        ];
        let (enabled, disabled) = partition_actions(&actions);
        assert_eq!(enabled.len(), 1);
        assert_eq!(disabled.len(), 1);
    }

    #[test]
    fn group_by_category_name_groups() {
        let actions = vec![
            make_action("a", ActionCategory::File),
            make_action("b", ActionCategory::File),
            make_action("c", ActionCategory::Edit),
        ];
        let groups = group_by_category_name(&actions);
        assert_eq!(groups["File"].len(), 2);
        assert_eq!(groups["Edit"].len(), 1);
    }

    #[test]
    fn find_actions_matching_by_label() {
        let actions = vec![
            make_action("open-file", ActionCategory::File),
            make_action("save-file", ActionCategory::File),
            make_action("zoom-in", ActionCategory::View),
        ];
        let results = find_actions_matching(&actions, "file");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn find_actions_matching_by_id() {
        let actions = vec![
            make_action("open-file", ActionCategory::File),
            make_action("zoom-in", ActionCategory::View),
        ];
        let results = find_actions_matching(&actions, "zoom");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn sort_by_label_sorts() {
        let mut actions = vec![
            make_action("z", ActionCategory::File),
            make_action("a", ActionCategory::File),
            make_action("m", ActionCategory::File),
        ];
        sort_by_label(&mut actions);
        assert_eq!(actions[0].id, "a");
        assert_eq!(actions[2].id, "z");
    }

    #[test]
    fn distinct_categories_deduplicates() {
        let actions = vec![
            make_action("a", ActionCategory::File),
            make_action("b", ActionCategory::File),
            make_action("c", ActionCategory::Edit),
        ];
        let cats = distinct_categories(&actions);
        assert_eq!(cats.len(), 2);
    }

    #[test]
    fn keybinding_stats_counts() {
        let actions = vec![
            Action::builder("a", "A", ActionCategory::File)
                .keybinding("Ctrl+A")
                .build(),
            make_action("b", ActionCategory::File),
        ];
        let (with, without) = keybinding_stats(&actions);
        assert_eq!(with, 1);
        assert_eq!(without, 1);
    }

    #[test]
    fn format_action_list_contains_categories() {
        let actions = vec![
            make_action("save", ActionCategory::File),
            make_action("undo", ActionCategory::Edit),
        ];
        let output = format_action_list(&actions);
        assert!(output.contains("[File]"));
        assert!(output.contains("[Edit]"));
    }

    #[test]
    fn clone_action_with_prefix_works() {
        let original = make_action("save", ActionCategory::File);
        let cloned = clone_action_with_prefix(&original, "ext");
        assert_eq!(cloned.id, "ext.save");
        assert!(cloned.label.starts_with("ext:"));
        assert_eq!(cloned.category, ActionCategory::File);
    }

    #[test]
    fn registry_disable_category_existing() {
        let mut reg = ActionRegistry::new();
        reg.register(make_action("a", ActionCategory::File));
        reg.register(make_action("b", ActionCategory::File));
        reg.register(make_action("c", ActionCategory::Edit));
        let disabled = reg.disable_category(ActionCategory::File);
        assert_eq!(disabled, 2);
        assert_eq!(reg.get_enabled_actions().len(), 1);
        let enabled = reg.enable_all();
        assert_eq!(enabled, 2);
        assert_eq!(reg.get_enabled_actions().len(), 3);
    }

    #[test]
    fn registry_keybinding_coverage() {
        let mut reg = ActionRegistry::new();
        reg.register(
            Action::builder("a", "A", ActionCategory::File)
                .keybinding("Ctrl+S")
                .build(),
        );
        reg.register(make_action("b", ActionCategory::File));
        let coverage = reg.keybinding_coverage();
        assert!(coverage.contains("1 with"));
        assert!(coverage.contains("1 without"));
    }

    // -----------------------------------------------------------------------
    // ActionConditionEvaluator tests
    // -----------------------------------------------------------------------

    #[test]
    fn condition_eval_empty_expression() {
        let eval = ActionConditionEvaluator::new();
        assert!(eval.evaluate(""));
        assert!(eval.evaluate("   "));
    }

    #[test]
    fn condition_eval_single_key() {
        let mut eval = ActionConditionEvaluator::new();
        eval.set("editorTextFocus", true);
        assert!(eval.evaluate("editorTextFocus"));
        assert!(!eval.evaluate("inDebugMode"));
    }

    #[test]
    fn condition_eval_negation() {
        let mut eval = ActionConditionEvaluator::new();
        eval.set("inDebugMode", false);
        assert!(eval.evaluate("!inDebugMode"));
        eval.set("inDebugMode", true);
        assert!(!eval.evaluate("!inDebugMode"));
    }

    #[test]
    fn condition_eval_and_chain() {
        let mut eval = ActionConditionEvaluator::new();
        eval.set("editorTextFocus", true);
        eval.set("inDebugMode", false);
        assert!(eval.evaluate("editorTextFocus && !inDebugMode"));
        eval.set("editorTextFocus", false);
        assert!(!eval.evaluate("editorTextFocus && !inDebugMode"));
    }

    #[test]
    fn condition_eval_or_chain() {
        let mut eval = ActionConditionEvaluator::new();
        eval.set("a", false);
        eval.set("b", true);
        assert!(eval.evaluate("a || b"));
        eval.set("b", false);
        assert!(!eval.evaluate("a || b"));
    }

    #[test]
    fn condition_eval_action_no_precondition() {
        let eval = ActionConditionEvaluator::new();
        let action = make_action("test.nopre", ActionCategory::Edit);
        assert!(eval.action_enabled(&action));
    }

    #[test]
    fn condition_eval_action_with_precondition() {
        let mut eval = ActionConditionEvaluator::new();
        eval.set("editorTextFocus", true);
        let action = Action::builder("test.pre", "Test", ActionCategory::Edit)
            .precondition("editorTextFocus")
            .build();
        assert!(eval.action_enabled(&action));
        eval.set("editorTextFocus", false);
        assert!(!eval.action_enabled(&action));
    }

    #[test]
    fn condition_eval_unset_and_len() {
        let mut eval = ActionConditionEvaluator::new();
        assert!(eval.is_empty());
        eval.set("a", true);
        eval.set("b", false);
        assert_eq!(eval.len(), 2);
        assert_eq!(eval.unset("a"), Some(true));
        assert_eq!(eval.len(), 1);
        assert_eq!(eval.unset("missing"), None);
    }

    // -----------------------------------------------------------------------
    // ActionKeybindingLabel tests
    // -----------------------------------------------------------------------

    #[test]
    fn keybinding_label_normalised_order() {
        let kb = ActionKeybindingLabel::parse("Shift+Ctrl+P");
        assert_eq!(kb.normalised(), "Ctrl+Shift+P");
        assert_eq!(kb.part_count(), 3);
    }

    #[test]
    fn keybinding_label_mac_display() {
        let kb = ActionKeybindingLabel::parse("Ctrl+Shift+Alt+P");
        assert_eq!(kb.display_mac(), "⌃⇧⌥P");
    }

    #[test]
    fn keybinding_label_has_modifier() {
        let kb = ActionKeybindingLabel::parse("Ctrl+S");
        assert!(kb.has_modifier("ctrl"));
        assert!(kb.has_modifier("Ctrl"));
        assert!(!kb.has_modifier("Shift"));
    }

    #[test]
    fn keybinding_label_display_trait() {
        let kb = ActionKeybindingLabel::parse("Alt+Ctrl+Z");
        assert_eq!(format!("{kb}"), "Ctrl+Alt+Z");
    }

    // -----------------------------------------------------------------------
    // ActionCategoryGrouper tests
    // -----------------------------------------------------------------------

    #[test]
    fn category_grouper_basic() {
        let actions = vec![
            make_action("f1", ActionCategory::File),
            make_action("e1", ActionCategory::Edit),
            make_action("f2", ActionCategory::File),
        ];
        let grouper = ActionCategoryGrouper::new(&actions);
        let groups = grouper.grouped();
        assert_eq!(groups.len(), 2);
        // Groups should be sorted: Edit before File
        assert_eq!(groups[0].0, ActionCategory::Edit);
        assert_eq!(groups[1].0, ActionCategory::File);
        assert_eq!(groups[1].1.len(), 2);
    }

    #[test]
    fn category_grouper_enabled_only() {
        let actions = vec![
            Action::builder("e1", "Edit One", ActionCategory::Edit)
                .enabled(false)
                .build(),
            make_action("e2", ActionCategory::Edit),
            make_action("f1", ActionCategory::File),
        ];
        let grouper = ActionCategoryGrouper::new(&actions);
        let groups = grouper.grouped_enabled();
        // Edit group should have only 1 (enabled) action
        let edit_group = groups.iter().find(|(c, _)| *c == ActionCategory::Edit).unwrap();
        assert_eq!(edit_group.1.len(), 1);
        assert_eq!(edit_group.1[0].id, "e2");
    }

    #[test]
    fn category_grouper_summary() {
        let actions = vec![
            make_action("f1", ActionCategory::File),
            make_action("e1", ActionCategory::Edit),
        ];
        let grouper = ActionCategoryGrouper::new(&actions);
        let summary = grouper.summary();
        assert!(summary.contains("Edit: 1"));
        assert!(summary.contains("File: 1"));
    }

    // -----------------------------------------------------------------------
    // Fuzzy search tests
    // -----------------------------------------------------------------------

    #[test]
    fn fuzzy_search_basic() {
        let actions = vec![
            make_action("editor.format", ActionCategory::Edit),
            make_action("file.save", ActionCategory::File),
            make_action("file.saveAll", ActionCategory::File),
        ];
        let results = action_fuzzy_search(&actions, "save");
        assert_eq!(results.len(), 2);
        // Both file.save and file.saveAll should match
        assert!(results.iter().any(|r| r.action.id == "file.save"));
        assert!(results.iter().any(|r| r.action.id == "file.saveAll"));
    }

    #[test]
    fn fuzzy_search_empty_query() {
        let actions = vec![make_action("a", ActionCategory::Edit)];
        let results = action_fuzzy_search(&actions, "");
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_prefix_bonus() {
        let actions = vec![
            Action::builder("other", "XsaveY", ActionCategory::File).build(),
            Action::builder("file.save", "Save File", ActionCategory::File).build(),
        ];
        let results = action_fuzzy_search(&actions, "save");
        // "Save File" should rank higher due to prefix match on label
        assert_eq!(results[0].action.id, "file.save");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn fuzzy_search_no_match() {
        let actions = vec![make_action("editor.format", ActionCategory::Edit)];
        let results = action_fuzzy_search(&actions, "zzz");
        assert!(results.is_empty());
    }

    // -- ActionPrecondition ------------------------------------------------

    #[test]
    fn precondition_all_met_simple() {
        let pre = ActionPrecondition::new().require_key("editorFocus");
        assert!(pre.all_met(&["editorFocus"], false, false));
        assert!(!pre.all_met(&["terminalFocus"], false, false));
    }

    #[test]
    fn precondition_forbidden_key() {
        let pre = ActionPrecondition::new().forbid_key("readOnly");
        assert!(pre.all_met(&["editorFocus"], false, false));
        assert!(!pre.all_met(&["readOnly"], false, false));
    }

    #[test]
    fn precondition_focus_required() {
        let pre = ActionPrecondition::new().has_focus();
        assert!(!pre.all_met(&[], false, false));
        assert!(pre.all_met(&[], true, false));
    }

    #[test]
    fn precondition_any_met() {
        let pre = ActionPrecondition::new().require_key("a").require_key("b");
        assert!(pre.any_met(&["b"], false, false));
        assert!(!pre.any_met(&["c"], false, false));
    }

    #[test]
    fn precondition_describe() {
        let pre = ActionPrecondition::new().has_focus().require_key("editor");
        let desc = pre.describe();
        assert!(desc.contains("focused"));
        assert!(desc.contains("require(editor)"));
    }

    // -- ActionKeybindingHint ----------------------------------------------

    #[test]
    fn keybinding_format_windows() {
        let hint = ActionKeybindingHint::new("S", false).ctrl().shift();
        assert_eq!(hint.format_shortcut(), "Ctrl+Shift+S");
    }

    #[test]
    fn keybinding_format_mac() {
        let hint = ActionKeybindingHint::new("S", true).ctrl().shift();
        assert_eq!(hint.format_shortcut(), "⌘⇧S");
    }

    #[test]
    fn keybinding_chord() {
        let first = ActionKeybindingHint::new("K", false).ctrl();
        let second = ActionKeybindingHint::new("C", false).ctrl();
        assert_eq!(ActionKeybindingHint::format_chord(&first, &second), "Ctrl+K Ctrl+C");
    }

    #[test]
    fn keybinding_human_readable() {
        let hint = ActionKeybindingHint::new("Z", false).ctrl();
        assert_eq!(hint.human_readable(), "Press Ctrl+Z");
    }

    // -- ActionCooldown ----------------------------------------------------

    #[test]
    fn cooldown_initial_can_execute() {
        let cd = ActionCooldown::new(1000);
        assert!(cd.can_execute_at(0));
        assert_eq!(cd.remaining_ms(0), 0);
    }

    #[test]
    fn cooldown_blocks_during_period() {
        let mut cd = ActionCooldown::new(1000);
        assert!(cd.execute_at(100));
        assert!(!cd.can_execute_at(500));
        assert_eq!(cd.remaining_ms(500), 600);
    }

    #[test]
    fn cooldown_allows_after_period() {
        let mut cd = ActionCooldown::new(1000);
        cd.execute_at(100);
        assert!(cd.can_execute_at(1200));
    }

    #[test]
    fn cooldown_reset() {
        let mut cd = ActionCooldown::new(1000);
        cd.execute_at(100);
        cd.reset();
        assert!(cd.can_execute_at(100));
        assert_eq!(cd.last_executed(), None);
    }


    #[test]
    fn wb_actions_entry_creation() {
        let e = WbActionsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_actions_entry_with_priority() {
        let e = WbActionsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_actions_entry_metadata() {
        let e = WbActionsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_actions_entry_remove_meta() {
        let mut e = WbActionsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_actions_entry_activate_deactivate() {
        let mut e = WbActionsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_actions_config_add_sorted() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("lo", "Lo").with_priority(1));
        c.add(WbActionsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_actions_config_capacity() {
        let mut c = WbActionsConfig::new(1);
        assert!(c.add(WbActionsEntry::new("a", "A")));
        assert!(!c.add(WbActionsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_actions_config_remove() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_actions_config_get() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_actions_config_active_entries() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A"));
        c.add(WbActionsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_actions_config_enable_disable() {
        let mut c = WbActionsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_actions_config_clear() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_actions_config_find_by_label() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_actions_config_top_n() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A").with_priority(1));
        c.add(WbActionsEntry::new("b", "B").with_priority(2));
        c.add(WbActionsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_actions_config_deactivate_activate_all() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A"));
        c.add(WbActionsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_actions_config_highest_priority() {
        let mut c = WbActionsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbActionsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_actions_config_contains() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_actions_config_labels() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "Alpha"));
        c.add(WbActionsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_actions_config_drain_inactive() {
        let mut c = WbActionsConfig::new(10);
        c.add(WbActionsEntry::new("a", "A"));
        c.add(WbActionsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xe_metrics_empty() {
        let m = XeMetrics::new("wb_act");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xe_metrics_record_and_mean() {
        let mut m = XeMetrics::new("wb_act");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xe_metrics_min_max() {
        let mut m = XeMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xe_metrics_variance_and_std() {
        let mut m = XeMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn xe_metrics_percentile() {
        let mut m = XeMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xe_metrics_merge() {
        let mut a = XeMetrics::new("a");
        a.record(1.0);
        let mut b = XeMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xe_metrics_reset() {
        let mut m = XeMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xe_rate_window_empty() {
        let rw = XeRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xe_rate_window_tick_and_rate() {
        let mut rw = XeRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xe_lru_cache_basic() {
        let mut c = XeLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xe_lru_cache_contains_and_keys() {
        let mut c = XeLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xe_lru_cache_remove() {
        let mut c = XeLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xe_metrics_sum() {
        let mut m = XeMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xe_metrics_label() {
        let m = XeMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xe_lru_cache_clear() {
        let mut c = XeLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_21_push_and_len() {
        let mut rb = super::XbRingBuffer21::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_21_overwrite() {
        let mut rb = super::XbRingBuffer21::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_21_get_out_of_bounds() {
        let rb = super::XbRingBuffer21::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_21_drain_all() {
        let mut rb = super::XbRingBuffer21::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_21_peek_front_back() {
        let mut rb = super::XbRingBuffer21::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_21_clear() {
        let mut rb = super::XbRingBuffer21::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_21_capacity() {
        let rb = super::XbRingBuffer21::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_21_basic() {
        let h = super::xb_fnv1a_21(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_21(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_21_different_inputs() {
        let h1 = super::xb_fnv1a_21(b"abc");
        let h2 = super::xb_fnv1a_21(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_21_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_21(&data);
        let dec = super::xb_rle_decode_21(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_21_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_21(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_21(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_21_values() {
        assert!((super::xb_clamp_21(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_21(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_21(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_21_values() {
        assert!((super::xb_lerp_21(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_21(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_21(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_21_wrap_around_twice() {
        let mut rb = super::XbRingBuffer21::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }

}
