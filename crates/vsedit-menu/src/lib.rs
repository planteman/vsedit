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

// ---------------------------------------------------------------------------
// Keyboard shortcut label formatting
// ---------------------------------------------------------------------------

/// Modifier keys for a keyboard shortcut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyModifiers {
    pub fn none() -> Self {
        Self { ctrl: false, shift: false, alt: false, meta: false }
    }

    pub fn ctrl() -> Self {
        Self { ctrl: true, shift: false, alt: false, meta: false }
    }

    pub fn ctrl_shift() -> Self {
        Self { ctrl: true, shift: true, alt: false, meta: false }
    }
}

/// A parsed keyboard shortcut with modifiers and a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardShortcut {
    pub modifiers: KeyModifiers,
    pub key: String,
}

impl KeyboardShortcut {
    pub fn new(modifiers: KeyModifiers, key: impl Into<String>) -> Self {
        Self { modifiers, key: key.into() }
    }

    /// Parse a shortcut string like "Ctrl+Shift+P" into a `KeyboardShortcut`.
    pub fn parse(input: &str) -> Option<Self> {
        let parts: Vec<&str> = input.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return None;
        }
        let mut modifiers = KeyModifiers::none();
        for &part in &parts[..parts.len() - 1] {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "shift" => modifiers.shift = true,
                "alt" | "option" => modifiers.alt = true,
                "meta" | "cmd" | "command" | "super" | "win" => modifiers.meta = true,
                _ => return None,
            }
        }
        let key = parts.last()?.to_string();
        if key.is_empty() {
            return None;
        }
        Some(Self { modifiers, key })
    }
}

/// Platform hint for formatting shortcut labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutPlatform {
    Windows,
    Mac,
    Linux,
}

/// Format a keyboard shortcut for display in a menu item.
///
/// On Mac, uses symbols (⌘, ⇧, ⌥, ⌃). On Windows/Linux, uses text labels.
pub fn menu_keyboard_shortcut_label(shortcut: &KeyboardShortcut, platform: ShortcutPlatform) -> String {
    let mut parts = Vec::new();
    match platform {
        ShortcutPlatform::Mac => {
            if shortcut.modifiers.ctrl { parts.push("⌃".to_string()); }
            if shortcut.modifiers.alt { parts.push("⌥".to_string()); }
            if shortcut.modifiers.shift { parts.push("⇧".to_string()); }
            if shortcut.modifiers.meta { parts.push("⌘".to_string()); }
            parts.push(shortcut.key.to_uppercase());
            parts.join("")
        }
        ShortcutPlatform::Windows | ShortcutPlatform::Linux => {
            if shortcut.modifiers.ctrl { parts.push("Ctrl".to_string()); }
            if shortcut.modifiers.alt { parts.push("Alt".to_string()); }
            if shortcut.modifiers.shift { parts.push("Shift".to_string()); }
            if shortcut.modifiers.meta { parts.push("Win".to_string()); }
            parts.push(shortcut.key.to_string());
            parts.join("+")
        }
    }
}

/// Format a `MenuItem`'s label with its keybinding for display.
/// Returns something like "Save          Ctrl+S" padded to `width`.
pub fn format_menu_item_with_shortcut(item: &MenuItem, width: usize) -> String {
    match &item.keybinding {
        Some(kb) => {
            let label_len = item.label.len();
            let kb_len = kb.len();
            let total = label_len + kb_len;
            if total + 2 <= width {
                let padding = width - total;
                format!("{}{:>pad$}", item.label, kb, pad = padding)
            } else {
                format!("{} {}", item.label, kb)
            }
        }
        None => item.label.clone(),
    }
}

// ---------------------------------------------------------------------------
// MenuItemKind Display
// ---------------------------------------------------------------------------

impl fmt::Display for MenuItemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItemKind::Action => write!(f, "action"),
            MenuItemKind::Submenu => write!(f, "submenu"),
            MenuItemKind::Separator => write!(f, "separator"),
        }
    }
}

// ---------------------------------------------------------------------------
// Deep action iterator
// ---------------------------------------------------------------------------

/// An iterator that yields all leaf `Action` items from a menu tree.
pub struct MenuActionIter<'a> {
    stack: Vec<&'a MenuItem>,
}

impl<'a> MenuActionIter<'a> {
    pub fn new(items: &'a [MenuItem]) -> Self {
        let stack: Vec<&'a MenuItem> = items.iter().rev().collect();
        Self { stack }
    }
}

impl<'a> Iterator for MenuActionIter<'a> {
    type Item = &'a MenuItem;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.stack.pop() {
            for child in item.children.iter().rev() {
                self.stack.push(child);
            }
            if item.kind == MenuItemKind::Action {
                return Some(item);
            }
        }
        None
    }
}

impl MenuBar {
    /// Returns an iterator over all leaf action items in the menu bar.
    pub fn actions(&self) -> MenuActionIter<'_> {
        MenuActionIter::new(&self.menus)
    }

    /// Count how many leaf action items exist across the entire menu bar.
    pub fn action_count(&self) -> usize {
        self.actions().count()
    }

    /// Collect all unique menu item IDs (non-empty) in the tree.
    pub fn all_ids(&self) -> Vec<&str> {
        fn collect_ids<'a>(items: &'a [MenuItem], out: &mut Vec<&'a str>) {
            for item in items {
                if !item.id.is_empty() {
                    out.push(&item.id);
                }
                collect_ids(&item.children, out);
            }
        }
        let mut ids = Vec::new();
        collect_ids(&self.menus, &mut ids);
        ids
    }
}

impl MenuItem {
    /// Returns true if this is a leaf action (no children, Action kind).
    pub fn is_leaf_action(&self) -> bool {
        self.kind == MenuItemKind::Action && self.children.is_empty()
    }

    /// Returns the depth of this item's submenu tree.
    pub fn max_depth(&self) -> usize {
        if self.children.is_empty() {
            0
        } else {
            1 + self.children.iter().map(|c| c.max_depth()).max().unwrap_or(0)
        }
    }

    /// Set the keybinding using builder style.
    pub fn with_keybinding(mut self, kb: impl Into<String>) -> Self {
        self.keybinding = Some(kb.into());
        self
    }

    /// Set enabled flag using builder style.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

// ---------------------------------------------------------------------------
// Menu item searching
// ---------------------------------------------------------------------------

/// Search all menu items (recursively) matching a query string (case-insensitive).
pub fn search_menu_items<'a>(items: &'a [MenuItem], query: &str) -> Vec<&'a MenuItem> {
    let query_lower = query.to_ascii_lowercase();
    let mut results = Vec::new();
    fn collect<'a>(items: &'a [MenuItem], query: &str, out: &mut Vec<&'a MenuItem>) {
        for item in items {
            if item.kind != MenuItemKind::Separator
                && item.label.to_ascii_lowercase().contains(query)
            {
                out.push(item);
            }
            collect(&item.children, query, out);
        }
    }
    collect(items, &query_lower, &mut results);
    results
}

// ---------------------------------------------------------------------------
// Submenu flattening
// ---------------------------------------------------------------------------

/// Flatten a menu hierarchy into a list of `(path, item)` pairs where path
/// describes the breadcrumb trail to reach the item.
pub fn flatten_menu<'a>(items: &'a [MenuItem], prefix: &str) -> Vec<(String, &'a MenuItem)> {
    let mut result = Vec::new();
    for item in items {
        let path = if prefix.is_empty() {
            item.label.clone()
        } else {
            format!("{} > {}", prefix, item.label)
        };
        if item.kind == MenuItemKind::Action {
            result.push((path.clone(), item));
        }
        if !item.children.is_empty() {
            result.extend(flatten_menu(&item.children, &path));
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Menu path resolution
// ---------------------------------------------------------------------------

/// Resolve a dot-separated path (e.g. "File.Save") to a menu item.
pub fn resolve_menu_path<'a>(items: &'a [MenuItem], path: &str) -> Option<&'a MenuItem> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = items;
    for (i, part) in parts.iter().enumerate() {
        let found = current.iter().find(|item| item.id == *part);
        match found {
            Some(item) if i == parts.len() - 1 => return Some(item),
            Some(item) => current = &item.children,
            None => return None,
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Menu keyboard navigation state
// ---------------------------------------------------------------------------

/// Tracks keyboard navigation state for a menu bar.
#[derive(Debug, Clone)]
pub struct MenuNavigationState {
    /// Index of the currently focused top-level menu (-1 = none).
    pub focused_menu: Option<usize>,
    /// Index of the focused item within the open menu.
    pub focused_item: Option<usize>,
    /// Whether a menu is currently open.
    pub is_open: bool,
    /// Total number of top-level menus.
    menu_count: usize,
}

impl MenuNavigationState {
    /// Create a new navigation state for a menu bar with `menu_count` menus.
    pub fn new(menu_count: usize) -> Self {
        Self {
            focused_menu: None,
            focused_item: None,
            is_open: false,
            menu_count,
        }
    }

    /// Move focus to the next top-level menu.
    pub fn move_right(&mut self) {
        if self.menu_count == 0 {
            return;
        }
        self.focused_menu = Some(match self.focused_menu {
            Some(i) => (i + 1) % self.menu_count,
            None => 0,
        });
        self.focused_item = None;
    }

    /// Move focus to the previous top-level menu.
    pub fn move_left(&mut self) {
        if self.menu_count == 0 {
            return;
        }
        self.focused_menu = Some(match self.focused_menu {
            Some(0) => self.menu_count - 1,
            Some(i) => i - 1,
            None => self.menu_count - 1,
        });
        self.focused_item = None;
    }

    /// Move focus down within the open menu.
    pub fn move_down(&mut self, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.focused_item = Some(match self.focused_item {
            Some(i) => (i + 1) % item_count,
            None => 0,
        });
    }

    /// Move focus up within the open menu.
    pub fn move_up(&mut self, item_count: usize) {
        if item_count == 0 {
            return;
        }
        self.focused_item = Some(match self.focused_item {
            Some(0) => item_count - 1,
            Some(i) => i - 1,
            None => item_count - 1,
        });
    }

    /// Toggle the open state of the focused menu.
    pub fn toggle_open(&mut self) {
        self.is_open = !self.is_open;
        if !self.is_open {
            self.focused_item = None;
        }
    }

    /// Close the menu and reset item focus.
    pub fn close(&mut self) {
        self.is_open = false;
        self.focused_item = None;
    }
}

// ---------------------------------------------------------------------------
// MenuDiff — detect differences between two menu hierarchies
// ---------------------------------------------------------------------------

/// Describes a single difference between two menu trees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuDiffKind {
    /// Item exists only in the left tree.
    Added(String),
    /// Item exists only in the right tree.
    Removed(String),
    /// Item exists in both but label changed.
    LabelChanged { id: String, old: String, new: String },
    /// Item exists in both but enabled state changed.
    EnabledChanged { id: String, was: bool, now: bool },
}

/// Compute differences between two flat lists of menu items.
pub fn diff_menus(old: &[MenuItem], new: &[MenuItem]) -> Vec<MenuDiffKind> {
    let mut diffs = Vec::new();
    let old_map: std::collections::HashMap<&str, &MenuItem> =
        old.iter().map(|i| (i.id.as_str(), i)).collect();
    let new_map: std::collections::HashMap<&str, &MenuItem> =
        new.iter().map(|i| (i.id.as_str(), i)).collect();

    for (id, _new_item) in &new_map {
        if !old_map.contains_key(id) {
            diffs.push(MenuDiffKind::Added(id.to_string()));
        }
    }
    for (id, old_item) in &old_map {
        match new_map.get(id) {
            None => diffs.push(MenuDiffKind::Removed(id.to_string())),
            Some(new_item) => {
                if old_item.label != new_item.label {
                    diffs.push(MenuDiffKind::LabelChanged {
                        id: id.to_string(),
                        old: old_item.label.clone(),
                        new: new_item.label.clone(),
                    });
                }
                if old_item.enabled != new_item.enabled {
                    diffs.push(MenuDiffKind::EnabledChanged {
                        id: id.to_string(),
                        was: old_item.enabled,
                        now: new_item.enabled,
                    });
                }
            }
        }
    }
    diffs
}

// ---------------------------------------------------------------------------
// MenuAccessKey — extract mnemonic / access keys from labels
// ---------------------------------------------------------------------------

/// Extract the access key from a label that uses `&` prefix convention
/// (e.g. "&File" → Some('F')).
pub fn extract_access_key(label: &str) -> Option<char> {
    let mut chars = label.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            if let Some(&next) = chars.peek() {
                if next != '&' {
                    return Some(next);
                }
                chars.next(); // skip escaped &&
            }
        }
    }
    None
}

/// Strip the `&` access key marker from a label for display.
pub fn strip_access_key(label: &str) -> String {
    let mut result = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '&' {
            if let Some(&next) = chars.peek() {
                if next != '&' {
                    result.push(next);
                    chars.next();
                    continue;
                }
                // escaped &&, emit single &
                chars.next();
            }
        }
        result.push(ch);
    }
    result
}

/// Count total actionable items in a menu hierarchy (excludes separators and submenus).
pub fn count_actions(items: &[MenuItem]) -> usize {
    let mut count = 0;
    for item in items {
        if item.kind == MenuItemKind::Action {
            count += 1;
        }
        count += count_actions(&item.children);
    }
    count
}

/// Collect all keybinding strings from a menu hierarchy.
pub fn collect_keybindings(items: &[MenuItem]) -> Vec<(&str, &str)> {
    let mut bindings = Vec::new();
    fn walk<'a>(items: &'a [MenuItem], out: &mut Vec<(&'a str, &'a str)>) {
        for item in items {
            if let Some(ref kb) = item.keybinding {
                out.push((item.id.as_str(), kb.as_str()));
            }
            walk(&item.children, out);
        }
    }
    walk(items, &mut bindings);
    bindings
}

// ---------------------------------------------------------------------------
// When-clause filtering
// ---------------------------------------------------------------------------

/// A when-clause context used to evaluate conditional menu visibility.
/// Maps context keys (e.g. "editorFocus", "resourceScheme") to string values.
#[derive(Debug, Clone, Default)]
pub struct WhenContext {
    values: HashMap<String, String>,
}

impl WhenContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a context key to "true".
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.values.insert(key.into(), "true".to_string());
    }

    /// Remove a context key.
    pub fn unset(&mut self, key: &str) {
        self.values.remove(key);
    }

    /// Set a context key to a string value.
    pub fn set_value(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    /// Check if a key is set (any value).
    pub fn has(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Get the value of a context key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    /// Evaluate a simple when-clause expression.
    ///
    /// Supports:
    /// - `key` — true if key is set
    /// - `!key` — true if key is NOT set
    /// - `key == value` — true if key equals value
    /// - `key != value` — true if key does not equal value
    /// - `expr1 && expr2` — logical AND (only one level, no nesting)
    pub fn evaluate(&self, expr: &str) -> bool {
        let expr = expr.trim();
        if expr.is_empty() {
            return true;
        }
        // Handle && by splitting and requiring all parts to be true
        if expr.contains("&&") {
            return expr.split("&&").all(|part| self.evaluate_single(part.trim()));
        }
        self.evaluate_single(expr)
    }

    fn evaluate_single(&self, expr: &str) -> bool {
        let expr = expr.trim();
        if let Some(rest) = expr.strip_prefix('!') {
            let key = rest.trim();
            return !self.has(key);
        }
        if let Some((key, value)) = expr.split_once("!=") {
            let key = key.trim();
            let value = value.trim();
            return self.get(key) != Some(value);
        }
        if let Some((key, value)) = expr.split_once("==") {
            let key = key.trim();
            let value = value.trim();
            return self.get(key) == Some(value);
        }
        // Simple key presence check
        self.has(expr)
    }
}

/// A menu item with an associated when-clause for conditional visibility.
#[derive(Debug, Clone)]
pub struct ConditionalMenuItem {
    pub item: MenuItem,
    pub when: Option<String>,
}

impl ConditionalMenuItem {
    pub fn new(item: MenuItem) -> Self {
        Self { item, when: None }
    }

    pub fn with_when(mut self, expr: impl Into<String>) -> Self {
        self.when = Some(expr.into());
        self
    }

    /// Returns true if this item should be visible given the context.
    pub fn is_visible(&self, ctx: &WhenContext) -> bool {
        match &self.when {
            None => true,
            Some(expr) => ctx.evaluate(expr),
        }
    }
}

/// Filter a list of conditional menu items by the current context.
pub fn filter_by_when(items: &[ConditionalMenuItem], ctx: &WhenContext) -> Vec<MenuItem> {
    items
        .iter()
        .filter(|ci| ci.is_visible(ctx))
        .map(|ci| ci.item.clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Menu item deduplication
// ---------------------------------------------------------------------------

/// Remove duplicate menu items (by id), keeping the first occurrence.
/// Separators (empty id) are never considered duplicates of each other.
pub fn deduplicate_menu_items(items: &[MenuItem]) -> Vec<MenuItem> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        if item.id.is_empty() {
            // Separators pass through
            result.push(item.clone());
        } else if seen.insert(item.id.clone()) {
            result.push(item.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Consecutive separator collapsing
// ---------------------------------------------------------------------------

/// Remove leading, trailing, and consecutive separators from a menu item list.
pub fn collapse_separators(items: &[MenuItem]) -> Vec<MenuItem> {
    let mut result = Vec::with_capacity(items.len());
    let mut last_was_sep = true; // treat start as separator to strip leading
    for item in items {
        if item.kind == MenuItemKind::Separator {
            if !last_was_sep {
                result.push(item.clone());
                last_was_sep = true;
            }
        } else {
            result.push(item.clone());
            last_was_sep = false;
        }
    }
    // Remove trailing separator
    if result.last().map_or(false, |i| i.kind == MenuItemKind::Separator) {
        result.pop();
    }
    result
}

// ---------------------------------------------------------------------------
// Dynamic menu builder
// ---------------------------------------------------------------------------

/// Builder for constructing menus programmatically with groups and ordering.
#[derive(Debug, Clone)]
pub struct MenuBuilder {
    groups: Vec<MenuItemGroup>,
}

impl MenuBuilder {
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    /// Add an item to a named group. Creates the group if it doesn't exist.
    pub fn add_to_group(
        &mut self,
        group_id: impl Into<String>,
        order: i32,
        item: MenuItem,
    ) -> &mut Self {
        let gid = group_id.into();
        if let Some(group) = self.groups.iter_mut().find(|g| g.group_id == gid) {
            group.add_item(item);
        } else {
            let mut group = MenuItemGroup::new(gid, order);
            group.add_item(item);
            self.groups.push(group);
        }
        self
    }

    /// Build the final flat list of menu items with separators between groups.
    pub fn build(&self) -> Vec<MenuItem> {
        MenuItemGroup::flatten_groups(&self.groups)
    }

    /// Build the final list and wrap it in a submenu.
    pub fn build_submenu(&self, id: impl Into<String>, label: impl Into<String>) -> MenuItem {
        let mut submenu = MenuItem::submenu(id, label);
        submenu.children = self.build();
        submenu
    }

    /// Return the number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Return the total number of items across all groups.
    pub fn total_items(&self) -> usize {
        self.groups.iter().map(|g| g.item_count()).sum()
    }
}

impl Default for MenuBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Menu item path tracking
// ---------------------------------------------------------------------------

impl MenuItem {
    /// Set the checked flag using builder style.
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    /// Add a child item using builder style.
    pub fn with_child(mut self, child: MenuItem) -> Self {
        self.children.push(child);
        self
    }

    /// Count total items in this subtree (including self).
    pub fn subtree_size(&self) -> usize {
        1 + self.children.iter().map(|c| c.subtree_size()).sum::<usize>()
    }
}

// ---------------------------------------------------------------------------
// MenuBar — merge and contribution-point management
// ---------------------------------------------------------------------------

impl MenuBar {
    /// Merge another menu bar into this one. Top-level menus with matching ids
    /// have their children appended; new menus are added at the end.
    pub fn merge(&mut self, other: &MenuBar) {
        for other_menu in &other.menus {
            if let Some(existing) = self.menus.iter_mut().find(|m| m.id == other_menu.id) {
                existing.children.extend(other_menu.children.iter().cloned());
            } else {
                self.menus.push(other_menu.clone());
            }
        }
    }

    /// Return the total number of items in the entire menu tree.
    pub fn total_item_count(&self) -> usize {
        self.menus.iter().map(|m| m.subtree_size()).sum()
    }

    /// Disable all actions that do NOT have a keybinding set.
    pub fn disable_unbound_actions(&mut self) {
        fn walk(items: &mut [MenuItem]) {
            for item in items {
                if item.kind == MenuItemKind::Action && item.keybinding.is_none() {
                    item.enabled = false;
                }
                walk(&mut item.children);
            }
        }
        walk(&mut self.menus);
    }

    /// Collect all duplicate item ids (ids that appear more than once).
    pub fn find_duplicate_ids(&self) -> Vec<String> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        fn walk<'a>(items: &'a [MenuItem], counts: &mut HashMap<&'a str, usize>) {
            for item in items {
                if !item.id.is_empty() {
                    *counts.entry(&item.id).or_insert(0) += 1;
                }
                walk(&item.children, counts);
            }
        }
        walk(&self.menus, &mut counts);
        counts
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|(id, _)| id.to_string())
            .collect()
    }

    /// Set the keybinding on a menu item found by id. Returns true if found.
    pub fn set_keybinding(&mut self, id: &str, keybinding: Option<String>) -> bool {
        if let Some(item) = self.find_item_mut(id) {
            item.keybinding = keybinding;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------------------
// MenuRegistry — additional helpers
// ---------------------------------------------------------------------------

impl MenuRegistry {
    /// Return all registered location keys.
    pub fn locations(&self) -> Vec<&str> {
        self.contributions.keys().map(|s| s.as_str()).collect()
    }

    /// Remove all contributions for a given location.
    pub fn clear_location(&mut self, location: &str) {
        self.contributions.remove(location);
    }

    /// Total number of contributions across all locations.
    pub fn total_contributions(&self) -> usize {
        self.contributions.values().map(|v| v.len()).sum()
    }

    /// Build a flat list of menu items for a location, grouped by group_id
    /// with separators between groups (sorted by order).
    pub fn build_menu_for_location(&self, location: &str) -> Vec<MenuItem> {
        let sorted = self.get_sorted(location);
        if sorted.is_empty() {
            return Vec::new();
        }

        let mut groups: Vec<(&str, Vec<&MenuItem>)> = Vec::new();
        for contrib in &sorted {
            if let Some(group) = groups.iter_mut().find(|(gid, _)| *gid == contrib.group_id) {
                group.1.push(&contrib.item);
            } else {
                groups.push((&contrib.group_id, vec![&contrib.item]));
            }
        }

        let mut result = Vec::new();
        for (i, (_gid, items)) in groups.iter().enumerate() {
            if i > 0 {
                result.push(MenuItem::separator());
            }
            for item in items {
                result.push((*item).clone());
            }
        }
        result
    }
}

// --- MenuSearchIndex ---

pub struct MenuSearchIndex {
    items: Vec<(String, String)>, // (id, label)
}

impl MenuSearchIndex {
    pub fn new() -> Self { Self { items: Vec::new() } }

    pub fn insert(&mut self, id: &str, label: &str) {
        self.items.push((id.to_string(), label.to_string()));
    }

    pub fn search_prefix(&self, prefix: &str) -> Vec<&str> {
        let lower = prefix.to_lowercase();
        self.items.iter()
            .filter(|(_, l)| l.to_lowercase().starts_with(&lower))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    pub fn search_contains(&self, query: &str) -> Vec<&str> {
        let lower = query.to_lowercase();
        self.items.iter()
            .filter(|(_, l)| l.to_lowercase().contains(&lower))
            .map(|(id, _)| id.as_str())
            .collect()
    }

    pub fn search_fuzzy(&self, query: &str) -> Vec<(&str, usize)> {
        let lower = query.to_lowercase();
        let mut results: Vec<(&str, usize)> = self.items.iter().filter_map(|(id, label)| {
            let label_lower = label.to_lowercase();
            let mut score = 0usize;
            let mut qi = lower.chars().peekable();
            for ch in label_lower.chars() {
                if qi.peek() == Some(&ch) { qi.next(); score += 1; }
            }
            if qi.peek().is_none() { Some((id.as_str(), score)) } else { None }
        }).collect();
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results
    }

    pub fn len(&self) -> usize { self.items.len() }
}

// --- MenuAccessKeyParser ---

pub struct MenuAccessKeyParser;

impl MenuAccessKeyParser {
    pub fn extract_key(label: &str) -> Option<char> {
        let mut chars = label.chars();
        while let Some(ch) = chars.next() {
            if ch == '&' {
                if let Some(next) = chars.next() {
                    if next != '&' { return Some(next); }
                }
            }
        }
        None
    }

    pub fn has_access_key(label: &str) -> bool {
        Self::extract_key(label).is_some()
    }

    pub fn strip_access_key(label: &str) -> String {
        label.replace('&', "")
    }

    pub fn format_with_underline(label: &str) -> String {
        let mut result = String::new();
        let mut found = false;
        let mut chars = label.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '&' && !found {
                if let Some(&next) = chars.peek() {
                    if next != '&' {
                        result.push('[');
                        result.push(chars.next().unwrap());
                        result.push(']');
                        found = true;
                        continue;
                    }
                }
            }
            result.push(ch);
        }
        result
    }
}

// --- MenuBarLayoutCalc ---

pub struct MenuBarLayoutCalc {
    item_widths: Vec<u16>,
    overflow_threshold: u16,
}

impl MenuBarLayoutCalc {
    pub fn new(widths: Vec<u16>, overflow_threshold: u16) -> Self {
        Self { item_widths: widths, overflow_threshold }
    }

    pub fn total_width(&self) -> u16 { self.item_widths.iter().sum() }

    pub fn item_at_x(&self, x: u16) -> Option<usize> {
        let mut acc = 0u16;
        for (i, &w) in self.item_widths.iter().enumerate() {
            if x >= acc && x < acc + w { return Some(i); }
            acc += w;
        }
        None
    }

    pub fn overflow_into_more_menu(&self) -> (Vec<u16>, Vec<u16>) {
        let mut visible = Vec::new();
        let mut overflow = Vec::new();
        let mut total = 0u16;
        for &w in &self.item_widths {
            if total + w <= self.overflow_threshold {
                visible.push(w);
                total += w;
            } else {
                overflow.push(w);
            }
        }
        (visible, overflow)
    }

    pub fn item_count(&self) -> usize { self.item_widths.len() }
}


// ---------------------------------------------------------------------------
// menu – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XMenuLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XMenuPanelState {
    pub region: XMenuLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XMenuPanelState {
    pub fn new(region: XMenuLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_menu_total_visible_area(panels: &[XMenuPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_menu_count_in_region(
    panels: &[XMenuPanelState],
    region: XMenuLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_menu_widest_panel(panels: &[XMenuPanelState]) -> Option<&XMenuPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_menu_collapse_region(
    panels: &mut [XMenuPanelState],
    region: XMenuLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XMenuLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XMenuLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}


/// Configuration manager for menu functionality.
pub struct MenuConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl MenuConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &MenuConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for menu operations.
pub struct MenuRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl MenuRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for menu.
pub struct MenuValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl MenuValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &MenuValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
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
    fn menu_validator_accepts_and_rejects() {
        let mut v = MenuValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad menu item");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad menu item"));
    }

    #[test]
    fn menu_validator_warnings() {
        let mut v = MenuValidationCollector::new();
        v.add_warning("deprecated item");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn menu_validator_clear_and_merge() {
        let mut v = MenuValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = MenuValidationCollector::new();
        a.add_error("a_err");
        let mut b = MenuValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn keyboard_shortcut_parse_ctrl_s() {
        let sc = KeyboardShortcut::parse("Ctrl+S").unwrap();
        assert!(sc.modifiers.ctrl);
        assert!(!sc.modifiers.shift);
        assert_eq!(sc.key, "S");
    }

    #[test]
    fn keyboard_shortcut_parse_ctrl_shift_p() {
        let sc = KeyboardShortcut::parse("Ctrl+Shift+P").unwrap();
        assert!(sc.modifiers.ctrl);
        assert!(sc.modifiers.shift);
        assert_eq!(sc.key, "P");
    }

    #[test]
    fn keyboard_shortcut_parse_invalid() {
        assert!(KeyboardShortcut::parse("").is_none());
        assert!(KeyboardShortcut::parse("+").is_none());
    }

    #[test]
    fn shortcut_label_mac() {
        let sc = KeyboardShortcut::new(KeyModifiers::ctrl_shift(), "P");
        let label = menu_keyboard_shortcut_label(&sc, ShortcutPlatform::Mac);
        assert!(label.contains('⌃'));
        assert!(label.contains('⇧'));
        assert!(label.contains('P'));
    }

    #[test]
    fn shortcut_label_windows() {
        let sc = KeyboardShortcut::new(KeyModifiers::ctrl(), "S");
        let label = menu_keyboard_shortcut_label(&sc, ShortcutPlatform::Windows);
        assert_eq!(label, "Ctrl+S");
    }

    #[test]
    fn format_item_with_shortcut() {
        let mut item = MenuItem::action("save", "Save");
        item.keybinding = Some("Ctrl+S".to_string());
        let result = format_menu_item_with_shortcut(&item, 30);
        assert!(result.contains("Save"));
        assert!(result.contains("Ctrl+S"));
    }

    #[test]
    fn format_item_without_shortcut() {
        let item = MenuItem::action("open", "Open");
        let result = format_menu_item_with_shortcut(&item, 30);
        assert_eq!(result, "Open");
    }

    #[test]
    fn menu_item_kind_display() {
        assert_eq!(MenuItemKind::Action.to_string(), "action");
        assert_eq!(MenuItemKind::Submenu.to_string(), "submenu");
        assert_eq!(MenuItemKind::Separator.to_string(), "separator");
    }

    #[test]
    fn menu_action_iter_flat() {
        let mut bar = MenuBar::new();
        bar.add_menu(MenuItem::action("a1", "A1"));
        bar.add_menu(MenuItem::separator());
        bar.add_menu(MenuItem::action("a2", "A2"));
        let ids: Vec<&str> = bar.actions().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["a1", "a2"]);
    }

    #[test]
    fn menu_action_iter_nested() {
        let mut sub = MenuItem::submenu("sub", "Sub");
        sub.children.push(MenuItem::action("c1", "C1"));
        sub.children.push(MenuItem::action("c2", "C2"));
        let mut bar = MenuBar::new();
        bar.add_menu(sub);
        assert_eq!(bar.action_count(), 2);
    }

    #[test]
    fn menu_all_ids() {
        let mut bar = MenuBar::new();
        bar.add_menu(MenuItem::action("x", "X"));
        bar.add_menu(MenuItem::separator()); // empty id
        let ids = bar.all_ids();
        assert_eq!(ids, vec!["x"]);
    }

    #[test]
    fn menu_item_is_leaf_action() {
        let a = MenuItem::action("a", "A");
        assert!(a.is_leaf_action());
        let s = MenuItem::submenu("s", "S");
        assert!(!s.is_leaf_action());
    }

    #[test]
    fn menu_item_max_depth() {
        let mut sub = MenuItem::submenu("s", "S");
        sub.children.push(MenuItem::action("a", "A"));
        assert_eq!(sub.max_depth(), 1);
        assert_eq!(MenuItem::action("a", "A").max_depth(), 0);
    }

    #[test]
    fn menu_item_builder_methods() {
        let item = MenuItem::action("save", "Save")
            .with_keybinding("Ctrl+S")
            .with_enabled(false);
        assert_eq!(item.keybinding.as_deref(), Some("Ctrl+S"));
        assert!(!item.enabled);
    }

    // ── Menu item searching ───────────────────────────────────────

    #[test]
    fn search_menu_items_finds_nested() {
        let mut file_menu = MenuItem::submenu("file", "File");
        file_menu.children.push(MenuItem::action("save", "Save File"));
        file_menu.children.push(MenuItem::action("open", "Open File"));
        let mut edit_menu = MenuItem::submenu("edit", "Edit");
        edit_menu.children.push(MenuItem::action("undo", "Undo"));

        let items = vec![file_menu, edit_menu];
        let results = search_menu_items(&items, "file");
        assert_eq!(results.len(), 3); // File, Save File, Open File
    }

    #[test]
    fn search_menu_items_case_insensitive() {
        let items = vec![MenuItem::action("a", "Save All")];
        let results = search_menu_items(&items, "SAVE");
        assert_eq!(results.len(), 1);
    }

    // ── Submenu flattening ────────────────────────────────────────

    #[test]
    fn flatten_menu_produces_breadcrumbs() {
        let mut file_menu = MenuItem::submenu("file", "File");
        file_menu.children.push(MenuItem::action("save", "Save"));
        file_menu.children.push(MenuItem::action("open", "Open"));
        let mut edit_menu = MenuItem::submenu("edit", "Edit");
        edit_menu.children.push(MenuItem::action("undo", "Undo"));

        let menus = [file_menu, edit_menu];
        let flat = flatten_menu(&menus, "");
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].0, "File > Save");
        assert_eq!(flat[1].0, "File > Open");
        assert_eq!(flat[2].0, "Edit > Undo");
    }

    #[test]
    fn flatten_empty_menu() {
        let flat = flatten_menu(&[], "");
        assert!(flat.is_empty());
    }

    // ── Menu path resolution ──────────────────────────────────────

    #[test]
    fn resolve_menu_path_finds_nested() {
        let mut file_menu = MenuItem::submenu("file", "File");
        file_menu.children.push(MenuItem::action("save", "Save"));
        let items = vec![file_menu];
        let found = resolve_menu_path(&items, "file.save");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "Save");
    }

    #[test]
    fn resolve_menu_path_returns_none_for_missing() {
        let items = vec![MenuItem::action("a", "A")];
        assert!(resolve_menu_path(&items, "b").is_none());
        assert!(resolve_menu_path(&items, "a.b").is_none());
    }

    // ── Keyboard navigation ───────────────────────────────────────

    #[test]
    fn navigation_move_right_wraps() {
        let mut nav = MenuNavigationState::new(3);
        nav.move_right();
        assert_eq!(nav.focused_menu, Some(0));
        nav.move_right();
        assert_eq!(nav.focused_menu, Some(1));
        nav.move_right();
        nav.move_right();
        assert_eq!(nav.focused_menu, Some(0)); // wraps
    }

    #[test]
    fn navigation_move_left_wraps() {
        let mut nav = MenuNavigationState::new(3);
        nav.move_left();
        assert_eq!(nav.focused_menu, Some(2));
        nav.move_left();
        assert_eq!(nav.focused_menu, Some(1));
    }

    #[test]
    fn navigation_move_down_up() {
        let mut nav = MenuNavigationState::new(2);
        nav.move_down(4);
        assert_eq!(nav.focused_item, Some(0));
        nav.move_down(4);
        assert_eq!(nav.focused_item, Some(1));
        nav.move_up(4);
        assert_eq!(nav.focused_item, Some(0));
        nav.move_up(4);
        assert_eq!(nav.focused_item, Some(3)); // wraps
    }

    #[test]
    fn navigation_toggle_open_close() {
        let mut nav = MenuNavigationState::new(2);
        assert!(!nav.is_open);
        nav.toggle_open();
        assert!(nav.is_open);
        nav.move_down(3);
        assert_eq!(nav.focused_item, Some(0));
        nav.close();
        assert!(!nav.is_open);
        assert_eq!(nav.focused_item, None);
    }

    // -- MenuDiff --

    #[test]
    fn diff_menus_detects_added() {
        let old = vec![MenuItem::action("save", "Save")];
        let new = vec![
            MenuItem::action("save", "Save"),
            MenuItem::action("open", "Open"),
        ];
        let diffs = diff_menus(&old, &new);
        assert!(diffs.iter().any(|d| matches!(d, MenuDiffKind::Added(id) if id == "open")));
    }

    #[test]
    fn diff_menus_detects_removed() {
        let old = vec![
            MenuItem::action("save", "Save"),
            MenuItem::action("open", "Open"),
        ];
        let new = vec![MenuItem::action("save", "Save")];
        let diffs = diff_menus(&old, &new);
        assert!(diffs.iter().any(|d| matches!(d, MenuDiffKind::Removed(id) if id == "open")));
    }

    #[test]
    fn diff_menus_detects_label_change() {
        let old = vec![MenuItem::action("save", "Save")];
        let new = vec![MenuItem::action("save", "Save File")];
        let diffs = diff_menus(&old, &new);
        assert!(diffs.iter().any(|d| matches!(d, MenuDiffKind::LabelChanged { id, old, new } if id == "save" && old == "Save" && new == "Save File")));
    }

    #[test]
    fn diff_menus_detects_enabled_change() {
        let old = vec![MenuItem::action("save", "Save")];
        let new = vec![MenuItem::action("save", "Save").with_enabled(false)];
        let diffs = diff_menus(&old, &new);
        assert!(diffs.iter().any(|d| matches!(d, MenuDiffKind::EnabledChanged { id, was: true, now: false } if id == "save")));
    }

    // -- Access keys --

    #[test]
    fn extract_access_key_basic() {
        assert_eq!(extract_access_key("&File"), Some('F'));
        assert_eq!(extract_access_key("Save &As"), Some('A'));
        assert_eq!(extract_access_key("No key"), None);
        assert_eq!(extract_access_key("&&Escaped"), None); // && means literal &
    }

    #[test]
    fn strip_access_key_basic() {
        assert_eq!(strip_access_key("&File"), "File");
        assert_eq!(strip_access_key("Save &As"), "Save As");
        assert_eq!(strip_access_key("No key"), "No key");
    }

    // -- count_actions / collect_keybindings --

    #[test]
    fn count_actions_recursive() {
        let mut file = MenuItem::submenu("file", "File");
        file.children.push(MenuItem::action("open", "Open"));
        file.children.push(MenuItem::action("save", "Save"));
        file.children.push(MenuItem::separator());
        assert_eq!(count_actions(&[file]), 2);
    }

    #[test]
    fn collect_keybindings_from_hierarchy() {
        let mut file = MenuItem::submenu("file", "File");
        file.children.push(MenuItem::action("open", "Open").with_keybinding("Ctrl+O"));
        file.children.push(MenuItem::action("save", "Save"));
        let items = [file];
        let bindings = collect_keybindings(&items);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0], ("open", "Ctrl+O"));
    }

    // ── When-clause filtering ─────────────────────────────────────

    #[test]
    fn when_context_flag_and_has() {
        let mut ctx = WhenContext::new();
        assert!(!ctx.has("editorFocus"));
        ctx.set_flag("editorFocus");
        assert!(ctx.has("editorFocus"));
        ctx.unset("editorFocus");
        assert!(!ctx.has("editorFocus"));
    }

    #[test]
    fn when_context_evaluate_simple_key() {
        let mut ctx = WhenContext::new();
        ctx.set_flag("editorFocus");
        assert!(ctx.evaluate("editorFocus"));
        assert!(!ctx.evaluate("terminalFocus"));
    }

    #[test]
    fn when_context_evaluate_negation() {
        let mut ctx = WhenContext::new();
        ctx.set_flag("editorFocus");
        assert!(!ctx.evaluate("!editorFocus"));
        assert!(ctx.evaluate("!terminalFocus"));
    }

    #[test]
    fn when_context_evaluate_equality() {
        let mut ctx = WhenContext::new();
        ctx.set_value("resourceScheme", "file");
        assert!(ctx.evaluate("resourceScheme == file"));
        assert!(!ctx.evaluate("resourceScheme == untitled"));
        assert!(ctx.evaluate("resourceScheme != untitled"));
    }

    #[test]
    fn when_context_evaluate_and() {
        let mut ctx = WhenContext::new();
        ctx.set_flag("editorFocus");
        ctx.set_value("resourceScheme", "file");
        assert!(ctx.evaluate("editorFocus && resourceScheme == file"));
        assert!(!ctx.evaluate("editorFocus && terminalFocus"));
    }

    #[test]
    fn when_context_evaluate_empty() {
        let ctx = WhenContext::new();
        assert!(ctx.evaluate(""));
        assert!(ctx.evaluate("  "));
    }

    #[test]
    fn conditional_menu_item_visibility() {
        let mut ctx = WhenContext::new();
        ctx.set_flag("editorFocus");

        let ci = ConditionalMenuItem::new(MenuItem::action("cut", "Cut"))
            .with_when("editorFocus");
        assert!(ci.is_visible(&ctx));

        let ci_hidden = ConditionalMenuItem::new(MenuItem::action("paste_terminal", "Paste"))
            .with_when("terminalFocus");
        assert!(!ci_hidden.is_visible(&ctx));

        let ci_always = ConditionalMenuItem::new(MenuItem::action("help", "Help"));
        assert!(ci_always.is_visible(&ctx));
    }

    #[test]
    fn filter_by_when_filters_correctly() {
        let mut ctx = WhenContext::new();
        ctx.set_flag("editorFocus");

        let items = vec![
            ConditionalMenuItem::new(MenuItem::action("cut", "Cut")).with_when("editorFocus"),
            ConditionalMenuItem::new(MenuItem::action("paste_term", "Paste")).with_when("terminalFocus"),
            ConditionalMenuItem::new(MenuItem::action("help", "Help")),
        ];
        let visible = filter_by_when(&items, &ctx);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].id, "cut");
        assert_eq!(visible[1].id, "help");
    }

    // ── Deduplication ─────────────────────────────────────────────

    #[test]
    fn deduplicate_menu_items_removes_dupes() {
        let items = vec![
            MenuItem::action("open", "Open"),
            MenuItem::action("save", "Save"),
            MenuItem::action("open", "Open File"),
            MenuItem::separator(),
            MenuItem::separator(),
        ];
        let deduped = deduplicate_menu_items(&items);
        assert_eq!(deduped.len(), 4); // open, save, sep, sep (seps kept)
        assert_eq!(deduped[0].label, "Open"); // first wins
    }

    // ── Separator collapsing ──────────────────────────────────────

    #[test]
    fn collapse_separators_removes_consecutive() {
        let items = vec![
            MenuItem::separator(),
            MenuItem::action("a", "A"),
            MenuItem::separator(),
            MenuItem::separator(),
            MenuItem::action("b", "B"),
            MenuItem::separator(),
        ];
        let collapsed = collapse_separators(&items);
        assert_eq!(collapsed.len(), 3); // A, sep, B
        assert_eq!(collapsed[0].id, "a");
        assert_eq!(collapsed[1].kind, MenuItemKind::Separator);
        assert_eq!(collapsed[2].id, "b");
    }

    #[test]
    fn collapse_separators_empty_input() {
        assert!(collapse_separators(&[]).is_empty());
    }

    // ── MenuBuilder ───────────────────────────────────────────────

    #[test]
    fn menu_builder_groups_and_build() {
        let mut builder = MenuBuilder::new();
        builder
            .add_to_group("navigation", 1, MenuItem::action("go_back", "Go Back"))
            .add_to_group("navigation", 1, MenuItem::action("go_fwd", "Go Forward"))
            .add_to_group("edit", 0, MenuItem::action("undo", "Undo"));

        assert_eq!(builder.group_count(), 2);
        assert_eq!(builder.total_items(), 3);

        let items = builder.build();
        // edit group (order 0) first, then separator, then navigation (order 1)
        assert_eq!(items[0].id, "undo");
        assert_eq!(items[1].kind, MenuItemKind::Separator);
        assert_eq!(items[2].id, "go_back");
        assert_eq!(items[3].id, "go_fwd");
    }

    #[test]
    fn menu_builder_build_submenu() {
        let mut builder = MenuBuilder::new();
        builder.add_to_group("main", 0, MenuItem::action("a", "A"));
        let submenu = builder.build_submenu("edit", "Edit");
        assert_eq!(submenu.kind, MenuItemKind::Submenu);
        assert_eq!(submenu.children.len(), 1);
        assert_eq!(submenu.children[0].id, "a");
    }

    // ── MenuItem builder methods ──────────────────────────────────

    #[test]
    fn menu_item_with_checked_and_child() {
        let item = MenuItem::submenu("view", "View")
            .with_checked(true)
            .with_child(MenuItem::action("minimap", "Toggle Minimap"));
        assert!(item.checked);
        assert_eq!(item.children.len(), 1);
        assert_eq!(item.children[0].id, "minimap");
    }

    #[test]
    fn menu_item_subtree_size() {
        let item = MenuItem::submenu("file", "File")
            .with_child(MenuItem::action("open", "Open"))
            .with_child(
                MenuItem::submenu("recent", "Recent")
                    .with_child(MenuItem::action("r1", "File 1"))
                    .with_child(MenuItem::action("r2", "File 2")),
            );
        assert_eq!(item.subtree_size(), 5); // file + open + recent + r1 + r2
    }

    // ── MenuBar merge ─────────────────────────────────────────────

    #[test]
    fn menu_bar_merge_combines_menus() {
        let mut bar1 = MenuBar::new();
        let file1 = MenuItem::submenu("file", "File")
            .with_child(MenuItem::action("open", "Open"));
        bar1.add_menu(file1);

        let mut bar2 = MenuBar::new();
        let file2 = MenuItem::submenu("file", "File")
            .with_child(MenuItem::action("save", "Save"));
        bar2.add_menu(file2);
        bar2.add_menu(MenuItem::submenu("edit", "Edit"));

        bar1.merge(&bar2);
        // file menu should now have both children
        let file = bar1.find_item("file").unwrap();
        assert_eq!(file.children.len(), 2);
        // edit menu should be added
        assert!(bar1.find_item("edit").is_some());
    }

    #[test]
    fn menu_bar_total_item_count() {
        let bar = make_bar();
        // file(submenu) + open + save + help = 4
        assert_eq!(bar.total_item_count(), 4);
    }

    #[test]
    fn menu_bar_disable_unbound_actions() {
        let mut bar = MenuBar::new();
        bar.add_menu(MenuItem::action("save", "Save").with_keybinding("Ctrl+S"));
        bar.add_menu(MenuItem::action("help", "Help"));
        bar.disable_unbound_actions();
        assert!(bar.find_item("save").unwrap().enabled);
        assert!(!bar.find_item("help").unwrap().enabled);
    }

    #[test]
    fn menu_bar_find_duplicate_ids() {
        let mut bar = MenuBar::new();
        bar.add_menu(MenuItem::action("open", "Open"));
        let sub = MenuItem::submenu("sub", "Sub")
            .with_child(MenuItem::action("open", "Open Again"));
        bar.add_menu(sub);
        let dupes = bar.find_duplicate_ids();
        assert!(dupes.contains(&"open".to_string()));
    }

    #[test]
    fn menu_bar_set_keybinding() {
        let mut bar = make_bar();
        assert!(bar.set_keybinding("open", Some("Ctrl+O".into())));
        assert_eq!(bar.find_item("open").unwrap().keybinding.as_deref(), Some("Ctrl+O"));
        assert!(bar.set_keybinding("open", None));
        assert!(bar.find_item("open").unwrap().keybinding.is_none());
        assert!(!bar.set_keybinding("nonexistent", Some("X".into())));
    }

    // ── MenuRegistry additional helpers ───────────────────────────

    #[test]
    fn menu_registry_locations_and_clear() {
        let mut reg = MenuRegistry::new();
        reg.add("editor/context", MenuContribution {
            group_id: "nav".into(), order: 1, item: MenuItem::action("a", "A"),
        });
        reg.add("editor/title", MenuContribution {
            group_id: "nav".into(), order: 1, item: MenuItem::action("b", "B"),
        });
        assert_eq!(reg.total_contributions(), 2);
        let locs = reg.locations();
        assert!(locs.contains(&"editor/context"));
        assert!(locs.contains(&"editor/title"));

        reg.clear_location("editor/context");
        assert!(reg.get("editor/context").is_none());
        assert_eq!(reg.total_contributions(), 1);
    }

    #[test]
    fn menu_registry_build_menu_for_location() {
        let mut reg = MenuRegistry::new();
        reg.add("ctx", MenuContribution {
            group_id: "edit".into(), order: 2, item: MenuItem::action("paste", "Paste"),
        });
        reg.add("ctx", MenuContribution {
            group_id: "nav".into(), order: 1, item: MenuItem::action("go_def", "Go to Definition"),
        });
        reg.add("ctx", MenuContribution {
            group_id: "edit".into(), order: 3, item: MenuItem::action("cut", "Cut"),
        });

        let items = reg.build_menu_for_location("ctx");
        // nav group first (order 1), then separator, then edit group (order 2, 3)
        assert_eq!(items[0].id, "go_def");
        assert_eq!(items[1].kind, MenuItemKind::Separator);
        assert_eq!(items[2].id, "paste");
        assert_eq!(items[3].id, "cut");
    }

    #[test]
    fn menu_registry_build_empty_location() {
        let reg = MenuRegistry::new();
        assert!(reg.build_menu_for_location("nonexistent").is_empty());
    }

    #[test]
    fn menu_search_index_prefix() {
        let mut idx = MenuSearchIndex::new();
        idx.insert("open", "Open File");
        idx.insert("save", "Save File");
        idx.insert("options", "Options");
        let results = idx.search_prefix("op");
        assert!(results.contains(&"open"));
        assert!(results.contains(&"options"));
    }

    #[test]
    fn menu_search_index_contains() {
        let mut idx = MenuSearchIndex::new();
        idx.insert("save", "Save File");
        idx.insert("saveas", "Save As...");
        let results = idx.search_contains("save");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn menu_search_index_fuzzy() {
        let mut idx = MenuSearchIndex::new();
        idx.insert("open", "Open File");
        idx.insert("opts", "Options");
        let results = idx.search_fuzzy("opf");
        assert!(!results.is_empty());
    }

    #[test]
    fn menu_access_key_extract() {
        assert_eq!(MenuAccessKeyParser::extract_key("&File"), Some('F'));
        assert_eq!(MenuAccessKeyParser::extract_key("E&xit"), Some('x'));
        assert_eq!(MenuAccessKeyParser::extract_key("Plain"), None);
    }

    #[test]
    fn menu_access_key_has() {
        assert!(MenuAccessKeyParser::has_access_key("&Edit"));
        assert!(!MenuAccessKeyParser::has_access_key("View"));
    }

    #[test]
    fn menu_access_key_strip() {
        assert_eq!(MenuAccessKeyParser::strip_access_key("&File"), "File");
    }

    #[test]
    fn menu_access_key_format_underline() {
        assert_eq!(MenuAccessKeyParser::format_with_underline("&File"), "[F]ile");
    }

    #[test]
    fn menu_bar_layout_total_width() {
        let calc = MenuBarLayoutCalc::new(vec![50, 60, 70], 200);
        assert_eq!(calc.total_width(), 180);
    }

    #[test]
    fn menu_bar_layout_item_at_x() {
        let calc = MenuBarLayoutCalc::new(vec![50, 60, 70], 200);
        assert_eq!(calc.item_at_x(0), Some(0));
        assert_eq!(calc.item_at_x(55), Some(1));
        assert_eq!(calc.item_at_x(115), Some(2));
        assert_eq!(calc.item_at_x(200), None);
    }

    #[test]
    fn menu_bar_layout_overflow() {
        let calc = MenuBarLayoutCalc::new(vec![50, 60, 70, 80], 130);
        let (visible, overflow) = calc.overflow_into_more_menu();
        assert_eq!(visible, vec![50, 60]);
        assert_eq!(overflow, vec![70, 80]);
    }

    #[test]
    fn menu_search_index_len() {
        let mut idx = MenuSearchIndex::new();
        idx.insert("a", "Alpha");
        idx.insert("b", "Beta");
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn menu_bar_layout_item_count() {
        let calc = MenuBarLayoutCalc::new(vec![10, 20, 30], 100);
        assert_eq!(calc.item_count(), 3);
    }

    // -- menu additional tests -------------------------------------------

    #[test]
    fn x_menu_panel_state_new() {
        let p = XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XMenuLayoutRegion::Sidebar);
    }

    #[test]
    fn x_menu_panel_area() {
        let p = XMenuPanelState::new(XMenuLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_menu_panel_toggle() {
        let mut p = XMenuPanelState::new(XMenuLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_menu_panel_resize() {
        let mut p = XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_menu_panel_is_narrow() {
        let mut p = XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_menu_total_visible_area_basic() {
        let panels = vec![
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "a"),
            XMenuPanelState::new(XMenuLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_menu_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_menu_total_visible_area_hidden() {
        let mut panels = vec![
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "a"),
            XMenuPanelState::new(XMenuLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_menu_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_menu_count_in_region_basic() {
        let panels = vec![
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "a"),
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "b"),
            XMenuPanelState::new(XMenuLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_menu_count_in_region(&panels, XMenuLayoutRegion::Sidebar), 2);
        assert_eq!(x_menu_count_in_region(&panels, XMenuLayoutRegion::Editor), 1);
        assert_eq!(x_menu_count_in_region(&panels, XMenuLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_menu_widest_panel_basic() {
        let mut panels = vec![
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "narrow"),
            XMenuPanelState::new(XMenuLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_menu_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_menu_collapse_region_basic() {
        let mut panels = vec![
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "a"),
            XMenuPanelState::new(XMenuLayoutRegion::Sidebar, "b"),
            XMenuPanelState::new(XMenuLayoutRegion::Editor, "c"),
        ];
        x_menu_collapse_region(&mut panels, XMenuLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_menu_layout_constraint_clamp() {
        let lc = XMenuLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_menu_layout_constraint_satisfied() {
        let lc = XMenuLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_menu_widest_panel_empty() {
        let panels: Vec<XMenuPanelState> = vec![];
        assert!(x_menu_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_menu_layout_region_eq() {
        assert_eq!(XMenuLayoutRegion::Sidebar, XMenuLayoutRegion::Sidebar);
        assert_ne!(XMenuLayoutRegion::Sidebar, XMenuLayoutRegion::Panel);
    }


    #[test]
    fn menu_config_new() {
        let cfg = MenuConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn menu_config_set_get() {
        let mut cfg = MenuConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn menu_config_remove() {
        let mut cfg = MenuConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn menu_config_keys_sorted() {
        let mut cfg = MenuConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn menu_config_bump_version() {
        let mut cfg = MenuConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn menu_config_clear() {
        let mut cfg = MenuConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn menu_config_merge() {
        let mut cfg1 = MenuConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = MenuConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn menu_config_disable() {
        let mut cfg = MenuConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn menu_rate_tracker_empty() {
        let rt = MenuRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn menu_rate_tracker_record() {
        let mut rt = MenuRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn menu_rate_tracker_prune() {
        let mut rt = MenuRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn menu_validator_valid() {
        let v = MenuValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn menu_validator_errors() {
        let mut v = MenuValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn menu_validator_clear() {
        let mut v = MenuValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn menu_validator_merge() {
        let mut v1 = MenuValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = MenuValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn menu_rate_tracker_clear() {
        let mut rt = MenuRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }

}
