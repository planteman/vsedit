//! Menu bar and context menu system.
//!
//! Provides data structures for application menu bars and right-click context
//! menus, along with lookup and mutation helpers.

use std::fmt;
// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The kind of menu entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItemKind {
    Action,
    Submenu,
    Separator,
}

/// A single entry in a menu hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub kind: MenuItemKind,
    pub keybinding: Option<String>,
    pub enabled: bool,
    pub checked: bool,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: MenuItemKind::Action,
            keybinding: None,
            enabled: true,
            checked: false,
            children: Vec::new(),
        }
    }

    pub fn submenu(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: MenuItemKind::Submenu,
            keybinding: None,
            enabled: true,
            checked: false,
            children: Vec::new(),
        }
    }

    pub fn separator() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            kind: MenuItemKind::Separator,
            keybinding: None,
            enabled: false,
            checked: false,
            children: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Menu bar
// ---------------------------------------------------------------------------

/// Top-level application menu bar.
#[derive(Debug, Clone, Default)]
pub struct MenuBar {
    pub menus: Vec<MenuItem>,
}

impl MenuBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_menu(&mut self, item: MenuItem) {
        self.menus.push(item);
    }

    /// Recursively search for an item by id.
    pub fn find_item(&self, id: &str) -> Option<&MenuItem> {
        fn search<'a>(items: &'a [MenuItem], id: &str) -> Option<&'a MenuItem> {
            for item in items {
                if item.id == id {
                    return Some(item);
                }
                if let Some(found) = search(&item.children, id) {
                    return Some(found);
                }
            }
            None
        }
        search(&self.menus, id)
    }

    /// Returns true if menus is empty.
    pub fn is_menus_empty(&self) -> bool {
        self.menus.is_empty()
    }

    /// Get the first menu, if any.
    pub fn first_menu(&self) -> Option<&MenuItem> {
        self.menus.first()
    }

    /// Get the last menu, if any.
    pub fn last_menu(&self) -> Option<&MenuItem> {
        self.menus.last()
    }

    /// Retain only menus matching the predicate.
    pub fn retain_menus(&mut self, f: impl Fn(&MenuItem) -> bool) {
        self.menus.retain(|item| f(item));
    }
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

/// A context (right-click) menu anchored at a screen position.
#[derive(Debug, Clone)]
pub struct ContextMenu {
    pub items: Vec<MenuItem>,
    pub x: u32,
    pub y: u32,
}

impl ContextMenu {
    pub fn new(x: u32, y: u32) -> Self {
        Self {
            items: Vec::new(),
            x,
            y,
        }
    }

    pub fn add_item(&mut self, item: MenuItem) {
        self.items.push(item);
    }

    pub fn add_separator(&mut self) {
        self.items.push(MenuItem::separator());
    }
}

// ---------------------------------------------------------------------------
// MenuBar additional helpers
// ---------------------------------------------------------------------------

impl MenuBar {
    /// Recursively search for an item by id, returning a mutable reference.
    pub fn find_item_mut(&mut self, id: &str) -> Option<&mut MenuItem> {
        fn search_mut<'a>(items: &'a mut [MenuItem], id: &str) -> Option<&'a mut MenuItem> {
            for item in items {
                if item.id == id {
                    return Some(item);
                }
                if let Some(found) = search_mut(&mut item.children, id) {
                    return Some(found);
                }
            }
            None
        }
        search_mut(&mut self.menus, id)
    }

    /// Sets the `enabled` flag on the item with the given id (recursive).
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        if let Some(item) = self.find_item_mut(id) {
            item.enabled = enabled;
            true
        } else {
            false
        }
    }

    /// Sets the `checked` flag on the item with the given id (recursive).
    pub fn set_checked(&mut self, id: &str, checked: bool) -> bool {
        if let Some(item) = self.find_item_mut(id) {
            item.checked = checked;
            true
        } else {
            false
        }
    }

    /// Returns a flattened list of all action items in the menu bar.
    pub fn get_all_actions(&self) -> Vec<&MenuItem> {
        fn collect_actions<'a>(items: &'a [MenuItem], out: &mut Vec<&'a MenuItem>) {
            for item in items {
                if item.kind == MenuItemKind::Action {
                    out.push(item);
                }
                collect_actions(&item.children, out);
            }
        }
        let mut result = Vec::new();
        collect_actions(&self.menus, &mut result);
        result
    }

    /// Recursively removes the first item matching the given id. Returns
    /// `true` if an item was removed.
    pub fn remove_item(&mut self, id: &str) -> bool {
        fn remove_in(items: &mut Vec<MenuItem>, id: &str) -> bool {
            if let Some(pos) = items.iter().position(|i| i.id == id) {
                items.remove(pos);
                return true;
            }
            for item in items.iter_mut() {
                if remove_in(&mut item.children, id) {
                    return true;
                }
            }
            false
        }
        remove_in(&mut self.menus, id)
    }
}

// ---------------------------------------------------------------------------
// Menu contribution system
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// A contribution that associates a menu item with a group and ordering.
#[derive(Debug, Clone)]
pub struct MenuContribution {
    pub group_id: String,
    pub order: i32,
    pub item: MenuItem,
}

/// Registry that collects menu contributions by location.
#[derive(Debug, Clone, Default)]
pub struct MenuRegistry {
    contributions: HashMap<String, Vec<MenuContribution>>,
}

impl MenuRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a contribution to the given location.
    pub fn add(&mut self, location: impl Into<String>, contribution: MenuContribution) {
        self.contributions
            .entry(location.into())
            .or_default()
            .push(contribution);
    }

    /// Returns contributions for a location (unsorted).
    pub fn get(&self, location: &str) -> Option<&Vec<MenuContribution>> {
        self.contributions.get(location)
    }

    /// Returns contributions for a location sorted by `order`.
    pub fn get_sorted(&self, location: &str) -> Vec<&MenuContribution> {
        let mut items: Vec<&MenuContribution> = match self.contributions.get(location) {
            Some(v) => v.iter().collect(),
            None => return Vec::new(),
        };
        items.sort_by_key(|c| c.order);
        items
    }
}

// ---------------------------------------------------------------------------
// Menu provider trait
// ---------------------------------------------------------------------------

/// Trait for dynamic menu providers.
pub trait MenuProvider {
    /// Returns the menu items provided by this implementation.
    fn get_menu_items(&self) -> Vec<MenuItem> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Accumulated statistics for menu operations.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl MenuStats {
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
    pub fn merge(&mut self, other: &MenuStats) {
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

impl Default for MenuStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MenuStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MenuStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for menu.
#[derive(Debug, Clone)]
pub struct MenuValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl MenuValidator {
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

impl Default for MenuValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Menu item groups
// ---------------------------------------------------------------------------

/// A named group of menu items with a sort order, delimited by separators.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItemGroup {
    pub group_id: String,
    pub order: i32,
    pub items: Vec<MenuItem>,
}

impl MenuItemGroup {
    pub fn new(group_id: impl Into<String>, order: i32) -> Self {
        Self { group_id: group_id.into(), order, items: Vec::new() }
    }

    pub fn add_item(&mut self, item: MenuItem) {
        self.items.push(item);
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Convert groups into a flat list of MenuItems with separators between groups.
    pub fn flatten_groups(groups: &[MenuItemGroup]) -> Vec<MenuItem> {
        let mut result = Vec::new();
        let mut sorted: Vec<&MenuItemGroup> = groups.iter().collect();
        sorted.sort_by_key(|g| g.order);
        for group in &sorted {
            if group.items.is_empty() {
                continue;
            }
            if !result.is_empty() {
                result.push(MenuItem::separator());
            }
            for item in &group.items {
                result.push(item.clone());
            }
        }
        result
    }
}

impl fmt::Display for MenuItemGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MenuItemGroup({}, order={}, items={})", self.group_id, self.order, self.items.len())
    }
}

// ---------------------------------------------------------------------------
// Menu item sorting
// ---------------------------------------------------------------------------

/// Sort menu contributions by group_id alphabetically, then by order within each group.
pub fn menu_item_sort(contributions: &mut [MenuContribution]) {
    contributions.sort_by(|a, b| {
        a.group_id.cmp(&b.group_id).then(a.order.cmp(&b.order))
    });
}

// ---------------------------------------------------------------------------
// Menu action resolution
// ---------------------------------------------------------------------------

/// A resolved menu action mapping a menu item to its command id.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMenuAction {
    pub menu_item_id: String,
    pub command_id: String,
    pub label: String,
    pub keybinding: Option<String>,
    pub enabled: bool,
}

impl fmt::Display for ResolvedMenuAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.keybinding {
            Some(kb) => write!(f, "{} ({}) [{}]", self.label, self.command_id, kb),
            None => write!(f, "{} ({})", self.label, self.command_id),
        }
    }
}

/// Look up the command for a menu item. Returns a ResolvedMenuAction by matching
/// menu_item_id to a command registry (represented as a HashMap<String, String>
/// mapping menu item ids to command ids).
pub fn menu_action_resolve(
    bar: &MenuBar,
    command_map: &HashMap<String, String>,
) -> Vec<ResolvedMenuAction> {
    let actions = bar.get_all_actions();
    actions.iter().filter_map(|item| {
        let command_id = command_map.get(&item.id)?;
        Some(ResolvedMenuAction {
            menu_item_id: item.id.clone(),
            command_id: command_id.clone(),
            label: item.label.clone(),
            keybinding: item.keybinding.clone(),
            enabled: item.enabled,
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_find_item() {
        let mut bar = MenuBar::new();
        let mut file_menu = MenuItem::submenu("file", "File");
        file_menu.children.push(MenuItem::action("open", "Open"));
        file_menu.children.push(MenuItem::action("save", "Save"));
        bar.add_menu(file_menu);

        assert!(bar.find_item("open").is_some());
        assert!(bar.find_item("nonexistent").is_none());
    }

    #[test]
    fn context_menu_separator() {
        let mut ctx = ContextMenu::new(100, 200);
        ctx.add_item(MenuItem::action("cut", "Cut"));
        ctx.add_separator();
        ctx.add_item(MenuItem::action("paste", "Paste"));
        assert_eq!(ctx.items.len(), 3);
        assert_eq!(ctx.items[1].kind, MenuItemKind::Separator);
    }

    #[test]
    fn menu_item_defaults() {
        let item = MenuItem::action("test", "Test");
        assert!(item.enabled);
        assert!(!item.checked);
        assert!(item.keybinding.is_none());
        assert!(item.children.is_empty());
    }

    fn make_bar() -> MenuBar {
        let mut bar = MenuBar::new();
        let mut file = MenuItem::submenu("file", "File");
        file.children.push(MenuItem::action("open", "Open"));
        file.children.push(MenuItem::action("save", "Save"));
        bar.add_menu(file);
        bar.add_menu(MenuItem::action("help", "Help"));
        bar
    }

    #[test]
    fn find_item_mut_updates() {
        let mut bar = make_bar();
        let item = bar.find_item_mut("open").unwrap();
        item.label = "Open File…".into();
        assert_eq!(bar.find_item("open").unwrap().label, "Open File…");
    }

    #[test]
    fn set_enabled_recursive() {
        let mut bar = make_bar();
        assert!(bar.set_enabled("save", false));
        assert!(!bar.find_item("save").unwrap().enabled);
        assert!(!bar.set_enabled("nonexistent", false));
    }

    #[test]
    fn set_checked_recursive() {
        let mut bar = make_bar();
        assert!(bar.set_checked("help", true));
        assert!(bar.find_item("help").unwrap().checked);
    }

    #[test]
    fn get_all_actions_flattens() {
        let bar = make_bar();
        let actions = bar.get_all_actions();
        let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"open"));
        assert!(ids.contains(&"save"));
        assert!(ids.contains(&"help"));
        assert_eq!(actions.len(), 3);
    }

    #[test]
    fn remove_item_from_children() {
        let mut bar = make_bar();
        assert!(bar.remove_item("open"));
        assert!(bar.find_item("open").is_none());
        assert!(bar.find_item("save").is_some());
    }

    #[test]
    fn remove_item_top_level() {
        let mut bar = make_bar();
        assert!(bar.remove_item("help"));
        assert!(bar.find_item("help").is_none());
    }

    #[test]
    fn remove_item_nonexistent() {
        let mut bar = make_bar();
        assert!(!bar.remove_item("nope"));
    }

    #[test]
    fn menu_registry_add_get() {
        let mut reg = MenuRegistry::new();
        let contrib = MenuContribution {
            group_id: "navigation".into(),
            order: 1,
            item: MenuItem::action("go_to", "Go To"),
        };
        reg.add("editor/context", contrib);
        assert_eq!(reg.get("editor/context").unwrap().len(), 1);
        assert!(reg.get("other").is_none());
    }

    #[test]
    fn menu_registry_get_sorted() {
        let mut reg = MenuRegistry::new();
        reg.add("ctx", MenuContribution { group_id: "g".into(), order: 3, item: MenuItem::action("c", "C") });
        reg.add("ctx", MenuContribution { group_id: "g".into(), order: 1, item: MenuItem::action("a", "A") });
        reg.add("ctx", MenuContribution { group_id: "g".into(), order: 2, item: MenuItem::action("b", "B") });
        let sorted = reg.get_sorted("ctx");
        let ids: Vec<&str> = sorted.iter().map(|c| c.item.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn menu_provider_default_impl() {
        struct DummyProvider;
        impl MenuProvider for DummyProvider {}
        let p = DummyProvider;
        assert!(p.get_menu_items().is_empty());
    }

    #[test]
    fn eq_menuitemkind_same() {
        assert_eq!(MenuItemKind::Action, MenuItemKind::Action);
    }

    #[test]
    fn ne_menuitemkind_diff() {
        assert_ne!(MenuItemKind::Action, MenuItemKind::Submenu);
    }

    #[test]
    fn menu_item_group_new() {
        let g = MenuItemGroup::new("navigation", 1);
        assert_eq!(g.group_id, "navigation");
        assert_eq!(g.order, 1);
        assert!(g.is_empty());
    }

    #[test]
    fn menu_item_group_add_items() {
        let mut g = MenuItemGroup::new("nav", 0);
        g.add_item(MenuItem::action("go_back", "Go Back"));
        g.add_item(MenuItem::action("go_forward", "Go Forward"));
        assert_eq!(g.item_count(), 2);
        assert!(!g.is_empty());
    }

    #[test]
    fn flatten_groups_with_separators() {
        let mut g1 = MenuItemGroup::new("nav", 1);
        g1.add_item(MenuItem::action("a", "A"));
        let mut g2 = MenuItemGroup::new("edit", 2);
        g2.add_item(MenuItem::action("b", "B"));
        g2.add_item(MenuItem::action("c", "C"));
        let flat = MenuItemGroup::flatten_groups(&[g2, g1]); // order matters
        assert_eq!(flat.len(), 4); // A, separator, B, C
        assert_eq!(flat[0].id, "a");
        assert_eq!(flat[1].kind, MenuItemKind::Separator);
        assert_eq!(flat[2].id, "b");
    }

    #[test]
    fn flatten_single_group_no_separator() {
        let mut g = MenuItemGroup::new("only", 0);
        g.add_item(MenuItem::action("x", "X"));
        let flat = MenuItemGroup::flatten_groups(&[g]);
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].id, "x");
    }

    #[test]
    fn flatten_empty_groups_skipped() {
        let g_empty = MenuItemGroup::new("empty", 0);
        let mut g_full = MenuItemGroup::new("full", 1);
        g_full.add_item(MenuItem::action("a", "A"));
        let flat = MenuItemGroup::flatten_groups(&[g_empty, g_full]);
        assert_eq!(flat.len(), 1);
    }

    #[test]
    fn menu_item_sort_by_group_then_order() {
        let mut contribs = vec![
            MenuContribution { group_id: "b".into(), order: 2, item: MenuItem::action("b2", "B2") },
            MenuContribution { group_id: "a".into(), order: 1, item: MenuItem::action("a1", "A1") },
            MenuContribution { group_id: "b".into(), order: 1, item: MenuItem::action("b1", "B1") },
            MenuContribution { group_id: "a".into(), order: 2, item: MenuItem::action("a2", "A2") },
        ];
        menu_item_sort(&mut contribs);
        assert_eq!(contribs[0].item.id, "a1");
        assert_eq!(contribs[1].item.id, "a2");
        assert_eq!(contribs[2].item.id, "b1");
        assert_eq!(contribs[3].item.id, "b2");
    }

    #[test]
    fn menu_action_resolve_basic() {
        let mut bar = MenuBar::new();
        let mut file = MenuItem::submenu("file", "File");
        file.children.push(MenuItem::action("open", "Open"));
        file.children.push(MenuItem::action("save", "Save"));
        bar.add_menu(file);

        let mut cmd_map = HashMap::new();
        cmd_map.insert("open".to_string(), "workbench.action.files.openFile".to_string());
        cmd_map.insert("save".to_string(), "workbench.action.files.save".to_string());

        let resolved = menu_action_resolve(&bar, &cmd_map);
        assert_eq!(resolved.len(), 2);
        assert!(resolved.iter().any(|r| r.command_id == "workbench.action.files.openFile"));
    }

    #[test]
    fn menu_action_resolve_missing_command() {
        let mut bar = MenuBar::new();
        bar.add_menu(MenuItem::action("unknown", "Unknown"));
        let cmd_map = HashMap::new();
        let resolved = menu_action_resolve(&bar, &cmd_map);
        assert!(resolved.is_empty());
    }

    #[test]
    fn resolved_menu_action_display_with_keybinding() {
        let action = ResolvedMenuAction {
            menu_item_id: "open".into(),
            command_id: "workbench.action.files.openFile".into(),
            label: "Open File".into(),
            keybinding: Some("Ctrl+O".into()),
            enabled: true,
        };
        let s = format!("{}", action);
        assert!(s.contains("Open File"));
        assert!(s.contains("Ctrl+O"));
    }

    #[test]
    fn resolved_menu_action_display_without_keybinding() {
        let action = ResolvedMenuAction {
            menu_item_id: "x".into(),
            command_id: "cmd.x".into(),
            label: "Do X".into(),
            keybinding: None,
            enabled: true,
        };
        let s = format!("{}", action);
        assert!(s.contains("Do X"));
        assert!(!s.contains("["));
    }

    #[test]
    fn menu_item_group_display() {
        let mut g = MenuItemGroup::new("nav", 1);
        g.add_item(MenuItem::action("a", "A"));
        let s = format!("{}", g);
        assert!(s.contains("nav"));
        assert!(s.contains("items=1"));
    }

    #[test]
    fn menu_stats_new_defaults() {
        let stats = MenuStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn menu_stats_record_success() {
        let mut stats = MenuStats::new();
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
    fn menu_stats_record_failure() {
        let mut stats = MenuStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn menu_stats_reset() {
        let mut stats = MenuStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn menu_stats_merge() {
        let mut a = MenuStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = MenuStats::new();
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
    fn menu_stats_display() {
        let mut stats = MenuStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn menu_stats_default() {
        let stats = MenuStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn menu_validator_accepts_valid_name() {
        let v = MenuValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn menu_validator_rejects_empty() {
        let v = MenuValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn menu_validator_rejects_too_long() {
        let v = MenuValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn menu_validator_forbidden_prefix() {
        let v = MenuValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn menu_validator_allowed_chars() {
        let v = MenuValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn menu_validator_range() {
        let v = MenuValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn menu_sanitize_removes_control() {
        let result = MenuValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn menu_truncate_short_string() {
        assert_eq!(MenuValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn menu_truncate_long_string() {
        let result = MenuValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn menu_is_ascii_printable() {
        assert!(MenuValidator::is_ascii_printable("Hello World 123"));
        assert!(!MenuValidator::is_ascii_printable("Hello\x00World"));
    }
}
