//! Action system with menu contributions.
//!
//! Equivalent to VS Code's `vs/platform/actions/common/actions.ts`.
//! Provides action registration with menu contributions, keybindings, and when-clause guards.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use vsedit_commands::{CommandHandler, CommandRegistration, CommandRegistry};
use vsedit_contextkey::ContextKeyExpr;

/// Errors that can occur in the action system.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionError {
    /// The action ID was not found in the registry.
    NotFound(String),
    /// The action's precondition is not satisfied.
    PreconditionFailed(String),
    /// A validation error when building an action.
    ValidationError(String),
    /// A menu item references a command that does not exist.
    OrphanedMenuItem { menu_id: MenuId, command_id: String },
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionError::NotFound(id) => write!(f, "action not found: {id}"),
            ActionError::PreconditionFailed(id) => {
                write!(f, "precondition not satisfied for action: {id}")
            }
            ActionError::ValidationError(msg) => write!(f, "validation error: {msg}"),
            ActionError::OrphanedMenuItem { menu_id, command_id } => {
                write!(f, "menu item {command_id} references unknown action in {menu_id}")
            }
        }
    }
}

impl std::error::Error for ActionError {}

/// Identifies a menu location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuId {
    CommandPalette,
    EditorContext,
    EditorTitle,
    EditorTitleContext,
    ExplorerContext,
    MenubarFile,
    MenubarEdit,
    MenubarSelection,
    MenubarView,
    MenubarGo,
    MenubarRun,
    MenubarTerminal,
    MenubarHelp,
    StatusBarItem,
    ViewTitle,
    ViewItemContext,
    SCMTitle,
    SCMContext,
    TerminalContext,
    DebugCallStackContext,
    TouchBar,
}

/// An item within a menu group.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub command_id: String,
    pub title: String,
    pub group: Option<String>,
    pub order: Option<i32>,
    pub when: Option<ContextKeyExpr>,
}

/// Metadata about a registered action.
#[derive(Clone)]
struct ActionMeta {
    title: String,
    category: Option<String>,
    tooltip: Option<String>,
    icon: Option<String>,
    precondition: Option<ContextKeyExpr>,
}

impl fmt::Display for MenuId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            MenuId::CommandPalette => "Command Palette",
            MenuId::EditorContext => "Editor Context",
            MenuId::EditorTitle => "Editor Title",
            MenuId::EditorTitleContext => "Editor Title Context",
            MenuId::ExplorerContext => "Explorer Context",
            MenuId::MenubarFile => "File",
            MenuId::MenubarEdit => "Edit",
            MenuId::MenubarSelection => "Selection",
            MenuId::MenubarView => "View",
            MenuId::MenubarGo => "Go",
            MenuId::MenubarRun => "Run",
            MenuId::MenubarTerminal => "Terminal",
            MenuId::MenubarHelp => "Help",
            MenuId::StatusBarItem => "Status Bar",
            MenuId::ViewTitle => "View Title",
            MenuId::ViewItemContext => "View Item Context",
            MenuId::SCMTitle => "SCM Title",
            MenuId::SCMContext => "SCM Context",
            MenuId::TerminalContext => "Terminal Context",
            MenuId::DebugCallStackContext => "Debug Call Stack Context",
            MenuId::TouchBar => "Touch Bar",
        };
        write!(f, "{name}")
    }
}

/// Builder for constructing `MenuItem` instances with a fluent API.
#[derive(Debug)]
pub struct MenuItemBuilder {
    command_id: String,
    title: String,
    group: Option<String>,
    order: Option<i32>,
    when: Option<ContextKeyExpr>,
}

impl MenuItemBuilder {
    pub fn new(command_id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            command_id: command_id.into(),
            title: title.into(),
            group: None,
            order: None,
            when: None,
        }
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn order(mut self, order: i32) -> Self {
        self.order = Some(order);
        self
    }

    pub fn when(mut self, expr: ContextKeyExpr) -> Self {
        self.when = Some(expr);
        self
    }

    /// Parse and set a when-clause from a string expression.
    pub fn when_expr(mut self, expr: &str) -> Result<Self, ActionError> {
        let parsed = ContextKeyExpr::parse(expr)
            .map_err(|e| ActionError::ValidationError(format!("invalid when clause: {e}")))?;
        self.when = Some(parsed);
        Ok(self)
    }

    pub fn build(self) -> Result<MenuItem, ActionError> {
        if self.command_id.is_empty() {
            return Err(ActionError::ValidationError(
                "command_id must not be empty".into(),
            ));
        }
        if self.title.is_empty() {
            return Err(ActionError::ValidationError(
                "title must not be empty".into(),
            ));
        }
        Ok(MenuItem {
            command_id: self.command_id,
            title: self.title,
            group: self.group,
            order: self.order,
            when: self.when,
        })
    }
}

/// Registry for actions (commands with UI metadata).
pub struct ActionRegistry {
    command_registry: Arc<CommandRegistry>,
    menus: RwLock<HashMap<MenuId, Vec<MenuItem>>>,
    actions: RwLock<HashMap<String, ActionMeta>>,
    _registrations: Mutex<Vec<CommandRegistration>>,
}

impl ActionRegistry {
    pub fn new(command_registry: Arc<CommandRegistry>) -> Self {
        Self {
            command_registry,
            menus: RwLock::new(HashMap::new()),
            actions: RwLock::new(HashMap::new()),
            _registrations: Mutex::new(Vec::new()),
        }
    }

    /// Register an action (command + menu contributions).
    pub fn register_action(
        &self,
        id: impl Into<String>,
        title: impl Into<String>,
        category: Option<String>,
        handler: CommandHandler,
        menu_items: Vec<(MenuId, MenuItem)>,
        precondition: Option<ContextKeyExpr>,
    ) {
        let id = id.into();
        let title = title.into();

        // Register the underlying command
        let reg = self.command_registry.register(&id, handler);
        self._registrations.lock().unwrap().push(reg);

        // Store action metadata
        {
            let mut actions = self.actions.write().unwrap();
            actions.insert(
                id.clone(),
                ActionMeta {
                    title,
                    category,
                    tooltip: None,
                    icon: None,
                    precondition,
                },
            );
        }

        // Register menu items
        {
            let mut menus = self.menus.write().unwrap();
            for (menu_id, item) in menu_items {
                menus.entry(menu_id).or_default().push(item);
            }
        }
    }

    /// Get all menu items for a given menu, sorted by group and order.
    pub fn get_menu_items(&self, menu_id: MenuId) -> Vec<MenuItem> {
        let menus = self.menus.read().unwrap();
        let mut items = menus.get(&menu_id).cloned().unwrap_or_default();
        items.sort_by(|a, b| {
            let ga = a.group.as_deref().unwrap_or("");
            let gb = b.group.as_deref().unwrap_or("");
            ga.cmp(gb)
                .then_with(|| a.order.unwrap_or(0).cmp(&b.order.unwrap_or(0)))
        });
        items
    }

    /// Get the title of a registered action.
    pub fn get_action_title(&self, id: &str) -> Option<String> {
        let actions = self.actions.read().unwrap();
        actions.get(id).map(|a| {
            if let Some(cat) = &a.category {
                format!("{cat}: {}", a.title)
            } else {
                a.title.clone()
            }
        })
    }

    /// Get all registered action IDs.
    pub fn get_action_ids(&self) -> Vec<String> {
        let actions = self.actions.read().unwrap();
        actions.keys().cloned().collect()
    }

    /// Check if an action's precondition is satisfied.
    pub fn is_action_enabled(
        &self,
        id: &str,
        context: &dyn vsedit_contextkey::IContext,
    ) -> bool {
        let actions = self.actions.read().unwrap();
        match actions.get(id) {
            Some(meta) => match &meta.precondition {
                Some(expr) => expr.evaluate(context),
                None => true,
            },
            None => false,
        }
    }

    /// Return the number of registered actions.
    pub fn action_count(&self) -> usize {
        self.actions.read().unwrap().len()
    }

    /// Return the number of menu items registered under `menu_id`.
    pub fn menu_item_count(&self, menu_id: MenuId) -> usize {
        let menus = self.menus.read().unwrap();
        menus.get(&menu_id).map_or(0, |v| v.len())
    }

    /// Check whether an action with the given `id` exists.
    pub fn has_action(&self, id: &str) -> bool {
        self.actions.read().unwrap().contains_key(id)
    }

    /// Get the category of a registered action, if any.
    pub fn get_action_category(&self, id: &str) -> Option<String> {
        self.actions.read().unwrap().get(id).and_then(|a| a.category.clone())
    }

    /// Set the tooltip for an existing action. Returns an error if the action is not found.
    pub fn set_action_tooltip(
        &self,
        id: &str,
        tooltip: impl Into<String>,
    ) -> Result<(), ActionError> {
        let mut actions = self.actions.write().unwrap();
        match actions.get_mut(id) {
            Some(meta) => {
                meta.tooltip = Some(tooltip.into());
                Ok(())
            }
            None => Err(ActionError::NotFound(id.to_string())),
        }
    }

    /// Set the icon for an existing action. Returns an error if the action is not found.
    pub fn set_action_icon(
        &self,
        id: &str,
        icon: impl Into<String>,
    ) -> Result<(), ActionError> {
        let mut actions = self.actions.write().unwrap();
        match actions.get_mut(id) {
            Some(meta) => {
                meta.icon = Some(icon.into());
                Ok(())
            }
            None => Err(ActionError::NotFound(id.to_string())),
        }
    }

    /// Execute an action if its precondition is satisfied.
    pub fn execute_if_enabled(
        &self,
        id: &str,
        context: &dyn vsedit_contextkey::IContext,
        args: vsedit_commands::CommandArgs,
    ) -> Result<Option<Box<dyn std::any::Any + Send>>, ActionError> {
        if !self.has_action(id) {
            return Err(ActionError::NotFound(id.to_string()));
        }
        if !self.is_action_enabled(id, context) {
            return Err(ActionError::PreconditionFailed(id.to_string()));
        }
        self.command_registry
            .execute(id, args)
            .map_err(|e| ActionError::ValidationError(e.to_string()))
    }

    /// Get all menu IDs that have at least one registered item.
    pub fn get_populated_menus(&self) -> Vec<MenuId> {
        let menus = self.menus.read().unwrap();
        menus
            .iter()
            .filter(|(_, items)| !items.is_empty())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Validate that every menu item references a registered action.
    pub fn validate_menu_integrity(&self) -> Vec<ActionError> {
        let menus = self.menus.read().unwrap();
        let actions = self.actions.read().unwrap();
        let mut errors = Vec::new();
        for (&menu_id, items) in menus.iter() {
            for item in items {
                if !actions.contains_key(&item.command_id) {
                    errors.push(ActionError::OrphanedMenuItem {
                        menu_id,
                        command_id: item.command_id.clone(),
                    });
                }
            }
        }
        errors
    }
}

/// Accumulated statistics for actions operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ActionsStats {
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
    pub fn merge(&mut self, other: &ActionsStats) {
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

impl Default for ActionsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ActionsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ActionsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for actions.
#[derive(Debug, Clone)]
pub struct ActionsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ActionsValidator {
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

impl Default for ActionsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Action weight / priority sorting
// ---------------------------------------------------------------------------

/// Weight assigned to an action for priority-based sorting in menus and palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionWeight {
    /// Primary sort group (lower groups appear first).
    pub group: u32,
    /// Order within the group (lower values first).
    pub order: i32,
}

impl ActionWeight {
    pub const fn new(group: u32, order: i32) -> Self {
        Self { group, order }
    }

    /// Default weight: group 0, order 0.
    pub const fn default_weight() -> Self {
        Self { group: 0, order: 0 }
    }

    /// Compare two weights for sorting.
    pub fn cmp_priority(&self, other: &Self) -> std::cmp::Ordering {
        self.group.cmp(&other.group).then(self.order.cmp(&other.order))
    }
}

impl Default for ActionWeight {
    fn default() -> Self {
        Self::default_weight()
    }
}

impl fmt::Display for ActionWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.group, self.order)
    }
}

impl PartialOrd for ActionWeight {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp_priority(other))
    }
}

impl Ord for ActionWeight {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_priority(other)
    }
}

/// A weighted action entry for sorting.
#[derive(Debug, Clone)]
pub struct WeightedAction {
    pub id: String,
    pub label: String,
    pub weight: ActionWeight,
}

impl WeightedAction {
    pub fn new(id: impl Into<String>, label: impl Into<String>, weight: ActionWeight) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            weight,
        }
    }
}

/// Sort a list of weighted actions by their weight.
pub fn sort_actions_by_weight(actions: &mut [WeightedAction]) {
    actions.sort_by(|a, b| a.weight.cmp(&b.weight));
}

/// Group weighted actions by their group number.
pub fn group_actions(actions: &[WeightedAction]) -> Vec<Vec<&WeightedAction>> {
    let mut map: std::collections::BTreeMap<u32, Vec<&WeightedAction>> = std::collections::BTreeMap::new();
    for a in actions {
        map.entry(a.weight.group).or_default().push(a);
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_handler() -> CommandHandler {
        Box::new(|_args| Ok(None))
    }

    #[test]
    fn register_action_with_menu() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds.clone());

        let item = MenuItem {
            command_id: "test.action".into(),
            title: "Test Action".into(),
            group: Some("navigation".into()),
            order: Some(1),
            when: None,
        };

        registry.register_action(
            "test.action",
            "Test Action",
            Some("Test".into()),
            noop_handler(),
            vec![(MenuId::CommandPalette, item)],
            None,
        );

        let items = registry.get_menu_items(MenuId::CommandPalette);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].command_id, "test.action");
        assert!(cmds.has("test.action"));
    }

    #[test]
    fn action_title_with_category() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);

        registry.register_action(
            "file.save",
            "Save",
            Some("File".into()),
            noop_handler(),
            vec![],
            None,
        );

        assert_eq!(
            registry.get_action_title("file.save"),
            Some("File: Save".to_string())
        );
    }

    #[test]
    fn menu_items_sorted_by_group_and_order() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);

        for (id, group, order) in [
            ("c", "z_navigation", 2),
            ("b", "a_edit", 1),
            ("a", "z_navigation", 1),
        ] {
            let item = MenuItem {
                command_id: id.into(),
                title: id.into(),
                group: Some(group.into()),
                order: Some(order),
                when: None,
            };
            registry.register_action(id, id, None, noop_handler(), vec![(MenuId::EditorContext, item)], None);
        }

        let items = registry.get_menu_items(MenuId::EditorContext);
        assert_eq!(items[0].command_id, "b"); // a_edit
        assert_eq!(items[1].command_id, "a"); // z_navigation, order 1
        assert_eq!(items[2].command_id, "c"); // z_navigation, order 2
    }

    #[test]
    fn precondition_evaluation() {
        use vsedit_contextkey::{ContextKeyExpr, ContextKeyService, ContextKeyValue, IContext};

        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);

        let when = ContextKeyExpr::parse("editorTextFocus").unwrap();

        registry.register_action("guarded", "Guarded", None, noop_handler(), vec![], Some(when));

        let ctx = ContextKeyService::new();

        // Not satisfied
        assert!(!registry.is_action_enabled("guarded", &ctx));

        // Satisfied
        ctx.set_context("editorTextFocus", ContextKeyValue::Bool(true));
        assert!(registry.is_action_enabled("guarded", &ctx));
    }

    #[test]
    fn unknown_action_not_enabled() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        let ctx = vsedit_contextkey::ContextKeyService::new();
        assert!(!registry.is_action_enabled("nonexistent", &ctx));
    }

    #[test]
    fn action_count_and_has_action() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        assert_eq!(registry.action_count(), 0);
        assert!(!registry.has_action("x"));

        registry.register_action("x", "X", None, noop_handler(), vec![], None);
        registry.register_action("y", "Y", None, noop_handler(), vec![], None);

        assert_eq!(registry.action_count(), 2);
        assert!(registry.has_action("x"));
        assert!(registry.has_action("y"));
        assert!(!registry.has_action("z"));
    }

    #[test]
    fn menu_item_count() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        assert_eq!(registry.menu_item_count(MenuId::CommandPalette), 0);

        let item = MenuItem {
            command_id: "a".into(),
            title: "A".into(),
            group: None,
            order: None,
            when: None,
        };
        registry.register_action(
            "a", "A", None, noop_handler(),
            vec![(MenuId::CommandPalette, item)], None,
        );
        assert_eq!(registry.menu_item_count(MenuId::CommandPalette), 1);
        assert_eq!(registry.menu_item_count(MenuId::EditorContext), 0);
    }

    #[test]
    fn action_title_without_category() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        registry.register_action("open", "Open", None, noop_handler(), vec![], None);
        assert_eq!(registry.get_action_title("open"), Some("Open".to_string()));
        assert_eq!(registry.get_action_title("missing"), None);
    }

    #[test]
    fn set_tooltip_and_icon() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        registry.register_action("a", "A", None, noop_handler(), vec![], None);

        assert!(registry.set_action_tooltip("a", "my tip").is_ok());
        assert!(registry.set_action_icon("a", "save-icon").is_ok());

        assert_eq!(
            registry.set_action_tooltip("missing", "x"),
            Err(ActionError::NotFound("missing".into()))
        );
        assert_eq!(
            registry.set_action_icon("missing", "x"),
            Err(ActionError::NotFound("missing".into()))
        );
    }

    #[test]
    fn get_action_category() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        registry.register_action("a", "A", Some("Cat".into()), noop_handler(), vec![], None);
        registry.register_action("b", "B", None, noop_handler(), vec![], None);

        assert_eq!(registry.get_action_category("a"), Some("Cat".to_string()));
        assert_eq!(registry.get_action_category("b"), None);
        assert_eq!(registry.get_action_category("missing"), None);
    }

    #[test]
    fn execute_if_enabled_not_found() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        let ctx = vsedit_contextkey::ContextKeyService::new();
        let result = registry.execute_if_enabled("nope", &ctx, vec![]);
        assert_eq!(result.unwrap_err(), ActionError::NotFound("nope".into()));
    }

    #[test]
    fn execute_if_enabled_precondition_failed() {
        use vsedit_contextkey::ContextKeyExpr;

        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        let when = ContextKeyExpr::parse("isDebugMode").unwrap();
        registry.register_action("dbg", "Debug", None, noop_handler(), vec![], Some(when));

        let ctx = vsedit_contextkey::ContextKeyService::new();
        let result = registry.execute_if_enabled("dbg", &ctx, vec![]);
        assert_eq!(
            result.unwrap_err(),
            ActionError::PreconditionFailed("dbg".into())
        );
    }

    #[test]
    fn execute_if_enabled_success() {
        use vsedit_contextkey::ContextKeyExpr;

        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        registry.register_action("run", "Run", None, noop_handler(), vec![], None);

        let ctx = vsedit_contextkey::ContextKeyService::new();
        let result = registry.execute_if_enabled("run", &ctx, vec![]);
        assert!(result.is_ok());
    }

    #[test]
    fn menu_item_builder_success() {
        let item = MenuItemBuilder::new("cmd.save", "Save File")
            .group("file_ops")
            .order(5)
            .build()
            .unwrap();

        assert_eq!(item.command_id, "cmd.save");
        assert_eq!(item.title, "Save File");
        assert_eq!(item.group, Some("file_ops".to_string()));
        assert_eq!(item.order, Some(5));
        assert!(item.when.is_none());
    }

    #[test]
    fn menu_item_builder_validation_errors() {
        let err = MenuItemBuilder::new("", "Title").build().unwrap_err();
        assert!(matches!(err, ActionError::ValidationError(_)));

        let err = MenuItemBuilder::new("cmd", "").build().unwrap_err();
        assert!(matches!(err, ActionError::ValidationError(_)));
    }

    #[test]
    fn menu_item_builder_with_when_expr() {
        let item = MenuItemBuilder::new("cmd.fmt", "Format")
            .when_expr("editorHasSelection")
            .unwrap()
            .build()
            .unwrap();
        assert!(item.when.is_some());

        let err = MenuItemBuilder::new("cmd.x", "X")
            .when_expr("&&")
            .unwrap_err();
        assert!(matches!(err, ActionError::ValidationError(_)));
    }

    #[test]
    fn action_error_display() {
        let e = ActionError::NotFound("x".into());
        assert_eq!(e.to_string(), "action not found: x");

        let e = ActionError::PreconditionFailed("y".into());
        assert_eq!(e.to_string(), "precondition not satisfied for action: y");

        let e = ActionError::OrphanedMenuItem {
            menu_id: MenuId::CommandPalette,
            command_id: "z".into(),
        };
        assert!(e.to_string().contains("Command Palette"));
    }

    #[test]
    fn menu_id_display() {
        assert_eq!(MenuId::CommandPalette.to_string(), "Command Palette");
        assert_eq!(MenuId::MenubarFile.to_string(), "File");
        assert_eq!(MenuId::TouchBar.to_string(), "Touch Bar");
    }

    #[test]
    fn get_populated_menus() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        assert!(registry.get_populated_menus().is_empty());

        let item = MenuItem {
            command_id: "a".into(),
            title: "A".into(),
            group: None,
            order: None,
            when: None,
        };
        registry.register_action(
            "a", "A", None, noop_handler(),
            vec![(MenuId::EditorTitle, item)], None,
        );
        let populated = registry.get_populated_menus();
        assert_eq!(populated.len(), 1);
        assert_eq!(populated[0], MenuId::EditorTitle);
    }

    #[test]
    fn actions_stats_new_defaults() {
        let stats = ActionsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn actions_stats_record_success() {
        let mut stats = ActionsStats::new();
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
    fn actions_stats_record_failure() {
        let mut stats = ActionsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn actions_stats_reset() {
        let mut stats = ActionsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn actions_stats_merge() {
        let mut a = ActionsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ActionsStats::new();
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
    fn actions_stats_display() {
        let mut stats = ActionsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn actions_stats_default() {
        let stats = ActionsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn actions_validator_accepts_valid_name() {
        let v = ActionsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn actions_validator_rejects_empty() {
        let v = ActionsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn actions_validator_rejects_too_long() {
        let v = ActionsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn actions_validator_forbidden_prefix() {
        let v = ActionsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn actions_validator_allowed_chars() {
        let v = ActionsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn actions_validator_range() {
        let v = ActionsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn actions_sanitize_removes_control() {
        let result = ActionsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn actions_truncate_short_string() {
        assert_eq!(ActionsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn actions_truncate_long_string() {
        let result = ActionsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn actions_is_ascii_printable() {
        assert!(ActionsValidator::is_ascii_printable("Hello World 123"));
        assert!(!ActionsValidator::is_ascii_printable("Hello\x00World"));
    }

    // -- ActionWeight --

    #[test]
    fn action_weight_ordering() {
        let a = ActionWeight::new(0, 1);
        let b = ActionWeight::new(0, 2);
        let c = ActionWeight::new(1, 0);
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn action_weight_display() {
        let w = ActionWeight::new(2, 5);
        assert_eq!(format!("{w}"), "2:5");
    }

    #[test]
    fn sort_actions_by_weight_order() {
        let mut actions = vec![
            WeightedAction::new("c", "C", ActionWeight::new(1, 0)),
            WeightedAction::new("a", "A", ActionWeight::new(0, 1)),
            WeightedAction::new("b", "B", ActionWeight::new(0, 5)),
        ];
        sort_actions_by_weight(&mut actions);
        assert_eq!(actions[0].id, "a");
        assert_eq!(actions[1].id, "b");
        assert_eq!(actions[2].id, "c");
    }

    #[test]
    fn group_actions_splits_by_group() {
        let actions = vec![
            WeightedAction::new("a", "A", ActionWeight::new(0, 0)),
            WeightedAction::new("b", "B", ActionWeight::new(1, 0)),
            WeightedAction::new("c", "C", ActionWeight::new(0, 1)),
        ];
        let groups = group_actions(&actions);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[1].len(), 1);
    }

    #[test]
    fn action_weight_default() {
        let w = ActionWeight::default();
        assert_eq!(w.group, 0);
        assert_eq!(w.order, 0);
    }
}
