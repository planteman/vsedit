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
}
