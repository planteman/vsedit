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

}
