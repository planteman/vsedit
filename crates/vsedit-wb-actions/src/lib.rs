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
}
