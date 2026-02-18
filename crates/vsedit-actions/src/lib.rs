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

// ---------------------------------------------------------------------------
// MenuItem extensions
// ---------------------------------------------------------------------------

impl MenuItem {
    pub fn is_separator(&self) -> bool {
        self.command_id == "-" || self.title == "-"
    }

    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_ascii_lowercase();
        self.title.to_ascii_lowercase().contains(&q)
            || self.command_id.to_ascii_lowercase().contains(&q)
    }
}

impl fmt::Display for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref group) = self.group {
            write!(f, "[{}] {} ({})", group, self.title, self.command_id)
        } else {
            write!(f, "{} ({})", self.title, self.command_id)
        }
    }
}

// ---------------------------------------------------------------------------
// MenuId extensions
// ---------------------------------------------------------------------------

impl MenuId {
    pub fn is_context_menu(&self) -> bool {
        matches!(
            self,
            MenuId::EditorContext
                | MenuId::EditorTitleContext
                | MenuId::ExplorerContext
                | MenuId::ViewItemContext
                | MenuId::SCMContext
                | MenuId::TerminalContext
                | MenuId::DebugCallStackContext
        )
    }

    pub fn is_main_menu(&self) -> bool {
        matches!(
            self,
            MenuId::MenubarFile
                | MenuId::MenubarEdit
                | MenuId::MenubarSelection
                | MenuId::MenubarView
                | MenuId::MenubarGo
                | MenuId::MenubarRun
                | MenuId::MenubarTerminal
                | MenuId::MenubarHelp
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
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
        }
    }
}

// ---------------------------------------------------------------------------
// ActionWeight extensions
// ---------------------------------------------------------------------------

impl ActionWeight {
    pub fn is_high(&self) -> bool {
        self.group == 0 && self.order <= 0
    }

    pub fn is_low(&self) -> bool {
        self.group >= 100
    }
}

// ---------------------------------------------------------------------------
// WeightedAction extensions
// ---------------------------------------------------------------------------

impl fmt::Display for WeightedAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}] ({})", self.label, self.id, self.weight)
    }
}

impl PartialEq for WeightedAction {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.weight == other.weight
    }
}

impl Eq for WeightedAction {}

impl PartialOrd for WeightedAction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeightedAction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.weight.cmp(&other.weight).then(self.id.cmp(&other.id))
    }
}

// ---------------------------------------------------------------------------
// ActionRegistry extensions
// ---------------------------------------------------------------------------

impl ActionRegistry {
    pub fn is_empty(&self) -> bool {
        self.actions.read().unwrap().is_empty()
    }

    pub fn find_by_label(&self, label: &str) -> Vec<String> {
        let actions = self.actions.read().unwrap();
        actions
            .iter()
            .filter(|(_, meta)| meta.title == label)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn clear(&self) {
        self.actions.write().unwrap().clear();
        self.menus.write().unwrap().clear();
        self._registrations.lock().unwrap().clear();
    }
}

// ---------------------------------------------------------------------------
// ActionGroup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ActionGroup {
    pub name: String,
    pub actions: Vec<WeightedAction>,
}

impl ActionGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            actions: Vec::new(),
        }
    }

    pub fn add(&mut self, action: WeightedAction) {
        self.actions.push(action);
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn sorted(&self) -> Vec<&WeightedAction> {
        let mut refs: Vec<&WeightedAction> = self.actions.iter().collect();
        refs.sort();
        refs
    }
}

impl fmt::Display for ActionGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({} actions)", self.name, self.actions.len())
    }
}

// ---------------------------------------------------------------------------
// ActionsStats extensions
// ---------------------------------------------------------------------------

impl ActionsStats {
    pub fn summary(&self) -> String {
        format!(
            "{} ops, {:.1}% success, avg {}ns",
            self.total_operations,
            self.success_rate() * 100.0,
            self.average_time_ns(),
        )
    }

    pub fn has_failures(&self) -> bool {
        self.failed_operations > 0
    }
}

// ---------------------------------------------------------------------------
// Action audit log
// ---------------------------------------------------------------------------

/// Records an action execution for auditing purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEntry {
    /// The action ID that was executed.
    pub action_id: String,
    /// Whether the execution succeeded.
    pub success: bool,
    /// Monotonic sequence number assigned at recording time.
    pub seq: u64,
    /// Optional description of the outcome.
    pub detail: Option<String>,
}

/// An append-only audit log tracking action executions.
#[derive(Debug)]
pub struct ActionAuditLog {
    entries: Mutex<Vec<AuditEntry>>,
    next_seq: Mutex<u64>,
}

impl ActionAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
            next_seq: Mutex::new(1),
        }
    }

    /// Record a successful action execution.
    pub fn record_success(&self, action_id: impl Into<String>) {
        self.push(action_id.into(), true, None);
    }

    /// Record a failed action execution with a detail message.
    pub fn record_failure(&self, action_id: impl Into<String>, detail: impl Into<String>) {
        self.push(action_id.into(), false, Some(detail.into()));
    }

    fn push(&self, action_id: String, success: bool, detail: Option<String>) {
        let mut seq = self.next_seq.lock().unwrap();
        let entry = AuditEntry {
            action_id,
            success,
            seq: *seq,
            detail,
        };
        *seq += 1;
        self.entries.lock().unwrap().push(entry);
    }

    /// Return a snapshot of all entries.
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().unwrap().clone()
    }

    /// Return only the entries for a given action ID.
    pub fn entries_for(&self, action_id: &str) -> Vec<AuditEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.action_id == action_id)
            .cloned()
            .collect()
    }

    /// Return the total number of recorded entries.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// Check if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }

    /// Clear all entries and reset the sequence counter.
    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
        *self.next_seq.lock().unwrap() = 1;
    }
}

impl Default for ActionAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Action frequency tracker
// ---------------------------------------------------------------------------

/// Tracks how often each action is executed to surface the most-used actions.
#[derive(Debug)]
pub struct ActionFrequencyTracker {
    counts: Mutex<HashMap<String, u64>>,
}

impl ActionFrequencyTracker {
    pub fn new() -> Self {
        Self {
            counts: Mutex::new(HashMap::new()),
        }
    }

    /// Increment the execution count for the given action.
    pub fn record(&self, action_id: impl Into<String>) {
        let mut counts = self.counts.lock().unwrap();
        *counts.entry(action_id.into()).or_insert(0) += 1;
    }

    /// Return the execution count for a specific action.
    pub fn count(&self, action_id: &str) -> u64 {
        self.counts.lock().unwrap().get(action_id).copied().unwrap_or(0)
    }

    /// Return the top-N most frequently executed actions, sorted descending.
    pub fn top_n(&self, n: usize) -> Vec<(String, u64)> {
        let counts = self.counts.lock().unwrap();
        let mut pairs: Vec<(String, u64)> = counts.iter().map(|(k, &v)| (k.clone(), v)).collect();
        pairs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        pairs.truncate(n);
        pairs
    }

    /// Return all tracked action IDs.
    pub fn tracked_actions(&self) -> Vec<String> {
        self.counts.lock().unwrap().keys().cloned().collect()
    }

    /// Reset all counts.
    pub fn reset(&self) {
        self.counts.lock().unwrap().clear();
    }
}

impl Default for ActionFrequencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Action conflict detection
// ---------------------------------------------------------------------------

/// Detects conflicts between actions that share the same resources or keybindings.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionConflict {
    /// The first action involved in the conflict.
    pub action_a: String,
    /// The second action involved in the conflict.
    pub action_b: String,
    /// A human-readable description of the conflict.
    pub reason: String,
}

impl fmt::Display for ActionConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "conflict between '{}' and '{}': {}",
            self.action_a, self.action_b, self.reason
        )
    }
}

/// Checks a set of (action_id, keybinding) pairs for duplicate keybindings.
pub fn detect_keybinding_conflicts(bindings: &[(String, String)]) -> Vec<ActionConflict> {
    let mut by_key: HashMap<&str, Vec<&str>> = HashMap::new();
    for (action_id, key) in bindings {
        by_key.entry(key.as_str()).or_default().push(action_id.as_str());
    }
    let mut conflicts = Vec::new();
    for (key, actions) in &by_key {
        if actions.len() > 1 {
            for i in 0..actions.len() {
                for j in (i + 1)..actions.len() {
                    conflicts.push(ActionConflict {
                        action_a: actions[i].to_string(),
                        action_b: actions[j].to_string(),
                        reason: format!("duplicate keybinding '{key}'"),
                    });
                }
            }
        }
    }
    conflicts.sort_by(|a, b| a.action_a.cmp(&b.action_a).then(a.action_b.cmp(&b.action_b)));
    conflicts
}

// ---------------------------------------------------------------------------
// Undo group
// ---------------------------------------------------------------------------

/// Groups multiple action IDs into a single undo unit so that undoing reverts
/// all of them atomically.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoGroup {
    /// Human-readable label for the undo group.
    pub label: String,
    /// Ordered list of action IDs that form this group.
    pub action_ids: Vec<String>,
}

impl UndoGroup {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action_ids: Vec::new(),
        }
    }

    /// Append an action to the undo group.
    pub fn push(&mut self, action_id: impl Into<String>) {
        self.action_ids.push(action_id.into());
    }

    /// Return the number of actions in this group.
    pub fn len(&self) -> usize {
        self.action_ids.len()
    }

    /// Check if the group is empty.
    pub fn is_empty(&self) -> bool {
        self.action_ids.is_empty()
    }

    /// Check whether an action is part of this group.
    pub fn contains(&self, action_id: &str) -> bool {
        self.action_ids.iter().any(|id| id == action_id)
    }
}

impl fmt::Display for UndoGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UndoGroup '{}' ({} actions)", self.label, self.action_ids.len())
    }
}

/// Manages a stack of undo groups.
#[derive(Debug)]
pub struct UndoGroupStack {
    groups: Mutex<Vec<UndoGroup>>,
    max_depth: usize,
}

impl UndoGroupStack {
    /// Create a stack with a maximum depth. Once exceeded, the oldest group is dropped.
    pub fn new(max_depth: usize) -> Self {
        Self {
            groups: Mutex::new(Vec::new()),
            max_depth,
        }
    }

    /// Push a completed undo group onto the stack.
    pub fn push(&self, group: UndoGroup) {
        let mut groups = self.groups.lock().unwrap();
        if groups.len() >= self.max_depth {
            groups.remove(0);
        }
        groups.push(group);
    }

    /// Pop the most recent undo group, if any.
    pub fn pop(&self) -> Option<UndoGroup> {
        self.groups.lock().unwrap().pop()
    }

    /// Peek at the most recent undo group without removing it.
    pub fn peek(&self) -> Option<UndoGroup> {
        self.groups.lock().unwrap().last().cloned()
    }

    /// Return the current number of undo groups on the stack.
    pub fn len(&self) -> usize {
        self.groups.lock().unwrap().len()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.groups.lock().unwrap().is_empty()
    }

    /// Clear all undo groups.
    pub fn clear(&self) {
        self.groups.lock().unwrap().clear();
    }
}

// ---------------------------------------------------------------------------
// PreCondition checking
// ---------------------------------------------------------------------------

/// A single precondition: a context key must equal an expected value.
#[derive(Debug, Clone, PartialEq)]
pub struct PreCondition {
    pub key: String,
    pub expected_value: String,
}

/// Result of evaluating preconditions against a context.
#[derive(Debug, Clone, PartialEq)]
pub enum PreConditionResult {
    /// All preconditions are satisfied.
    Satisfied,
    /// A precondition failed.
    Failed {
        missing_key: Option<String>,
        wrong_value: Option<(String, String, String)>,
    },
}

impl fmt::Display for PreConditionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreConditionResult::Satisfied => write!(f, "all preconditions satisfied"),
            PreConditionResult::Failed {
                missing_key: Some(k),
                ..
            } => write!(f, "missing context key: {k}"),
            PreConditionResult::Failed {
                wrong_value: Some((key, expected, actual)),
                ..
            } => write!(
                f,
                "key \"{key}\": expected \"{expected}\", got \"{actual}\""
            ),
            PreConditionResult::Failed { .. } => write!(f, "precondition failed"),
        }
    }
}

/// Validates whether an action can run based on context key/value pairs.
#[derive(Debug, Clone)]
pub struct ActionPreConditionChecker {
    conditions: Vec<PreCondition>,
}

impl ActionPreConditionChecker {
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
        }
    }

    pub fn add_condition(&mut self, key: &str, value: &str) {
        self.conditions.push(PreCondition {
            key: key.to_string(),
            expected_value: value.to_string(),
        });
    }

    pub fn check(&self, context: &HashMap<String, String>) -> PreConditionResult {
        for cond in &self.conditions {
            match context.get(&cond.key) {
                None => {
                    return PreConditionResult::Failed {
                        missing_key: Some(cond.key.clone()),
                        wrong_value: None,
                    };
                }
                Some(actual) if actual != &cond.expected_value => {
                    return PreConditionResult::Failed {
                        missing_key: None,
                        wrong_value: Some((
                            cond.key.clone(),
                            cond.expected_value.clone(),
                            actual.clone(),
                        )),
                    };
                }
                _ => {}
            }
        }
        PreConditionResult::Satisfied
    }
}

// ---------------------------------------------------------------------------
// Collapsible action groups
// ---------------------------------------------------------------------------

/// State of a single collapsible action group.
#[derive(Debug, Clone)]
pub struct ActionGroupState {
    pub label: String,
    pub collapsed: bool,
    pub action_ids: Vec<String>,
}

/// Manages collapsible menu sections for action groups.
#[derive(Debug, Clone)]
pub struct ActionGroupCollapse {
    groups: HashMap<String, ActionGroupState>,
}

impl ActionGroupCollapse {
    pub fn new() -> Self {
        Self {
            groups: HashMap::new(),
        }
    }

    pub fn add_group(&mut self, id: &str, label: &str) {
        self.groups.insert(
            id.to_string(),
            ActionGroupState {
                label: label.to_string(),
                collapsed: false,
                action_ids: Vec::new(),
            },
        );
    }

    /// Adds an action to an existing group. Returns `false` if the group does not exist.
    pub fn add_action_to_group(&mut self, group_id: &str, action_id: &str) -> bool {
        match self.groups.get_mut(group_id) {
            Some(state) => {
                state.action_ids.push(action_id.to_string());
                true
            }
            None => false,
        }
    }

    /// Toggles the collapsed state of a group. Returns `false` if the group does not exist.
    pub fn toggle_group(&mut self, id: &str) -> bool {
        match self.groups.get_mut(id) {
            Some(state) => {
                state.collapsed = !state.collapsed;
                true
            }
            None => false,
        }
    }

    /// Returns visible action ids for a group.
    /// If collapsed, returns an empty vec; otherwise returns all action ids.
    pub fn visible_actions(&self, group_id: &str) -> Vec<&str> {
        match self.groups.get(group_id) {
            Some(state) if !state.collapsed => {
                state.action_ids.iter().map(|s| s.as_str()).collect()
            }
            _ => Vec::new(),
        }
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }
}

// ---------------------------------------------------------------------------
// Keybinding rendering
// ---------------------------------------------------------------------------

/// Renders keybinding labels for display in menus and tooltips.
#[derive(Debug, Clone)]
pub struct ActionKeybindingRenderer;

impl ActionKeybindingRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Joins key parts with `+`, e.g. `["Ctrl","Shift","P"]` → `"Ctrl+Shift+P"`.
    pub fn render_keybinding(parts: &[&str]) -> String {
        parts.join("+")
    }

    /// Renders a two-chord keybinding, e.g. `"Ctrl+K Ctrl+C"`.
    pub fn render_chord(first: &[&str], second: &[&str]) -> String {
        format!(
            "{} {}",
            Self::render_keybinding(first),
            Self::render_keybinding(second)
        )
    }

    /// Maps modifier names to platform-specific symbols.
    pub fn platform_label(key: &str, is_mac: bool) -> String {
        if is_mac {
            match key {
                "Ctrl" => "⌘".to_string(),
                "Alt" => "⌥".to_string(),
                "Shift" => "⇧".to_string(),
                "Meta" => "⌃".to_string(),
                other => other.to_string(),
            }
        } else {
            key.to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// Execution metrics
// ---------------------------------------------------------------------------

/// Record of a single action execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ActionExecution {
    pub action_id: String,
    pub duration_ms: u64,
    pub success: bool,
    pub timestamp: u64,
}

/// Tracks execution metrics for actions.
#[derive(Debug, Clone)]
pub struct ActionExecutionMetrics {
    executions: Vec<ActionExecution>,
}

impl ActionExecutionMetrics {
    pub fn new() -> Self {
        Self {
            executions: Vec::new(),
        }
    }

    pub fn record(&mut self, action_id: &str, duration_ms: u64, success: bool, timestamp: u64) {
        self.executions.push(ActionExecution {
            action_id: action_id.to_string(),
            duration_ms,
            success,
            timestamp,
        });
    }

    pub fn total_executions(&self) -> usize {
        self.executions.len()
    }

    pub fn success_rate(&self) -> f64 {
        if self.executions.is_empty() {
            return 0.0;
        }
        let successes = self.executions.iter().filter(|e| e.success).count();
        successes as f64 / self.executions.len() as f64
    }

    pub fn average_duration_ms(&self) -> f64 {
        if self.executions.is_empty() {
            return 0.0;
        }
        let total: u64 = self.executions.iter().map(|e| e.duration_ms).sum();
        total as f64 / self.executions.len() as f64
    }

    /// Returns the action_id with the most executions, or `None` if empty.
    pub fn most_used_action(&self) -> Option<&str> {
        if self.executions.is_empty() {
            return None;
        }
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for e in &self.executions {
            *counts.entry(e.action_id.as_str()).or_insert(0) += 1;
        }
        counts
            .into_iter()
            .max_by_key(|&(_, count)| count)
            .map(|(id, _)| id)
    }

    /// Returns references to all failed executions.
    pub fn failures(&self) -> Vec<&ActionExecution> {
        self.executions.iter().filter(|e| !e.success).collect()
    }
}

// ---------------------------------------------------------------------------
// ActionPrecondition - action precondition evaluator
// ---------------------------------------------------------------------------

/// Severity level for action precondition evaluator issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionPreconditionSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ActionPreconditionSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ActionPrecondition].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPreconditionEntry {
    pub id: String,
    pub label: String,
    pub severity: ActionPreconditionSeverity,
    pub detail: Option<String>,
    pub condition_count: usize,
    enabled: bool,
}

impl ActionPreconditionEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ActionPreconditionSeverity::Low,
            detail: None,
            condition_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ActionPreconditionSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_condition_count(mut self, val: usize) -> Self {
        self.condition_count = val;
        self
    }

    pub fn is_satisfied(&self) -> bool {
        self.enabled && self.severity >= ActionPreconditionSeverity::Medium
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
        format!("[{}] {} ({}): {}", self.severity, self.id, self.condition_count, det)
    }
}

impl fmt::Display for ActionPreconditionEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ActionPreconditionEntry] items.
#[derive(Debug, Clone)]
pub struct ActionPrecondition {
    entries: Vec<ActionPreconditionEntry>,
    name: String,
    capacity: usize,
}

impl ActionPrecondition {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ActionPreconditionEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ActionPreconditionEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ActionPreconditionEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn condition_count(&self) -> usize { self.entries.len() }

    pub fn is_satisfied(&self) -> bool {
        self.entries.iter().any(|e| e.is_satisfied())
    }

    pub fn entries_by_severity(&self, severity: ActionPreconditionSeverity) -> Vec<&ActionPreconditionEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ActionPreconditionSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ActionPreconditionEntry> {
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

    pub fn enabled_entries(&self) -> Vec<&ActionPreconditionEntry> {
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
// ActionPrioritySorter - action priority sorter
// ---------------------------------------------------------------------------

/// Configuration for [ActionPrioritySorter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPrioritySorterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub priority_level: usize,
}

impl ActionPrioritySorterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, priority_level: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_priority_level(mut self, val: usize) -> Self { self.priority_level = val; self }
}

impl Default for ActionPrioritySorterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ActionPrioritySorter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPrioritySorterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ActionPrioritySorterItem {
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

    pub fn needs_sorting(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ActionPrioritySorterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ActionPrioritySorterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ActionPrioritySorter {
    config: ActionPrioritySorterConfig,
    items: Vec<ActionPrioritySorterItem>,
}

impl ActionPrioritySorter {
    pub fn new(config: ActionPrioritySorterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ActionPrioritySorterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ActionPrioritySorterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ActionPrioritySorterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn priority_level(&self) -> usize { self.items.len() }

    pub fn needs_sorting(&self) -> bool {
        self.items.iter().any(|i| i.needs_sorting())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ActionPrioritySorterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ActionPrioritySorterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ActionPrioritySorterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-actions: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionsXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ActionsXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ActionsXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ActionsXRegistry {
    entries: Vec<ActionsXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ActionsXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ActionsXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ActionsXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ActionsXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ActionsXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ActionsXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ActionsXConfig> {
        let mut sorted: Vec<&ActionsXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ActionsXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ActionsXIterator<'_> {
        ActionsXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ActionsXIterator<'a> {
    inner: std::slice::Iter<'a, ActionsXConfig>,
}

impl<'a> Iterator for ActionsXIterator<'a> {
    type Item = &'a ActionsXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ActionsXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ActionsXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ActionsXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ActionsXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ActionsXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ActionsXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ActionsXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ActionsXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ActionsXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ActionsXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ActionsXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ActionsXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ActionsXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for actions
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaActionsRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaActionsRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaActionsCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaActionsCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaActionsCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 4
// ---------------------------------------------------------------------------

/// Generic object pool `Xc4Pool<T>`.
pub struct Xc4Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc4Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc4PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc4Pool<T> {
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
    pub fn stats(&self) -> Xc4PoolStats {
        Xc4PoolStats {
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

impl<T> Default for Xc4Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc4Scheduler`.
pub struct Xc4Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc4Scheduler {
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

impl Default for Xc4Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_4 hash for the given byte slice.
pub fn xc_4_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_4 convention.
pub fn xc_4_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_62 deepening: state machine + event bus ---

/// States for the Xd62 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd62State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd62State {
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
pub struct Xd62Transition {
    pub from: Xd62State,
    pub to: Xd62State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd62StateMachine {
    current: Xd62State,
    history: Vec<Xd62Transition>,
    step_counter: usize,
}

impl Xd62StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd62State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd62State {
        self.current
    }

    pub fn history(&self) -> &[Xd62Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd62State) -> Result<Xd62State, String> {
        let allowed = match (self.current, target) {
            (Xd62State::Idle, Xd62State::Running) => true,
            (Xd62State::Running, Xd62State::Paused) => true,
            (Xd62State::Running, Xd62State::Done) => true,
            (Xd62State::Paused, Xd62State::Running) => true,
            (Xd62State::Paused, Xd62State::Done) => true,
            (Xd62State::Done, Xd62State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_62: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd62Transition {
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
            "Xd62SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd62State> {
        let prefix = "Xd62SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd62State::Idle),
            "Running" => Some(Xd62State::Running),
            "Paused" => Some(Xd62State::Paused),
            "Done" => Some(Xd62State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd62State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd62 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd62Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd62Event {
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

type Xd62HandlerFn = Box<dyn Fn(&Xd62Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd62EventBus {
    handlers: Vec<(usize, Option<String>, Xd62HandlerFn)>,
    next_id: usize,
    published: Vec<Xd62Event>,
}

impl Xd62EventBus {
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
        F: Fn(&Xd62Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd62Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd62Event) {
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

    pub fn published_events(&self) -> &[Xd62Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #60
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf60Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf60TrieNode {
    children: std::collections::HashMap<char, Xf60TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf60Trie {
    root: Xf60TrieNode,
    count: usize,
}

impl Xf60Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf60TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf60TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf60TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf60BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf60BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 3).
pub struct Xh3SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh3SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 45 as u64,
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

/// A compact bit set supporting boolean operations (variant 3).
pub struct Xh3BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh3BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 3).
pub struct Xi3Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi3Deque<T> {
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
pub struct Xi3Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi3Interval {
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

/// A simple interval tree (variant 3).
pub struct Xi3IntervalTree {
    xi_intervals: Vec<Xi3Interval>,
}

impl Xi3IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi3Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi3Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi3Interval) -> Vec<&Xi3Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi3Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi3Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi3Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi3Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi3Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi3Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 3) ---

/// Disjoint set / union-find for crate 3.
pub struct Xj3UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj3UnionFind {
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

const XJ3_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 3.
pub struct Xj3BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj3BTreeNode<K, V>>>,
    len: usize,
}

struct Xj3BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj3BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj3BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ3_BTREE_ORDER - 1
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
        let mid = XJ3_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj3BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj3BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj3BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj3BTreeNode::xj_new_leaf();
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


// --- xk_3 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk3SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk3SegmentTree {
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
pub struct Xk3DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk3DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_3).
#[derive(Debug, Clone)]
pub struct Xl3Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl3Rope {
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

/// Suffix array for efficient string searching (xl_3).
#[derive(Debug, Clone)]
pub struct Xl3SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl3SuffixArray {
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
pub struct Xm3MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm3MatrixSparse {
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
pub struct Xm3Tokenizer {
    text: String,
}

impl Xm3Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 3.
pub struct Xn3Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn3Fenwick {
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

// ----- AVL tree map — crate 3 -----

#[derive(Debug, Clone)]
struct Xn3AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn3AvlNode<K, V>>>,
    right: Option<Box<Xn3AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 3.
#[derive(Debug, Clone)]
pub struct Xn3AVL<K, V> {
    root: Option<Box<Xn3AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn3AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn3AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn3AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn3AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn3AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn3AvlNode<K, V>>) -> Box<Xn3AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn3AvlNode<K, V>>) -> Box<Xn3AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn3AvlNode<K, V>>) -> Box<Xn3AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn3AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn3AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn3AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn3AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn3AvlNode<K, V>>) -> &Xn3AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn3AvlNode<K, V>>) -> (Box<Xn3AvlNode<K, V>>, Option<Box<Xn3AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn3AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn3AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn3AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn3AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn3AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn3AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn3AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    // -- New tests --

    #[test]
    fn menu_item_is_separator() {
        let sep = MenuItem {
            command_id: "-".into(),
            title: "-".into(),
            group: None,
            order: None,
            when: None,
        };
        assert!(sep.is_separator());

        let normal = MenuItem {
            command_id: "cmd.save".into(),
            title: "Save".into(),
            group: None,
            order: None,
            when: None,
        };
        assert!(!normal.is_separator());
    }

    #[test]
    fn menu_item_matches_filter() {
        let item = MenuItem {
            command_id: "editor.formatDocument".into(),
            title: "Format Document".into(),
            group: None,
            order: None,
            when: None,
        };
        assert!(item.matches_filter("format"));
        assert!(item.matches_filter("FORMAT"));
        assert!(item.matches_filter("editor.format"));
        assert!(!item.matches_filter("compile"));
    }

    #[test]
    fn menu_item_display() {
        let with_group = MenuItem {
            command_id: "cmd.save".into(),
            title: "Save".into(),
            group: Some("file".into()),
            order: None,
            when: None,
        };
        assert_eq!(format!("{with_group}"), "[file] Save (cmd.save)");

        let without_group = MenuItem {
            command_id: "cmd.open".into(),
            title: "Open".into(),
            group: None,
            order: None,
            when: None,
        };
        assert_eq!(format!("{without_group}"), "Open (cmd.open)");
    }

    #[test]
    fn menu_id_is_context_and_main_menu() {
        assert!(MenuId::EditorContext.is_context_menu());
        assert!(MenuId::ExplorerContext.is_context_menu());
        assert!(MenuId::TerminalContext.is_context_menu());
        assert!(!MenuId::CommandPalette.is_context_menu());
        assert!(!MenuId::MenubarFile.is_context_menu());

        assert!(MenuId::MenubarFile.is_main_menu());
        assert!(MenuId::MenubarEdit.is_main_menu());
        assert!(MenuId::MenubarHelp.is_main_menu());
        assert!(!MenuId::EditorContext.is_main_menu());
        assert!(!MenuId::TouchBar.is_main_menu());
    }

    #[test]
    fn menu_id_label() {
        assert_eq!(MenuId::CommandPalette.label(), "Command Palette");
        assert_eq!(MenuId::MenubarFile.label(), "File");
        assert_eq!(MenuId::TouchBar.label(), "Touch Bar");
    }

    #[test]
    fn action_weight_is_high_and_is_low() {
        assert!(ActionWeight::new(0, 0).is_high());
        assert!(ActionWeight::new(0, -5).is_high());
        assert!(!ActionWeight::new(0, 1).is_high());
        assert!(!ActionWeight::new(1, 0).is_high());

        assert!(ActionWeight::new(100, 0).is_low());
        assert!(ActionWeight::new(200, 0).is_low());
        assert!(!ActionWeight::new(99, 0).is_low());
    }

    #[test]
    fn weighted_action_display_and_ord() {
        let a = WeightedAction::new("a", "Alpha", ActionWeight::new(0, 1));
        let b = WeightedAction::new("b", "Beta", ActionWeight::new(0, 2));
        assert_eq!(format!("{a}"), "Alpha [a] (0:1)");
        assert!(a < b);
    }

    #[test]
    fn action_registry_is_empty_find_by_label_clear() {
        let cmds = Arc::new(CommandRegistry::new());
        let registry = ActionRegistry::new(cmds);
        assert!(registry.is_empty());

        registry.register_action("a", "Save", Some("File".into()), noop_handler(), vec![], None);
        registry.register_action("b", "Save", None, noop_handler(), vec![], None);
        registry.register_action("c", "Open", None, noop_handler(), vec![], None);

        assert!(!registry.is_empty());
        let mut found = registry.find_by_label("Save");
        found.sort();
        assert_eq!(found, vec!["a".to_string(), "b".to_string()]);
        assert!(registry.find_by_label("Missing").is_empty());

        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.action_count(), 0);
    }

    #[test]
    fn action_group_basics() {
        let mut group = ActionGroup::new("navigation");
        assert!(group.is_empty());
        assert_eq!(group.len(), 0);

        group.add(WeightedAction::new("b", "Beta", ActionWeight::new(0, 2)));
        group.add(WeightedAction::new("a", "Alpha", ActionWeight::new(0, 1)));
        assert_eq!(group.len(), 2);
        assert!(!group.is_empty());

        let sorted = group.sorted();
        assert_eq!(sorted[0].id, "a");
        assert_eq!(sorted[1].id, "b");

        assert_eq!(format!("{group}"), "navigation (2 actions)");
    }

    #[test]
    fn actions_stats_summary_and_has_failures() {
        let mut stats = ActionsStats::new();
        assert!(!stats.has_failures());
        assert!(stats.summary().contains("0 ops"));

        stats.record_success(1000);
        stats.record_failure(2000);
        assert!(stats.has_failures());
        let summary = stats.summary();
        assert!(summary.contains("2 ops"));
        assert!(summary.contains("50.0%"));
    }

    // -- AuditLog tests --

    #[test]
    fn audit_log_record_and_query() {
        let log = ActionAuditLog::new();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);

        log.record_success("file.save");
        log.record_success("file.save");
        log.record_failure("file.open", "permission denied");

        assert_eq!(log.len(), 3);
        assert!(!log.is_empty());

        let all = log.entries();
        assert_eq!(all[0].seq, 1);
        assert_eq!(all[1].seq, 2);
        assert_eq!(all[2].seq, 3);
        assert!(all[0].success);
        assert!(!all[2].success);
        assert_eq!(all[2].detail.as_deref(), Some("permission denied"));

        let saves = log.entries_for("file.save");
        assert_eq!(saves.len(), 2);

        log.clear();
        assert!(log.is_empty());
    }

    // -- FrequencyTracker tests --

    #[test]
    fn frequency_tracker_top_n() {
        let tracker = ActionFrequencyTracker::new();
        for _ in 0..5 {
            tracker.record("file.save");
        }
        for _ in 0..3 {
            tracker.record("file.open");
        }
        tracker.record("edit.undo");

        assert_eq!(tracker.count("file.save"), 5);
        assert_eq!(tracker.count("file.open"), 3);
        assert_eq!(tracker.count("edit.undo"), 1);
        assert_eq!(tracker.count("nonexistent"), 0);

        let top = tracker.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0], ("file.save".to_string(), 5));
        assert_eq!(top[1], ("file.open".to_string(), 3));

        tracker.reset();
        assert_eq!(tracker.count("file.save"), 0);
        assert!(tracker.tracked_actions().is_empty());
    }

    // -- Keybinding conflict detection --

    #[test]
    fn detect_keybinding_conflicts_finds_duplicates() {
        let bindings = vec![
            ("file.save".to_string(), "Ctrl+S".to_string()),
            ("custom.save".to_string(), "Ctrl+S".to_string()),
            ("file.open".to_string(), "Ctrl+O".to_string()),
        ];
        let conflicts = detect_keybinding_conflicts(&bindings);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].action_a, "file.save");
        assert_eq!(conflicts[0].action_b, "custom.save");
        assert!(conflicts[0].reason.contains("Ctrl+S"));
    }

    #[test]
    fn detect_keybinding_conflicts_no_duplicates() {
        let bindings = vec![
            ("a".to_string(), "Ctrl+A".to_string()),
            ("b".to_string(), "Ctrl+B".to_string()),
        ];
        assert!(detect_keybinding_conflicts(&bindings).is_empty());
    }

    // -- UndoGroup and UndoGroupStack tests --

    #[test]
    fn undo_group_and_stack() {
        let mut group = UndoGroup::new("Refactor rename");
        assert!(group.is_empty());
        group.push("editor.rename");
        group.push("editor.formatDocument");
        assert_eq!(group.len(), 2);
        assert!(group.contains("editor.rename"));
        assert!(!group.contains("editor.save"));
        assert_eq!(
            format!("{group}"),
            "UndoGroup 'Refactor rename' (2 actions)"
        );

        let stack = UndoGroupStack::new(3);
        assert!(stack.is_empty());

        stack.push(group.clone());
        assert_eq!(stack.len(), 1);

        let peeked = stack.peek().unwrap();
        assert_eq!(peeked.label, "Refactor rename");
        assert_eq!(stack.len(), 1); // peek doesn't remove

        let popped = stack.pop().unwrap();
        assert_eq!(popped.label, "Refactor rename");
        assert!(stack.is_empty());

        // Verify max_depth eviction
        for i in 0..5 {
            let mut g = UndoGroup::new(format!("group-{i}"));
            g.push(format!("action-{i}"));
            stack.push(g);
        }
        assert_eq!(stack.len(), 3); // max_depth = 3, oldest are evicted
        let top = stack.peek().unwrap();
        assert_eq!(top.label, "group-4");

        stack.clear();
        assert!(stack.is_empty());
    }

    // -- ActionPreConditionChecker tests --

    #[test]
    fn precondition_satisfied_when_context_matches() {
        let mut checker = ActionPreConditionChecker::new();
        checker.add_condition("editorLangId", "rust");
        checker.add_condition("isDebugging", "false");

        let mut ctx = HashMap::new();
        ctx.insert("editorLangId".to_string(), "rust".to_string());
        ctx.insert("isDebugging".to_string(), "false".to_string());

        assert_eq!(checker.check(&ctx), PreConditionResult::Satisfied);
    }

    #[test]
    fn precondition_fails_on_missing_key() {
        let mut checker = ActionPreConditionChecker::new();
        checker.add_condition("editorLangId", "rust");

        let ctx = HashMap::new();
        let result = checker.check(&ctx);
        assert_eq!(
            result,
            PreConditionResult::Failed {
                missing_key: Some("editorLangId".to_string()),
                wrong_value: None,
            }
        );
        assert!(format!("{result}").contains("missing context key"));
    }

    #[test]
    fn precondition_fails_on_wrong_value() {
        let mut checker = ActionPreConditionChecker::new();
        checker.add_condition("editorLangId", "rust");

        let mut ctx = HashMap::new();
        ctx.insert("editorLangId".to_string(), "python".to_string());

        let result = checker.check(&ctx);
        assert_eq!(
            result,
            PreConditionResult::Failed {
                missing_key: None,
                wrong_value: Some((
                    "editorLangId".to_string(),
                    "rust".to_string(),
                    "python".to_string()
                )),
            }
        );
        assert!(format!("{result}").contains("expected"));
    }

    // -- ActionGroupCollapse tests --

    #[test]
    fn group_collapse_add_and_count() {
        let mut collapse = ActionGroupCollapse::new();
        assert_eq!(collapse.group_count(), 0);

        collapse.add_group("file", "File");
        collapse.add_group("edit", "Edit");
        assert_eq!(collapse.group_count(), 2);
    }

    #[test]
    fn group_collapse_add_action_to_group() {
        let mut collapse = ActionGroupCollapse::new();
        collapse.add_group("nav", "Navigation");

        assert!(collapse.add_action_to_group("nav", "goto.line"));
        assert!(collapse.add_action_to_group("nav", "goto.symbol"));
        assert!(!collapse.add_action_to_group("nonexistent", "foo"));
    }

    #[test]
    fn group_collapse_toggle_and_visible() {
        let mut collapse = ActionGroupCollapse::new();
        collapse.add_group("nav", "Navigation");
        collapse.add_action_to_group("nav", "goto.line");
        collapse.add_action_to_group("nav", "goto.symbol");

        assert_eq!(collapse.visible_actions("nav"), vec!["goto.line", "goto.symbol"]);

        assert!(collapse.toggle_group("nav"));
        assert!(collapse.visible_actions("nav").is_empty());

        assert!(collapse.toggle_group("nav"));
        assert_eq!(collapse.visible_actions("nav").len(), 2);

        assert!(!collapse.toggle_group("nonexistent"));
    }

    // -- ActionKeybindingRenderer tests --

    #[test]
    fn render_keybinding_joins_parts() {
        assert_eq!(
            ActionKeybindingRenderer::render_keybinding(&["Ctrl", "Shift", "P"]),
            "Ctrl+Shift+P"
        );
    }

    #[test]
    fn render_chord_two_sequences() {
        assert_eq!(
            ActionKeybindingRenderer::render_chord(&["Ctrl", "K"], &["Ctrl", "C"]),
            "Ctrl+K Ctrl+C"
        );
    }

    #[test]
    fn platform_label_mac_modifiers() {
        assert_eq!(ActionKeybindingRenderer::platform_label("Ctrl", true), "⌘");
        assert_eq!(ActionKeybindingRenderer::platform_label("Alt", true), "⌥");
        assert_eq!(ActionKeybindingRenderer::platform_label("Shift", true), "⇧");
        assert_eq!(ActionKeybindingRenderer::platform_label("Meta", true), "⌃");
        assert_eq!(ActionKeybindingRenderer::platform_label("P", true), "P");
        assert_eq!(ActionKeybindingRenderer::platform_label("Ctrl", false), "Ctrl");
    }

    // -- ActionExecutionMetrics tests --

    #[test]
    fn metrics_record_and_totals() {
        let mut metrics = ActionExecutionMetrics::new();
        assert_eq!(metrics.total_executions(), 0);
        assert_eq!(metrics.success_rate(), 0.0);
        assert_eq!(metrics.average_duration_ms(), 0.0);
        assert!(metrics.most_used_action().is_none());

        metrics.record("editor.save", 10, true, 1000);
        metrics.record("editor.save", 20, true, 2000);
        metrics.record("editor.format", 30, false, 3000);

        assert_eq!(metrics.total_executions(), 3);
    }

    #[test]
    fn metrics_success_rate_and_average() {
        let mut metrics = ActionExecutionMetrics::new();
        metrics.record("a", 10, true, 1);
        metrics.record("b", 30, true, 2);
        metrics.record("c", 20, false, 3);

        let rate = metrics.success_rate();
        assert!((rate - 2.0 / 3.0).abs() < 1e-9);

        let avg = metrics.average_duration_ms();
        assert!((avg - 20.0).abs() < 1e-9);
    }

    #[test]
    fn metrics_most_used_and_failures() {
        let mut metrics = ActionExecutionMetrics::new();
        metrics.record("save", 5, true, 1);
        metrics.record("save", 6, true, 2);
        metrics.record("save", 7, false, 3);
        metrics.record("format", 10, false, 4);

        assert_eq!(metrics.most_used_action(), Some("save"));

        let fails = metrics.failures();
        assert_eq!(fails.len(), 2);
        assert!(!fails[0].success);
        assert!(!fails[1].success);
    }

#[test]
    fn actionprecondition_severity_ordering() {
        assert!(ActionPreconditionSeverity::Critical > ActionPreconditionSeverity::High);
        assert!(ActionPreconditionSeverity::High > ActionPreconditionSeverity::Medium);
        assert!(ActionPreconditionSeverity::Medium > ActionPreconditionSeverity::Low);
    }

    #[test]
    fn actionprecondition_severity_display() {
        assert_eq!(ActionPreconditionSeverity::Low.to_string(), "low");
        assert_eq!(ActionPreconditionSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn actionprecondition_entry_creation() {
        let e = ActionPreconditionEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ActionPreconditionSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn actionprecondition_entry_builder() {
        let e = ActionPreconditionEntry::new("e2", "Entry 2")
            .with_severity(ActionPreconditionSeverity::High)
            .with_detail("some detail")
            .with_condition_count(42);
        assert_eq!(e.severity, ActionPreconditionSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.condition_count, 42);
    }

    #[test]
    fn actionprecondition_entry_enable_disable() {
        let mut e = ActionPreconditionEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn actionprecondition_add_and_count() {
        let mut mgr = ActionPrecondition::new("test");
        mgr.add(ActionPreconditionEntry::new("a", "A"));
        mgr.add(ActionPreconditionEntry::new("b", "B").with_severity(ActionPreconditionSeverity::High));
        assert_eq!(mgr.condition_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn actionprecondition_remove() {
        let mut mgr = ActionPrecondition::new("test");
        mgr.add(ActionPreconditionEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn actionprecondition_capacity() {
        let mut mgr = ActionPrecondition::new("test").with_capacity(1);
        assert!(mgr.add(ActionPreconditionEntry::new("a", "A")));
        assert!(!mgr.add(ActionPreconditionEntry::new("b", "B")));
    }

    #[test]
    fn actionprecondition_sorted_by_severity() {
        let mut mgr = ActionPrecondition::new("test");
        mgr.add(ActionPreconditionEntry::new("lo", "Low"));
        mgr.add(ActionPreconditionEntry::new("hi", "High").with_severity(ActionPreconditionSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ActionPreconditionSeverity::Critical);
    }

    #[test]
    fn actionprecondition_summary() {
        let mgr = ActionPrecondition::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn actionprioritysorter_config_defaults() {
        let cfg = ActionPrioritySorterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn actionprioritysorter_item_creation() {
        let item = ActionPrioritySorterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn actionprioritysorter_add_and_get() {
        let mut mgr = ActionPrioritySorter::new(ActionPrioritySorterConfig::new("test"));
        mgr.add(ActionPrioritySorterItem::new("k1", "v1"));
        assert_eq!(mgr.priority_level(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn actionprioritysorter_remove_item() {
        let mut mgr = ActionPrioritySorter::new(ActionPrioritySorterConfig::new("test"));
        mgr.add(ActionPrioritySorterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn actionprioritysorter_sorted_by_priority() {
        let mut mgr = ActionPrioritySorter::new(ActionPrioritySorterConfig::new("test"));
        mgr.add(ActionPrioritySorterItem::new("lo", "low").with_priority(1));
        mgr.add(ActionPrioritySorterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn actionprioritysorter_items_with_tag() {
        let mut mgr = ActionPrioritySorter::new(ActionPrioritySorterConfig::new("test"));
        mgr.add(ActionPrioritySorterItem::new("a", "1").with_tag("x"));
        mgr.add(ActionPrioritySorterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn actionprioritysorter_report() {
        let mgr = ActionPrioritySorter::new(ActionPrioritySorterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn actions_x_config_new() {
        let c = ActionsXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn actions_x_config_builder() {
        let c = ActionsXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn actions_x_config_display() {
        let c = ActionsXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn actions_x_registry_insert_get() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn actions_x_registry_duplicate() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a")).unwrap();
        assert!(reg.insert(ActionsXConfig::new("a")).is_err());
    }

    #[test]
    fn actions_x_registry_remove() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a")).unwrap();
        reg.insert(ActionsXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn actions_x_registry_active_entries() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a")).unwrap();
        reg.insert(ActionsXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn actions_x_registry_by_weight() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ActionsXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn actions_x_registry_tags() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ActionsXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn actions_x_registry_total_weight() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ActionsXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn actions_x_registry_iterator() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a")).unwrap();
        reg.insert(ActionsXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn actions_x_cache_put_get() {
        let mut cache = ActionsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn actions_x_cache_eviction() {
        let mut cache = ActionsXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn actions_x_cache_lru_order() {
        let mut cache = ActionsXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn actions_x_cache_most_least_recent() {
        let mut cache = ActionsXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn actions_x_formatter_entry() {
        let e = ActionsXConfig::new("k").with_value("v");
        let fmt = ActionsXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn actions_x_formatter_summary() {
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ActionsXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn actions_x_validator_valid() {
        let v = ActionsXValidator::new();
        let c = ActionsXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn actions_x_validator_empty_key() {
        let v = ActionsXValidator::new();
        let c = ActionsXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn actions_x_validator_require_value() {
        let v = ActionsXValidator::new().require_value(true);
        let c = ActionsXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn actions_x_validator_allowed_tags() {
        let v = ActionsXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ActionsXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn actions_x_validator_validate_all() {
        let v = ActionsXValidator::new();
        let mut reg = ActionsXRegistry::new();
        reg.insert(ActionsXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // xa_ extended tests for actions
    #[test]
    fn xa_actions_ring_new() {
        let rb = super::XaActionsRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_actions_ring_push_len() {
        let mut rb = super::XaActionsRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_actions_ring_wrap() {
        let mut rb = super::XaActionsRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_actions_ring_mean_empty() {
        let rb = super::XaActionsRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_actions_ring_mean_values() {
        let mut rb = super::XaActionsRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_actions_ring_min_max() {
        let mut rb = super::XaActionsRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_actions_ring_iter() {
        let mut rb = super::XaActionsRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_actions_counter_new() {
        let c = super::XaActionsCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_actions_counter_inc() {
        let mut c = super::XaActionsCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_actions_counter_inc_by() {
        let mut c = super::XaActionsCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_actions_counter_reset() {
        let mut c = super::XaActionsCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_actions_counter_clear() {
        let mut c = super::XaActionsCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_actions_counter_default() {
        let c = super::XaActionsCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 4 ----

    #[test]
    fn xc_4_pool_new_empty() {
        let pool: super::Xc4Pool<i32> = super::Xc4Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_4_pool_release_acquire() {
        let mut pool = super::Xc4Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_4_pool_acquire_empty() {
        let mut pool: super::Xc4Pool<i32> = super::Xc4Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_4_pool_full() {
        let mut pool = super::Xc4Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_4_pool_drain() {
        let mut pool = super::Xc4Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_4_pool_stats() {
        let mut pool = super::Xc4Pool::new(8);
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
    fn xc_4_pool_clear() {
        let mut pool = super::Xc4Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_4_pool_shrink() {
        let mut pool = super::Xc4Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_4_pool_default() {
        let pool: super::Xc4Pool<String> = super::Xc4Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_4_pool_extend() {
        let mut pool = super::Xc4Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_4_pool_retain() {
        let mut pool = super::Xc4Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_4_scheduler_round_robin() {
        let mut sched = super::Xc4Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_4_scheduler_empty() {
        let mut sched = super::Xc4Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_4_scheduler_reset() {
        let mut sched = super::Xc4Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_4_scheduler_add_remove() {
        let mut sched = super::Xc4Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_4_scheduler_targets() {
        let sched = super::Xc4Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_4_hash_empty() {
        assert_eq!(super::xc_4_hash(b""), 5381);
    }

    #[test]
    fn xc_4_hash_data() {
        let h = super::xc_4_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_4_hash(b"hello"), h);
    }

    #[test]
    fn xc_4_reverse_str() {
        assert_eq!(super::xc_4_reverse("abc"), "cba");
        assert_eq!(super::xc_4_reverse(""), "");
    }


    // --- xd_62 deepening tests ---

    #[test]
    fn xd_62_sm_initial_state() {
        let sm = Xd62StateMachine::new();
        assert_eq!(sm.current_state(), Xd62State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_62_sm_valid_idle_to_running() {
        let mut sm = Xd62StateMachine::new();
        assert!(sm.transition(Xd62State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd62State::Running);
    }

    #[test]
    fn xd_62_sm_valid_running_to_paused() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        assert!(sm.transition(Xd62State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd62State::Paused);
    }

    #[test]
    fn xd_62_sm_valid_running_to_done() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        assert!(sm.transition(Xd62State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd62State::Done);
    }

    #[test]
    fn xd_62_sm_valid_paused_to_running() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        sm.transition(Xd62State::Paused).unwrap();
        assert!(sm.transition(Xd62State::Running).is_ok());
    }

    #[test]
    fn xd_62_sm_valid_done_to_idle() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        sm.transition(Xd62State::Done).unwrap();
        assert!(sm.transition(Xd62State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd62State::Idle);
    }

    #[test]
    fn xd_62_sm_invalid_idle_to_done() {
        let mut sm = Xd62StateMachine::new();
        assert!(sm.transition(Xd62State::Done).is_err());
    }

    #[test]
    fn xd_62_sm_invalid_idle_to_paused() {
        let mut sm = Xd62StateMachine::new();
        assert!(sm.transition(Xd62State::Paused).is_err());
    }

    #[test]
    fn xd_62_sm_history_tracking() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        sm.transition(Xd62State::Paused).unwrap();
        sm.transition(Xd62State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd62State::Idle);
        assert_eq!(sm.history()[0].to, Xd62State::Running);
        assert_eq!(sm.history()[1].from, Xd62State::Running);
        assert_eq!(sm.history()[2].to, Xd62State::Done);
    }

    #[test]
    fn xd_62_sm_serialize_deserialize() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd62StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd62State::Running));
    }

    #[test]
    fn xd_62_sm_deserialize_invalid() {
        assert_eq!(Xd62StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_62_sm_reset() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd62State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_62_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd62EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd62Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_62_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd62EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd62Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd62Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_62_bus_unsubscribe() {
        let mut bus = Xd62EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_62_event_kind_and_payload() {
        let e = Xd62Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd62Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_62_bus_clear_history() {
        let mut bus = Xd62EventBus::new();
        bus.publish(Xd62Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_62_sm_step_counter_increments() {
        let mut sm = Xd62StateMachine::new();
        sm.transition(Xd62State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd62State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #60 --

    #[test]
    fn xf60_trie_insert_search() {
        let mut t = Xf60Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf60_trie_starts_with() {
        let mut t = Xf60Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf60_trie_remove() {
        let mut t = Xf60Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf60_trie_word_count() {
        let mut t = Xf60Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf60_trie_longest_prefix() {
        let mut t = Xf60Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf60_trie_all_words() {
        let mut t = Xf60Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf60_trie_autocomplete() {
        let mut t = Xf60Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf60_trie_empty_search() {
        let t = Xf60Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf60_bloom_add_contains() {
        let mut bf = Xf60BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf60_bloom_probably_absent() {
        let bf = Xf60BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf60_bloom_false_positive_rate() {
        let mut bf = Xf60BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf60_bloom_clear() {
        let mut bf = Xf60BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf60_bloom_union() {
        let mut a = Xf60BloomFilter::xf_new(512, 2);
        let mut b = Xf60BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf60_bloom_intersection_estimate() {
        let mut a = Xf60BloomFilter::xf_new(512, 2);
        let mut b = Xf60BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf60_bloom_union_size_mismatch() {
        let a = Xf60BloomFilter::xf_new(256, 2);
        let b = Xf60BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh3_skip_insert_contains() {
        let mut sl = super::Xh3SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh3_skip_remove() {
        let mut sl = super::Xh3SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh3_skip_len() {
        let mut sl = super::Xh3SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh3_skip_range_query() {
        let mut sl = super::Xh3SkipList::xh_new(4);
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
    fn xh3_skip_floor_ceiling() {
        let mut sl = super::Xh3SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh3_skip_rank() {
        let mut sl = super::Xh3SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh3_skip_empty() {
        let sl = super::Xh3SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh3_skip_duplicates() {
        let mut sl = super::Xh3SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh3_bitset_set_test() {
        let mut bs = super::Xh3BitSet::xh_new(256);
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
    fn xh3_bitset_clear_count() {
        let mut bs = super::Xh3BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh3_bitset_and_or_xor() {
        let mut a = super::Xh3BitSet::xh_new(128);
        let mut b = super::Xh3BitSet::xh_new(128);
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
    fn xh3_bitset_iter_ones() {
        let mut bs = super::Xh3BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh3_bitset_first_last() {
        let mut bs = super::Xh3BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh3_bitset_empty() {
        let bs = super::Xh3BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi3_deque_push_pop_back() {
        let mut dq = super::Xi3Deque::xi_new(4);
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
    fn xi3_deque_push_pop_front() {
        let mut dq = super::Xi3Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi3_deque_mixed_ops() {
        let mut dq = super::Xi3Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi3_deque_get_and_split() {
        let mut dq = super::Xi3Deque::xi_new(8);
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
    fn xi3_deque_rotate_left() {
        let mut dq = super::Xi3Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi3_deque_rotate_right() {
        let mut dq = super::Xi3Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi3_deque_grow() {
        let mut dq = super::Xi3Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi3_deque_empty() {
        let dq = super::Xi3Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi3_interval_tree_insert_query() {
        let mut tree = super::Xi3IntervalTree::xi_new();
        tree.xi_insert(super::Xi3Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi3Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi3Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi3_interval_tree_overlap() {
        let mut tree = super::Xi3IntervalTree::xi_new();
        tree.xi_insert(super::Xi3Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi3Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi3Interval::xi_new(12, 20));
        let q = super::Xi3Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi3_interval_tree_remove() {
        let mut tree = super::Xi3IntervalTree::xi_new();
        tree.xi_insert(super::Xi3Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi3Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi3_interval_tree_gaps() {
        let mut tree = super::Xi3IntervalTree::xi_new();
        tree.xi_insert(super::Xi3Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi3Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi3Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi3Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi3Interval::xi_new(8, 10));
    }

    #[test]
    fn xi3_interval_tree_merge() {
        let mut tree = super::Xi3IntervalTree::xi_new();
        tree.xi_insert(super::Xi3Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi3Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi3Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi3Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi3Interval::xi_new(10, 15));
    }

    #[test]
    fn xi3_interval_tree_all() {
        let mut tree = super::Xi3IntervalTree::xi_new();
        tree.xi_insert(super::Xi3Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi3Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi3_interval_tree_empty() {
        let tree = super::Xi3IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi3_interval_tree_contains_point() {
        let iv = super::Xi3Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 3) ---

    #[test]
    fn xj_3_uf_make_and_find() {
        let mut uf = super::Xj3UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_3_uf_union_connected() {
        let mut uf = super::Xj3UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_3_uf_component_count() {
        let mut uf = super::Xj3UnionFind::xj_new();
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
    fn xj_3_uf_component_size() {
        let mut uf = super::Xj3UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_3_uf_largest_component() {
        let mut uf = super::Xj3UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_3_uf_many_elements() {
        let mut uf = super::Xj3UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_3_uf_separate_components() {
        let mut uf = super::Xj3UnionFind::xj_new();
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
    fn xj_3_uf_path_compression() {
        let mut uf = super::Xj3UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_3_bt_insert_get() {
        let mut bt = super::Xj3BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_3_bt_contains_len() {
        let mut bt = super::Xj3BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_3_bt_replace() {
        let mut bt = super::Xj3BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_3_bt_remove() {
        let mut bt = super::Xj3BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_3_bt_keys_values() {
        let mut bt = super::Xj3BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_3_bt_range() {
        let mut bt = super::Xj3BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_3_bt_min_max() {
        let mut bt = super::Xj3BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_3_bt_many_inserts() {
        let mut bt = super::Xj3BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_3 segment tree tests ---

    #[test]
    fn xk_3_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk3SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_3_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk3SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_3_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk3SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_3_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk3SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_3_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk3SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_3_st_single_element() {
        let data = vec![42];
        let st = super::Xk3SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_3_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk3SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_3_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk3SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_3 disjoint intervals tests ---

    #[test]
    fn xk_3_di_add_and_count() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_3_di_merge_overlap() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_3_di_contains() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_3_di_remove() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_3_di_covered_length() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_3_di_gaps() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_3_di_merge_adjacent() {
        let mut di = super::Xk3DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_3_di_empty() {
        let di = super::Xk3DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_3_rope_new_empty() {
        let rope = super::Xl3Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_3_rope_from_str() {
        let rope = super::Xl3Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_3_rope_insert_at() {
        let mut rope = super::Xl3Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_3_rope_delete_range() {
        let mut rope = super::Xl3Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_3_rope_char_at() {
        let rope = super::Xl3Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_3_rope_split_concat() {
        let rope = super::Xl3Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_3_rope_line_count() {
        let rope = super::Xl3Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_3_rope_line_at() {
        let rope = super::Xl3Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_3_sa_build_and_search() {
        let sa = super::Xl3SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_3_sa_count() {
        let sa = super::Xl3SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_3_sa_longest_repeated() {
        let sa = super::Xl3SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_3_sa_all_positions() {
        let sa = super::Xl3SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_3_sa_len() {
        let sa = super::Xl3SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_3_sa_empty() {
        let sa = super::Xl3SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_3_rope_slice() {
        let rope = super::Xl3Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_3_sa_search_start() {
        let sa = super::Xl3SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_3_sparse_set_get() {
        let mut m = super::Xm3MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_3_sparse_row_col() {
        let mut m = super::Xm3MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_3_sparse_transpose() {
        let mut m = super::Xm3MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_3_sparse_multiply_vec() {
        let mut m = super::Xm3MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_3_sparse_nnz_density() {
        let mut m = super::Xm3MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_3_sparse_clear() {
        let mut m = super::Xm3MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_3_sparse_overwrite_zero() {
        let mut m = super::Xm3MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_3_tokenizer_basic() {
        let t = super::Xm3Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_3_tokenizer_count() {
        let t = super::Xm3Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_3_tokenizer_unique() {
        let t = super::Xm3Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_3_tokenizer_frequency() {
        let t = super::Xm3Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_3_tokenizer_delimiter() {
        let t = super::Xm3Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_3_tokenizer_whitespace() {
        let t = super::Xm3Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_3_tokenizer_empty() {
        let t = super::Xm3Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 3 ----

    #[test]
    fn xn_3_fenwick_prefix_sum() {
        let mut ft = super::Xn3Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_3_fenwick_range_sum() {
        let mut ft = super::Xn3Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_3_fenwick_point_query() {
        let mut ft = super::Xn3Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_3_fenwick_len() {
        let ft = super::Xn3Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_3_fenwick_multiple_updates() {
        let mut ft = super::Xn3Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_3_fenwick_single_element() {
        let mut ft = super::Xn3Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_3_fenwick_find_kth() {
        let mut ft = super::Xn3Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_3_fenwick_negative_delta() {
        let mut ft = super::Xn3Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 3 ----

    #[test]
    fn xn_3_avl_insert_get() {
        let mut m = super::Xn3AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_3_avl_remove() {
        let mut m = super::Xn3AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_3_avl_in_order() {
        let mut m = super::Xn3AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_3_avl_min_max() {
        let mut m = super::Xn3AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_3_avl_floor_ceiling() {
        let mut m = super::Xn3AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_3_avl_height_balanced() {
        let mut m = super::Xn3AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_3_avl_overwrite() {
        let mut m = super::Xn3AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_3_avl_empty() {
        let m: super::Xn3AVL<i32, i32> = super::Xn3AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }
}
