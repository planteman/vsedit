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


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for menu
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaMenuRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaMenuRingBuf {
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
pub struct XaMenuCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaMenuCounter {
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

impl Default for XaMenuCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 122
// ---------------------------------------------------------------------------

/// Generic object pool `Xc122Pool<T>`.
pub struct Xc122Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc122Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc122PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc122Pool<T> {
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
    pub fn stats(&self) -> Xc122PoolStats {
        Xc122PoolStats {
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

impl<T> Default for Xc122Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc122Scheduler`.
pub struct Xc122Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc122Scheduler {
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

impl Default for Xc122Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_122 hash for the given byte slice.
pub fn xc_122_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_122 convention.
pub fn xc_122_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_49 deepening: state machine + event bus ---

/// States for the Xd49 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd49State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd49State {
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
pub struct Xd49Transition {
    pub from: Xd49State,
    pub to: Xd49State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd49StateMachine {
    current: Xd49State,
    history: Vec<Xd49Transition>,
    step_counter: usize,
}

impl Xd49StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd49State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd49State {
        self.current
    }

    pub fn history(&self) -> &[Xd49Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd49State) -> Result<Xd49State, String> {
        let allowed = match (self.current, target) {
            (Xd49State::Idle, Xd49State::Running) => true,
            (Xd49State::Running, Xd49State::Paused) => true,
            (Xd49State::Running, Xd49State::Done) => true,
            (Xd49State::Paused, Xd49State::Running) => true,
            (Xd49State::Paused, Xd49State::Done) => true,
            (Xd49State::Done, Xd49State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_49: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd49Transition {
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
            "Xd49SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd49State> {
        let prefix = "Xd49SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd49State::Idle),
            "Running" => Some(Xd49State::Running),
            "Paused" => Some(Xd49State::Paused),
            "Done" => Some(Xd49State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd49State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd49 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd49Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd49Event {
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

type Xd49HandlerFn = Box<dyn Fn(&Xd49Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd49EventBus {
    handlers: Vec<(usize, Option<String>, Xd49HandlerFn)>,
    next_id: usize,
    published: Vec<Xd49Event>,
}

impl Xd49EventBus {
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
        F: Fn(&Xd49Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd49Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd49Event) {
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

    pub fn published_events(&self) -> &[Xd49Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #47
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf47Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf47TrieNode {
    children: std::collections::HashMap<char, Xf47TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf47Trie {
    root: Xf47TrieNode,
    count: usize,
}

impl Xf47Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf47TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf47TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf47TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf47BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf47BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 121).
pub struct Xh121SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh121SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 163 as u64,
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

/// A compact bit set supporting boolean operations (variant 121).
pub struct Xh121BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh121BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 121).
pub struct Xi121Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi121Deque<T> {
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
pub struct Xi121Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi121Interval {
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

/// A simple interval tree (variant 121).
pub struct Xi121IntervalTree {
    xi_intervals: Vec<Xi121Interval>,
}

impl Xi121IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi121Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi121Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi121Interval) -> Vec<&Xi121Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi121Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi121Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi121Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi121Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi121Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi121Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 121) ---

/// Disjoint set / union-find for crate 121.
pub struct Xj121UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj121UnionFind {
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

const XJ121_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 121.
pub struct Xj121BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj121BTreeNode<K, V>>>,
    len: usize,
}

struct Xj121BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj121BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj121BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ121_BTREE_ORDER - 1
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
        let mid = XJ121_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj121BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj121BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj121BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj121BTreeNode::xj_new_leaf();
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


// --- xk_121 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk121SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk121SegmentTree {
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
pub struct Xk121DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk121DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_121).
#[derive(Debug, Clone)]
pub struct Xl121Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl121Rope {
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

/// Suffix array for efficient string searching (xl_121).
#[derive(Debug, Clone)]
pub struct Xl121SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl121SuffixArray {
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
pub struct Xm121MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm121MatrixSparse {
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
pub struct Xm121Tokenizer {
    text: String,
}

impl Xm121Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 121.
pub struct Xn121Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn121Fenwick {
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

// ----- AVL tree map — crate 121 -----

#[derive(Debug, Clone)]
struct Xn121AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn121AvlNode<K, V>>>,
    right: Option<Box<Xn121AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 121.
#[derive(Debug, Clone)]
pub struct Xn121AVL<K, V> {
    root: Option<Box<Xn121AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn121AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn121AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn121AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn121AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn121AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn121AvlNode<K, V>>) -> Box<Xn121AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn121AvlNode<K, V>>) -> Box<Xn121AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn121AvlNode<K, V>>) -> Box<Xn121AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn121AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn121AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn121AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn121AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn121AvlNode<K, V>>) -> &Xn121AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn121AvlNode<K, V>>) -> (Box<Xn121AvlNode<K, V>>, Option<Box<Xn121AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn121AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn121AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn121AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn121AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn121AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn121AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn121AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo121RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo121Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo121RBNode<K, V> {
    key: K,
    value: V,
    color: Xo121Color,
    left: Option<Box<Xo121RBNode<K, V>>>,
    right: Option<Box<Xo121RBNode<K, V>>>,
}

/// A red-black tree map for crate 121.
#[derive(Debug, Clone)]
pub struct Xo121RedBlack<K, V> {
    root: Option<Box<Xo121RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo121RedBlack<K, V> {
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
            r.color = Xo121Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo121RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo121RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo121RBNode {
                    key, value, color: Xo121Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo121RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo121Color::Red)
    }

    fn xo_balance(mut h: Box<Xo121RBNode<K, V>>) -> Box<Xo121RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo121Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo121RBNode<K, V>>) -> Box<Xo121RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo121Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo121RBNode<K, V>>) -> Box<Xo121RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo121Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo121RBNode<K, V>>) {
        h.color = Xo121Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo121Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo121Color::Black; }
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
            r.color = Xo121Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo121RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo121RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo121RBNode<K, V>) -> (K, V, Option<Box<Xo121RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo121RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo121Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo121RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo121ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 121.
#[derive(Debug, Clone)]
pub struct Xo121ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo121ConsistentHash {
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
            let vkey = format!("{}#xo121#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo121#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 121).
#[derive(Debug)]
pub struct Xp121SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp121Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp121Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp121Node<K, V>>>,
    xp_right: Option<Box<Xp121Node<K, V>>>,
}

impl<K: Ord, V> Xp121Node<K, V> {
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

impl<K: Ord, V> Default for Xp121SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp121SplayTree<K, V> {
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

    fn xp_splay_node(node: Option<Box<Xp121Node<K, V>>>, key: &K) -> Option<Box<Xp121Node<K, V>>> {
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

    fn xp_rotate_right(mut node: Box<Xp121Node<K, V>>) -> Box<Xp121Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp121Node<K, V>>) -> Box<Xp121Node<K, V>> {
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
            self.xp_root = Some(Box::new(Xp121Node::xp_new(key, val)));
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
                let mut new_node = Box::new(Xp121Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp121Node::xp_new(key, val));
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


// --------------- Xq121Treap ---------------

use std::cmp::Ordering as Xq121Ord;

struct Xq121TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq121TreapNode<K, V>>>,
    right: Option<Box<Xq121TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq121Treap<K, V> {
    root: Option<Box<Xq121TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq121TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_121_size<K, V>(node: &Option<Box<Xq121TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_121_update_size<K, V>(node: &mut Xq121TreapNode<K, V>) {
    node.size = 1 + xq_121_size(&node.left) + xq_121_size(&node.right);
}

fn xq_121_rotate_right<K, V>(mut node: Box<Xq121TreapNode<K, V>>) -> Box<Xq121TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_121_update_size(&mut node);
    left.right = Some(node);
    xq_121_update_size(&mut left);
    left
}

fn xq_121_rotate_left<K, V>(mut node: Box<Xq121TreapNode<K, V>>) -> Box<Xq121TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_121_update_size(&mut node);
    right.left = Some(node);
    xq_121_update_size(&mut right);
    right
}

fn xq_121_insert_node<K: Ord, V>(
    node: Option<Box<Xq121TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq121TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq121TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq121Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq121Ord::Less => {
                let (new_left, old) = xq_121_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_121_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_121_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq121Ord::Greater => {
                let (new_right, old) = xq_121_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_121_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_121_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_121_remove_node<K: Ord, V>(
    node: Option<Box<Xq121TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq121TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq121Ord::Less => {
                let (new_left, old) = xq_121_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_121_update_size(&mut n);
                (Some(n), old)
            }
            Xq121Ord::Greater => {
                let (new_right, old) = xq_121_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_121_update_size(&mut n);
                (Some(n), old)
            }
            Xq121Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_121_rotate_right(n);
                    let (new_right, old) = xq_121_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_121_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_121_rotate_left(n);
                    let (new_left, old) = xq_121_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_121_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_121_find_min<K, V>(node: &Option<Box<Xq121TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_121_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_121_find_max<K, V>(node: &Option<Box<Xq121TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_121_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_121_rank<K: Ord, V>(node: &Option<Box<Xq121TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq121Ord::Less => xq_121_rank(&n.left, key),
            Xq121Ord::Equal => xq_121_size(&n.left),
            Xq121Ord::Greater => 1 + xq_121_size(&n.left) + xq_121_rank(&n.right, key),
        },
    }
}

fn xq_121_kth<K, V>(node: &Option<Box<Xq121TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_121_size(&n.left);
        if k < left_size {
            xq_121_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_121_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_121_in_order<K: Clone, V>(node: &Option<Box<Xq121TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_121_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_121_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq121Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 121 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_121_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq121Ord::Equal => return Some(&n.value),
                Xq121Ord::Less => cur = &n.left,
                Xq121Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_121_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_121_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_121_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_121_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_121_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_121_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_121_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq121VEBTree ---------------

pub struct Xq121VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq121VEBTree>>,
    clusters: Vec<Option<Box<Xq121VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq121VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq121VEBTree::xq_new(sqrt_hi))) };
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
                    self.clusters[hi] = Some(Box::new(Xq121VEBTree::xq_new(self.sqrt_lo)));
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


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr121KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr121KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr121BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr121KDNode {
    xr_point: Xr121KDPoint,
    xr_left: Option<Box<Xr121KDNode>>,
    xr_right: Option<Box<Xr121KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr121KDTree {
    xr_root: Option<Box<Xr121KDNode>>,
    xr_size: usize,
}

impl Xr121KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr121KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr121KDNode>>,
        point: Xr121KDPoint,
        depth: usize,
    ) -> Box<Xr121KDNode> {
        match node {
            None => Box::new(Xr121KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr121KDPoint) -> Option<Xr121KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr121KDNode>,
        query: &Xr121KDPoint,
        depth: usize,
        best: &mut Xr121KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr121KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr121KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr121KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr121KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr121KDNode>>, pts: &mut Vec<Xr121KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr121KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr121BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr121BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

/// A persistent (immutable) array that returns new versions on modification.
#[derive(Debug, Clone)]
pub struct Xs121PersistentArray<T: Clone> {
    xs_versions: Vec<Vec<T>>,
}

impl<T: Clone + PartialEq> Xs121PersistentArray<T> {
    /// Create a new empty persistent array.
    pub fn xs_new() -> Self {
        Xs121PersistentArray {
            xs_versions: vec![Vec::new()],
        }
    }

    /// Create from an initial vector.
    pub fn xs_from_vec(data: Vec<T>) -> Self {
        Xs121PersistentArray {
            xs_versions: vec![data],
        }
    }

    /// Set value at index, creating a new version. Returns version index.
    pub fn xs_set(&mut self, index: usize, value: T) -> Option<usize> {
        let current = self.xs_versions.last()?;
        if index >= current.len() {
            return None;
        }
        let mut new_ver = current.clone();
        new_ver[index] = value;
        self.xs_versions.push(new_ver);
        Some(self.xs_versions.len() - 1)
    }

    /// Push a value, creating a new version.
    pub fn xs_push(&mut self, value: T) -> usize {
        let mut new_ver = self.xs_versions.last().cloned().unwrap_or_default();
        new_ver.push(value);
        self.xs_versions.push(new_ver);
        self.xs_versions.len() - 1
    }

    /// Get value at index in the latest version.
    pub fn xs_get(&self, index: usize) -> Option<&T> {
        self.xs_versions.last()?.get(index)
    }

    /// Get value at index in a specific version.
    pub fn xs_get_version(&self, version: usize, index: usize) -> Option<&T> {
        self.xs_versions.get(version)?.get(index)
    }

    /// Return the length of the latest version.
    pub fn xs_len(&self) -> usize {
        self.xs_versions.last().map_or(0, |v| v.len())
    }

    /// Check if the latest version is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_len() == 0
    }

    /// Return the number of versions.
    pub fn xs_version_count(&self) -> usize {
        self.xs_versions.len()
    }

    /// Return the version history as a slice of slices.
    pub fn xs_history(&self) -> Vec<&[T]> {
        self.xs_versions.iter().map(|v| v.as_slice()).collect()
    }

    /// Compute the diff indices between two versions.
    pub fn xs_diff(&self, v1: usize, v2: usize) -> Vec<usize> {
        let ver1 = match self.xs_versions.get(v1) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let ver2 = match self.xs_versions.get(v2) {
            Some(v) => v,
            None => return Vec::new(),
        };
        let max_len = ver1.len().max(ver2.len());
        let mut diffs = Vec::new();
        for i in 0..max_len {
            let a = ver1.get(i);
            let b = ver2.get(i);
            if a != b {
                diffs.push(i);
            }
        }
        diffs
    }

    /// Rollback to a specific version, creating a new version with that data.
    pub fn xs_rollback(&mut self, version: usize) -> Option<usize> {
        let data = self.xs_versions.get(version)?.clone();
        self.xs_versions.push(data);
        Some(self.xs_versions.len() - 1)
    }

    /// Get the latest version data as a slice.
    pub fn xs_as_slice(&self) -> &[T] {
        self.xs_versions.last().map_or(&[], |v| v.as_slice())
    }
}

/// A single-producer single-consumer queue.
#[derive(Debug)]
pub struct Xs121ConcurrentQueue<T> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_capacity: usize,
}

impl<T> Xs121ConcurrentQueue<T> {
    /// Create a new queue with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs121ConcurrentQueue {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_capacity: cap,
        }
    }

    /// Push an item into the queue. Returns false if full.
    pub fn xs_push(&mut self, item: T) -> bool {
        if self.xs_count >= self.xs_capacity {
            return false;
        }
        self.xs_buffer[self.xs_tail] = Some(item);
        self.xs_tail = (self.xs_tail + 1) % self.xs_capacity;
        self.xs_count += 1;
        true
    }

    /// Pop an item from the queue.
    pub fn xs_pop(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_capacity;
        self.xs_count -= 1;
        item
    }

    /// Try to pop without blocking.
    pub fn xs_try_pop(&mut self) -> Option<T> {
        self.xs_pop()
    }

    /// Return the number of items in the queue.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if the queue is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_capacity
    }

    /// Drain all items from the queue into a vector.
    pub fn xs_drain(&mut self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        while let Some(item) = self.xs_pop() {
            result.push(item);
        }
        result
    }

    /// Check if the queue is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count >= self.xs_capacity
    }

    /// Clear the queue.
    pub fn xs_clear(&mut self) {
        while self.xs_pop().is_some() {}
    }
}

/// A map from non-overlapping ranges to values.
#[derive(Debug, Clone)]
pub struct Xs121RangeMap<V: Clone> {
    xs_entries: Vec<(usize, usize, V)>,
}

impl<V: Clone + PartialEq> Xs121RangeMap<V> {
    /// Create a new empty range map.
    pub fn xs_new() -> Self {
        Xs121RangeMap {
            xs_entries: Vec::new(),
        }
    }

    /// Insert a range [start, end) with value. Removes overlapping entries.
    pub fn xs_insert(&mut self, start: usize, end: usize, value: V) {
        if start >= end {
            return;
        }
        self.xs_entries.retain(|&(s, e, _)| e <= start || s >= end);
        self.xs_entries.push((start, end, value));
        self.xs_entries.sort_by_key(|&(s, _, _)| s);
    }

    /// Get the value for a point.
    pub fn xs_get(&self, point: usize) -> Option<&V> {
        for (s, e, v) in &self.xs_entries {
            if point >= *s && point < *e {
                return Some(v);
            }
        }
        None
    }

    /// Remove the range containing the given point.
    pub fn xs_remove(&mut self, point: usize) -> Option<V> {
        let idx = self.xs_entries.iter().position(|(s, e, _)| point >= *s && point < *e)?;
        let (_, _, v) = self.xs_entries.remove(idx);
        Some(v)
    }

    /// Return the gaps (uncovered ranges) between min and max of entries.
    pub fn xs_gaps(&self, range_start: usize, range_end: usize) -> Vec<(usize, usize)> {
        let mut gaps = Vec::new();
        let mut pos = range_start;
        for (s, e, _) in &self.xs_entries {
            if *s > pos && *s < range_end {
                gaps.push((pos, *s));
            }
            if *e > pos {
                pos = *e;
            }
        }
        if pos < range_end {
            gaps.push((pos, range_end));
        }
        gaps
    }

    /// Return all covered ranges.
    pub fn xs_covered_ranges(&self) -> Vec<(usize, usize)> {
        self.xs_entries.iter().map(|(s, e, _)| (*s, *e)).collect()
    }

    /// Return total coverage (sum of all range lengths).
    pub fn xs_total_coverage(&self) -> usize {
        self.xs_entries.iter().map(|(s, e, _)| e - s).sum()
    }

    /// Return the number of ranges.
    pub fn xs_len(&self) -> usize {
        self.xs_entries.len()
    }

    /// Check if the map is empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_entries.is_empty()
    }

    /// Check if a point is covered.
    pub fn xs_contains(&self, point: usize) -> bool {
        self.xs_get(point).is_some()
    }

    /// Clear all entries.
    pub fn xs_clear(&mut self) {
        self.xs_entries.clear();
    }
}

/// A fixed-size circular buffer.
#[derive(Debug, Clone)]
pub struct Xs121CircularBuffer<T: Clone> {
    xs_buffer: Vec<Option<T>>,
    xs_head: usize,
    xs_tail: usize,
    xs_count: usize,
    xs_cap: usize,
}

impl<T: Clone> Xs121CircularBuffer<T> {
    /// Create a new circular buffer with given capacity.
    pub fn xs_new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        let mut buffer = Vec::with_capacity(cap);
        for _ in 0..cap {
            buffer.push(None);
        }
        Xs121CircularBuffer {
            xs_buffer: buffer,
            xs_head: 0,
            xs_tail: 0,
            xs_count: 0,
            xs_cap: cap,
        }
    }

    /// Push an item to the back. Overwrites oldest if full.
    pub fn xs_push_back(&mut self, item: T) {
        if self.xs_count == self.xs_cap {
            // Overwrite oldest
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_head = (self.xs_head + 1) % self.xs_cap;
        } else {
            self.xs_buffer[self.xs_tail] = Some(item);
            self.xs_tail = (self.xs_tail + 1) % self.xs_cap;
            self.xs_count += 1;
        }
    }

    /// Pop an item from the front.
    pub fn xs_pop_front(&mut self) -> Option<T> {
        if self.xs_count == 0 {
            return None;
        }
        let item = self.xs_buffer[self.xs_head].take();
        self.xs_head = (self.xs_head + 1) % self.xs_cap;
        self.xs_count -= 1;
        item
    }

    /// Peek at the front item.
    pub fn xs_peek_front(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        self.xs_buffer[self.xs_head].as_ref()
    }

    /// Peek at the back item.
    pub fn xs_peek_back(&self) -> Option<&T> {
        if self.xs_count == 0 {
            return None;
        }
        let idx = if self.xs_tail == 0 { self.xs_cap - 1 } else { self.xs_tail - 1 };
        self.xs_buffer[idx].as_ref()
    }

    /// Check if the buffer is full.
    pub fn xs_is_full(&self) -> bool {
        self.xs_count == self.xs_cap
    }

    /// Return the number of items.
    pub fn xs_len(&self) -> usize {
        self.xs_count
    }

    /// Check if empty.
    pub fn xs_is_empty(&self) -> bool {
        self.xs_count == 0
    }

    /// Return the capacity.
    pub fn xs_capacity(&self) -> usize {
        self.xs_cap
    }

    /// Iterate over items from front to back.
    pub fn xs_iter(&self) -> Vec<&T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item);
            }
        }
        result
    }

    /// Clear the buffer.
    pub fn xs_clear(&mut self) {
        for slot in self.xs_buffer.iter_mut() {
            *slot = None;
        }
        self.xs_head = 0;
        self.xs_tail = 0;
        self.xs_count = 0;
    }

    /// Convert to a Vec.
    pub fn xs_to_vec(&self) -> Vec<T> {
        let mut result = Vec::with_capacity(self.xs_count);
        for i in 0..self.xs_count {
            let idx = (self.xs_head + i) % self.xs_cap;
            if let Some(ref item) = self.xs_buffer[idx] {
                result.push(item.clone());
            }
        }
        result
    }
}


// --- xt_ Fibonacci Heap ---

/// A node in a Fibonacci heap, storing a key and value with parent/child/sibling pointers.
#[derive(Debug, Clone)]
pub struct XtFibNode<K: Ord + Clone, V: Clone> {
    pub xt_key: K,
    pub xt_value: V,
    xt_degree: usize,
    xt_marked: bool,
    xt_children: Vec<usize>,
    xt_parent: Option<usize>,
}

impl<K: Ord + Clone, V: Clone> XtFibNode<K, V> {
    /// Create a new Fibonacci heap node.
    pub fn xt_new(key: K, value: V) -> Self {
        Self {
            xt_key: key,
            xt_value: value,
            xt_degree: 0,
            xt_marked: false,
            xt_children: Vec::new(),
            xt_parent: None,
        }
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibNode<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibNode(key={}, val={}, deg={})", self.xt_key, self.xt_value, self.xt_degree)
    }
}

/// Fibonacci heap with lazy consolidation for amortized O(1) insert and decrease-key.
#[derive(Debug, Clone)]
pub struct XtFibonacciHeap<K: Ord + Clone, V: Clone> {
    xt_nodes: Vec<XtFibNode<K, V>>,
    xt_roots: Vec<usize>,
    xt_min_idx: Option<usize>,
    xt_size: usize,
}

impl<K: Ord + Clone, V: Clone> Default for XtFibonacciHeap<K, V> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<K: Ord + Clone + std::fmt::Display, V: Clone + std::fmt::Display> std::fmt::Display for XtFibonacciHeap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FibHeap(size={}, roots={})", self.xt_size, self.xt_roots.len())
    }
}

impl<K: Ord + Clone, V: Clone> XtFibonacciHeap<K, V> {
    /// Create an empty Fibonacci heap.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_roots: Vec::new(),
            xt_min_idx: None,
            xt_size: 0,
        }
    }

    /// Return the number of elements.
    pub fn xt_len(&self) -> usize {
        self.xt_size
    }

    /// Check if the heap is empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_size == 0
    }

    /// Insert a key-value pair, returning its node index.
    pub fn xt_insert(&mut self, key: K, value: V) -> usize {
        let idx = self.xt_nodes.len();
        self.xt_nodes.push(XtFibNode::xt_new(key, value));
        self.xt_roots.push(idx);
        match self.xt_min_idx {
            None => self.xt_min_idx = Some(idx),
            Some(mi) => {
                if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                    self.xt_min_idx = Some(idx);
                }
            }
        }
        self.xt_size += 1;
        idx
    }

    /// Peek at the minimum key-value pair.
    pub fn xt_find_min(&self) -> Option<(&K, &V)> {
        self.xt_min_idx.map(|i| (&self.xt_nodes[i].xt_key, &self.xt_nodes[i].xt_value))
    }

    /// Extract the minimum element.
    pub fn xt_extract_min(&mut self) -> Option<(K, V)> {
        let mi = self.xt_min_idx?;
        let children = self.xt_nodes[mi].xt_children.clone();
        for &c in &children {
            self.xt_nodes[c].xt_parent = None;
            self.xt_roots.push(c);
        }
        self.xt_roots.retain(|&r| r != mi);
        if self.xt_roots.is_empty() {
            self.xt_min_idx = None;
        } else {
            self.xt_min_idx = Some(self.xt_roots[0]);
            self.xt_consolidate();
        }
        self.xt_size -= 1;
        let node = &self.xt_nodes[mi];
        Some((node.xt_key.clone(), node.xt_value.clone()))
    }

    fn xt_consolidate(&mut self) {
        let max_deg = (self.xt_size as f64).log2().ceil() as usize + 2;
        let mut degree_table: Vec<Option<usize>> = vec![None; max_deg + 1];
        let roots = self.xt_roots.clone();
        self.xt_roots.clear();
        for root in roots {
            let mut x = root;
            let mut d = self.xt_nodes[x].xt_degree;
            while d < degree_table.len() {
                if let Some(y) = degree_table[d] {
                    degree_table[d] = None;
                    let (parent, child) = if self.xt_nodes[x].xt_key <= self.xt_nodes[y].xt_key {
                        (x, y)
                    } else {
                        (y, x)
                    };
                    self.xt_nodes[parent].xt_children.push(child);
                    self.xt_nodes[child].xt_parent = Some(parent);
                    self.xt_nodes[parent].xt_degree += 1;
                    self.xt_nodes[child].xt_marked = false;
                    x = parent;
                    d = self.xt_nodes[x].xt_degree;
                } else {
                    break;
                }
            }
            if d < degree_table.len() {
                degree_table[d] = Some(x);
            }
            self.xt_roots.push(x);
        }
        self.xt_roots.sort();
        self.xt_roots.dedup();
        self.xt_min_idx = self.xt_roots.iter().copied()
            .min_by(|&a, &b| self.xt_nodes[a].xt_key.cmp(&self.xt_nodes[b].xt_key));
    }

    /// Decrease the key of a node (key must be smaller than current).
    pub fn xt_decrease_key(&mut self, idx: usize, new_key: K) {
        if new_key >= self.xt_nodes[idx].xt_key {
            return;
        }
        self.xt_nodes[idx].xt_key = new_key;
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[p].xt_key {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
        if let Some(mi) = self.xt_min_idx {
            if self.xt_nodes[idx].xt_key < self.xt_nodes[mi].xt_key {
                self.xt_min_idx = Some(idx);
            }
        }
    }

    fn xt_cut(&mut self, x: usize, p: usize) {
        self.xt_nodes[p].xt_children.retain(|&c| c != x);
        self.xt_nodes[p].xt_degree = self.xt_nodes[p].xt_children.len();
        self.xt_nodes[x].xt_parent = None;
        self.xt_nodes[x].xt_marked = false;
        self.xt_roots.push(x);
    }

    fn xt_cascading_cut(&mut self, idx: usize) {
        if let Some(p) = self.xt_nodes[idx].xt_parent {
            if !self.xt_nodes[idx].xt_marked {
                self.xt_nodes[idx].xt_marked = true;
            } else {
                self.xt_cut(idx, p);
                self.xt_cascading_cut(p);
            }
        }
    }

    /// Merge another Fibonacci heap into this one.
    pub fn xt_merge(&mut self, other: &mut XtFibonacciHeap<K, V>) {
        let offset = self.xt_nodes.len();
        for mut node in other.xt_nodes.drain(..) {
            node.xt_parent = node.xt_parent.map(|p| p + offset);
            node.xt_children = node.xt_children.iter().map(|&c| c + offset).collect();
            self.xt_nodes.push(node);
        }
        for r in other.xt_roots.drain(..) {
            self.xt_roots.push(r + offset);
        }
        match (self.xt_min_idx, other.xt_min_idx) {
            (None, Some(oi)) => self.xt_min_idx = Some(oi + offset),
            (Some(si), Some(oi)) => {
                let oi2 = oi + offset;
                if self.xt_nodes[oi2].xt_key < self.xt_nodes[si].xt_key {
                    self.xt_min_idx = Some(oi2);
                }
            }
            _ => {}
        }
        self.xt_size += other.xt_size;
        other.xt_size = 0;
        other.xt_min_idx = None;
    }

    /// Return all keys in sorted order (destructive).
    pub fn xt_drain_sorted(&mut self) -> Vec<(K, V)> {
        let mut result = Vec::with_capacity(self.xt_size);
        while let Some(pair) = self.xt_extract_min() {
            result.push(pair);
        }
        result
    }

    /// Clear the heap.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_roots.clear();
        self.xt_min_idx = None;
        self.xt_size = 0;
    }
}

// --- xt_ Doubly-Linked List with Cursors ---

/// A node in a doubly-linked list with prev/next indices.
#[derive(Debug, Clone)]
pub struct XtDllNode<T: Clone> {
    pub xt_value: T,
    xt_prev: Option<usize>,
    xt_next: Option<usize>,
    xt_active: bool,
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDllNode<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DllNode({})", self.xt_value)
    }
}

/// Doubly-linked list with O(1) insertion/deletion at any position via cursor indices.
#[derive(Debug, Clone)]
pub struct XtDoublyLinkedList<T: Clone> {
    xt_nodes: Vec<XtDllNode<T>>,
    xt_head: Option<usize>,
    xt_tail: Option<usize>,
    xt_len: usize,
    xt_free: Vec<usize>,
}

impl<T: Clone> Default for XtDoublyLinkedList<T> {
    fn default() -> Self {
        Self::xt_new()
    }
}

impl<T: Clone + std::fmt::Display> std::fmt::Display for XtDoublyLinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DLL(len={})", self.xt_len)
    }
}

impl<T: Clone> XtDoublyLinkedList<T> {
    /// Create an empty doubly-linked list.
    pub fn xt_new() -> Self {
        Self {
            xt_nodes: Vec::new(),
            xt_head: None,
            xt_tail: None,
            xt_len: 0,
            xt_free: Vec::new(),
        }
    }

    /// Return the length.
    pub fn xt_len(&self) -> usize {
        self.xt_len
    }

    /// Check if empty.
    pub fn xt_is_empty(&self) -> bool {
        self.xt_len == 0
    }

    fn xt_alloc(&mut self, value: T) -> usize {
        if let Some(idx) = self.xt_free.pop() {
            self.xt_nodes[idx] = XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            };
            idx
        } else {
            let idx = self.xt_nodes.len();
            self.xt_nodes.push(XtDllNode {
                xt_value: value,
                xt_prev: None,
                xt_next: None,
                xt_active: true,
            });
            idx
        }
    }

    /// Push a value to the front, returning its index.
    pub fn xt_push_front(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_head {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_head) => {
                self.xt_nodes[idx].xt_next = Some(old_head);
                self.xt_nodes[old_head].xt_prev = Some(idx);
                self.xt_head = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Push a value to the back, returning its index.
    pub fn xt_push_back(&mut self, value: T) -> usize {
        let idx = self.xt_alloc(value);
        match self.xt_tail {
            None => {
                self.xt_head = Some(idx);
                self.xt_tail = Some(idx);
            }
            Some(old_tail) => {
                self.xt_nodes[idx].xt_prev = Some(old_tail);
                self.xt_nodes[old_tail].xt_next = Some(idx);
                self.xt_tail = Some(idx);
            }
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value after the given index, returning the new index.
    pub fn xt_insert_after(&mut self, after: usize, value: T) -> usize {
        if !self.xt_nodes[after].xt_active {
            return self.xt_push_back(value);
        }
        let idx = self.xt_alloc(value);
        let next = self.xt_nodes[after].xt_next;
        self.xt_nodes[after].xt_next = Some(idx);
        self.xt_nodes[idx].xt_prev = Some(after);
        self.xt_nodes[idx].xt_next = next;
        if let Some(n) = next {
            self.xt_nodes[n].xt_prev = Some(idx);
        } else {
            self.xt_tail = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Insert a value before the given index, returning the new index.
    pub fn xt_insert_before(&mut self, before: usize, value: T) -> usize {
        if !self.xt_nodes[before].xt_active {
            return self.xt_push_front(value);
        }
        let idx = self.xt_alloc(value);
        let prev = self.xt_nodes[before].xt_prev;
        self.xt_nodes[before].xt_prev = Some(idx);
        self.xt_nodes[idx].xt_next = Some(before);
        self.xt_nodes[idx].xt_prev = prev;
        if let Some(p) = prev {
            self.xt_nodes[p].xt_next = Some(idx);
        } else {
            self.xt_head = Some(idx);
        }
        self.xt_len += 1;
        idx
    }

    /// Remove the node at the given index.
    pub fn xt_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.xt_nodes.len() || !self.xt_nodes[idx].xt_active {
            return None;
        }
        let prev = self.xt_nodes[idx].xt_prev;
        let next = self.xt_nodes[idx].xt_next;
        match prev {
            Some(p) => self.xt_nodes[p].xt_next = next,
            None => self.xt_head = next,
        }
        match next {
            Some(n) => self.xt_nodes[n].xt_prev = prev,
            None => self.xt_tail = prev,
        }
        self.xt_nodes[idx].xt_active = false;
        self.xt_nodes[idx].xt_prev = None;
        self.xt_nodes[idx].xt_next = None;
        self.xt_free.push(idx);
        self.xt_len -= 1;
        Some(self.xt_nodes[idx].xt_value.clone())
    }

    /// Pop from front.
    pub fn xt_pop_front(&mut self) -> Option<T> {
        self.xt_head.and_then(|h| self.xt_remove(h))
    }

    /// Pop from back.
    pub fn xt_pop_back(&mut self) -> Option<T> {
        self.xt_tail.and_then(|t| self.xt_remove(t))
    }

    /// Peek at the front value.
    pub fn xt_peek_front(&self) -> Option<&T> {
        self.xt_head.map(|h| &self.xt_nodes[h].xt_value)
    }

    /// Peek at the back value.
    pub fn xt_peek_back(&self) -> Option<&T> {
        self.xt_tail.map(|t| &self.xt_nodes[t].xt_value)
    }

    /// Get value at a given index.
    pub fn xt_get(&self, idx: usize) -> Option<&T> {
        if idx < self.xt_nodes.len() && self.xt_nodes[idx].xt_active {
            Some(&self.xt_nodes[idx].xt_value)
        } else {
            None
        }
    }

    /// Iterate from head to tail.
    pub fn xt_iter_forward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_next;
        }
        result
    }

    /// Iterate from tail to head.
    pub fn xt_iter_backward(&self) -> Vec<&T> {
        let mut result = Vec::new();
        let mut cur = self.xt_tail;
        while let Some(idx) = cur {
            result.push(&self.xt_nodes[idx].xt_value);
            cur = self.xt_nodes[idx].xt_prev;
        }
        result
    }

    /// Collect all values into a Vec (front to back).
    pub fn xt_to_vec(&self) -> Vec<T> {
        self.xt_iter_forward().into_iter().cloned().collect()
    }

    /// Clear the list.
    pub fn xt_clear(&mut self) {
        self.xt_nodes.clear();
        self.xt_head = None;
        self.xt_tail = None;
        self.xt_len = 0;
        self.xt_free.clear();
    }

    /// Return the head cursor index.
    pub fn xt_head_cursor(&self) -> Option<usize> {
        self.xt_head
    }

    /// Return the tail cursor index.
    pub fn xt_tail_cursor(&self) -> Option<usize> {
        self.xt_tail
    }

    /// Move cursor to next.
    pub fn xt_cursor_next(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_next
        } else {
            None
        }
    }

    /// Move cursor to prev.
    pub fn xt_cursor_prev(&self, cursor: usize) -> Option<usize> {
        if cursor < self.xt_nodes.len() && self.xt_nodes[cursor].xt_active {
            self.xt_nodes[cursor].xt_prev
        } else {
            None
        }
    }

    /// Reverse the list in place.
    pub fn xt_reverse(&mut self) {
        let mut cur = self.xt_head;
        while let Some(idx) = cur {
            let next = self.xt_nodes[idx].xt_next;
            let prev = self.xt_nodes[idx].xt_prev;
            self.xt_nodes[idx].xt_next = prev;
            self.xt_nodes[idx].xt_prev = next;
            cur = next;
        }
        std::mem::swap(&mut self.xt_head, &mut self.xt_tail);
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


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for menu
    #[test]
    fn xa_menu_ring_new() {
        let rb = super::XaMenuRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_menu_ring_push_len() {
        let mut rb = super::XaMenuRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_menu_ring_wrap() {
        let mut rb = super::XaMenuRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_menu_ring_mean_empty() {
        let rb = super::XaMenuRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_menu_ring_mean_values() {
        let mut rb = super::XaMenuRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_menu_ring_min_max() {
        let mut rb = super::XaMenuRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_menu_ring_iter() {
        let mut rb = super::XaMenuRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_menu_counter_new() {
        let c = super::XaMenuCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_menu_counter_inc() {
        let mut c = super::XaMenuCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_menu_counter_inc_by() {
        let mut c = super::XaMenuCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_menu_counter_reset() {
        let mut c = super::XaMenuCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_menu_counter_clear() {
        let mut c = super::XaMenuCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_menu_counter_default() {
        let c = super::XaMenuCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 122 ----

    #[test]
    fn xc_122_pool_new_empty() {
        let pool: super::Xc122Pool<i32> = super::Xc122Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_122_pool_release_acquire() {
        let mut pool = super::Xc122Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_122_pool_acquire_empty() {
        let mut pool: super::Xc122Pool<i32> = super::Xc122Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_122_pool_full() {
        let mut pool = super::Xc122Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_122_pool_drain() {
        let mut pool = super::Xc122Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_122_pool_stats() {
        let mut pool = super::Xc122Pool::new(8);
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
    fn xc_122_pool_clear() {
        let mut pool = super::Xc122Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_122_pool_shrink() {
        let mut pool = super::Xc122Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_122_pool_default() {
        let pool: super::Xc122Pool<String> = super::Xc122Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_122_pool_extend() {
        let mut pool = super::Xc122Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_122_pool_retain() {
        let mut pool = super::Xc122Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_122_scheduler_round_robin() {
        let mut sched = super::Xc122Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_122_scheduler_empty() {
        let mut sched = super::Xc122Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_122_scheduler_reset() {
        let mut sched = super::Xc122Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_122_scheduler_add_remove() {
        let mut sched = super::Xc122Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_122_scheduler_targets() {
        let sched = super::Xc122Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_122_hash_empty() {
        assert_eq!(super::xc_122_hash(b""), 5381);
    }

    #[test]
    fn xc_122_hash_data() {
        let h = super::xc_122_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_122_hash(b"hello"), h);
    }

    #[test]
    fn xc_122_reverse_str() {
        assert_eq!(super::xc_122_reverse("abc"), "cba");
        assert_eq!(super::xc_122_reverse(""), "");
    }


    // --- xd_49 deepening tests ---

    #[test]
    fn xd_49_sm_initial_state() {
        let sm = Xd49StateMachine::new();
        assert_eq!(sm.current_state(), Xd49State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_49_sm_valid_idle_to_running() {
        let mut sm = Xd49StateMachine::new();
        assert!(sm.transition(Xd49State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd49State::Running);
    }

    #[test]
    fn xd_49_sm_valid_running_to_paused() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        assert!(sm.transition(Xd49State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd49State::Paused);
    }

    #[test]
    fn xd_49_sm_valid_running_to_done() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        assert!(sm.transition(Xd49State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd49State::Done);
    }

    #[test]
    fn xd_49_sm_valid_paused_to_running() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        sm.transition(Xd49State::Paused).unwrap();
        assert!(sm.transition(Xd49State::Running).is_ok());
    }

    #[test]
    fn xd_49_sm_valid_done_to_idle() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        sm.transition(Xd49State::Done).unwrap();
        assert!(sm.transition(Xd49State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd49State::Idle);
    }

    #[test]
    fn xd_49_sm_invalid_idle_to_done() {
        let mut sm = Xd49StateMachine::new();
        assert!(sm.transition(Xd49State::Done).is_err());
    }

    #[test]
    fn xd_49_sm_invalid_idle_to_paused() {
        let mut sm = Xd49StateMachine::new();
        assert!(sm.transition(Xd49State::Paused).is_err());
    }

    #[test]
    fn xd_49_sm_history_tracking() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        sm.transition(Xd49State::Paused).unwrap();
        sm.transition(Xd49State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd49State::Idle);
        assert_eq!(sm.history()[0].to, Xd49State::Running);
        assert_eq!(sm.history()[1].from, Xd49State::Running);
        assert_eq!(sm.history()[2].to, Xd49State::Done);
    }

    #[test]
    fn xd_49_sm_serialize_deserialize() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd49StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd49State::Running));
    }

    #[test]
    fn xd_49_sm_deserialize_invalid() {
        assert_eq!(Xd49StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_49_sm_reset() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd49State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_49_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd49EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd49Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_49_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd49EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd49Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd49Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_49_bus_unsubscribe() {
        let mut bus = Xd49EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_49_event_kind_and_payload() {
        let e = Xd49Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd49Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_49_bus_clear_history() {
        let mut bus = Xd49EventBus::new();
        bus.publish(Xd49Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_49_sm_step_counter_increments() {
        let mut sm = Xd49StateMachine::new();
        sm.transition(Xd49State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd49State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #47 --

    #[test]
    fn xf47_trie_insert_search() {
        let mut t = Xf47Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf47_trie_starts_with() {
        let mut t = Xf47Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf47_trie_remove() {
        let mut t = Xf47Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf47_trie_word_count() {
        let mut t = Xf47Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf47_trie_longest_prefix() {
        let mut t = Xf47Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf47_trie_all_words() {
        let mut t = Xf47Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf47_trie_autocomplete() {
        let mut t = Xf47Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf47_trie_empty_search() {
        let t = Xf47Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf47_bloom_add_contains() {
        let mut bf = Xf47BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf47_bloom_probably_absent() {
        let bf = Xf47BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf47_bloom_false_positive_rate() {
        let mut bf = Xf47BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf47_bloom_clear() {
        let mut bf = Xf47BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf47_bloom_union() {
        let mut a = Xf47BloomFilter::xf_new(512, 2);
        let mut b = Xf47BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf47_bloom_intersection_estimate() {
        let mut a = Xf47BloomFilter::xf_new(512, 2);
        let mut b = Xf47BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf47_bloom_union_size_mismatch() {
        let a = Xf47BloomFilter::xf_new(256, 2);
        let b = Xf47BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh121_skip_insert_contains() {
        let mut sl = super::Xh121SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh121_skip_remove() {
        let mut sl = super::Xh121SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh121_skip_len() {
        let mut sl = super::Xh121SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh121_skip_range_query() {
        let mut sl = super::Xh121SkipList::xh_new(4);
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
    fn xh121_skip_floor_ceiling() {
        let mut sl = super::Xh121SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh121_skip_rank() {
        let mut sl = super::Xh121SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh121_skip_empty() {
        let sl = super::Xh121SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh121_skip_duplicates() {
        let mut sl = super::Xh121SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh121_bitset_set_test() {
        let mut bs = super::Xh121BitSet::xh_new(256);
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
    fn xh121_bitset_clear_count() {
        let mut bs = super::Xh121BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh121_bitset_and_or_xor() {
        let mut a = super::Xh121BitSet::xh_new(128);
        let mut b = super::Xh121BitSet::xh_new(128);
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
    fn xh121_bitset_iter_ones() {
        let mut bs = super::Xh121BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh121_bitset_first_last() {
        let mut bs = super::Xh121BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh121_bitset_empty() {
        let bs = super::Xh121BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi121_deque_push_pop_back() {
        let mut dq = super::Xi121Deque::xi_new(4);
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
    fn xi121_deque_push_pop_front() {
        let mut dq = super::Xi121Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi121_deque_mixed_ops() {
        let mut dq = super::Xi121Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi121_deque_get_and_split() {
        let mut dq = super::Xi121Deque::xi_new(8);
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
    fn xi121_deque_rotate_left() {
        let mut dq = super::Xi121Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi121_deque_rotate_right() {
        let mut dq = super::Xi121Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi121_deque_grow() {
        let mut dq = super::Xi121Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi121_deque_empty() {
        let dq = super::Xi121Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi121_interval_tree_insert_query() {
        let mut tree = super::Xi121IntervalTree::xi_new();
        tree.xi_insert(super::Xi121Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi121Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi121Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi121_interval_tree_overlap() {
        let mut tree = super::Xi121IntervalTree::xi_new();
        tree.xi_insert(super::Xi121Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi121Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi121Interval::xi_new(12, 20));
        let q = super::Xi121Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi121_interval_tree_remove() {
        let mut tree = super::Xi121IntervalTree::xi_new();
        tree.xi_insert(super::Xi121Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi121Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi121_interval_tree_gaps() {
        let mut tree = super::Xi121IntervalTree::xi_new();
        tree.xi_insert(super::Xi121Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi121Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi121Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi121Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi121Interval::xi_new(8, 10));
    }

    #[test]
    fn xi121_interval_tree_merge() {
        let mut tree = super::Xi121IntervalTree::xi_new();
        tree.xi_insert(super::Xi121Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi121Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi121Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi121Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi121Interval::xi_new(10, 15));
    }

    #[test]
    fn xi121_interval_tree_all() {
        let mut tree = super::Xi121IntervalTree::xi_new();
        tree.xi_insert(super::Xi121Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi121Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi121_interval_tree_empty() {
        let tree = super::Xi121IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi121_interval_tree_contains_point() {
        let iv = super::Xi121Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 121) ---

    #[test]
    fn xj_121_uf_make_and_find() {
        let mut uf = super::Xj121UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_121_uf_union_connected() {
        let mut uf = super::Xj121UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_121_uf_component_count() {
        let mut uf = super::Xj121UnionFind::xj_new();
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
    fn xj_121_uf_component_size() {
        let mut uf = super::Xj121UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_121_uf_largest_component() {
        let mut uf = super::Xj121UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_121_uf_many_elements() {
        let mut uf = super::Xj121UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_121_uf_separate_components() {
        let mut uf = super::Xj121UnionFind::xj_new();
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
    fn xj_121_uf_path_compression() {
        let mut uf = super::Xj121UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_121_bt_insert_get() {
        let mut bt = super::Xj121BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_121_bt_contains_len() {
        let mut bt = super::Xj121BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_121_bt_replace() {
        let mut bt = super::Xj121BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_121_bt_remove() {
        let mut bt = super::Xj121BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_121_bt_keys_values() {
        let mut bt = super::Xj121BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_121_bt_range() {
        let mut bt = super::Xj121BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_121_bt_min_max() {
        let mut bt = super::Xj121BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_121_bt_many_inserts() {
        let mut bt = super::Xj121BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_121 segment tree tests ---

    #[test]
    fn xk_121_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk121SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_121_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk121SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_121_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk121SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_121_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk121SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_121_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk121SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_121_st_single_element() {
        let data = vec![42];
        let st = super::Xk121SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_121_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk121SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_121_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk121SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_121 disjoint intervals tests ---

    #[test]
    fn xk_121_di_add_and_count() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_121_di_merge_overlap() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_121_di_contains() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_121_di_remove() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_121_di_covered_length() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_121_di_gaps() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_121_di_merge_adjacent() {
        let mut di = super::Xk121DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_121_di_empty() {
        let di = super::Xk121DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_121_rope_new_empty() {
        let rope = super::Xl121Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_121_rope_from_str() {
        let rope = super::Xl121Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_121_rope_insert_at() {
        let mut rope = super::Xl121Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_121_rope_delete_range() {
        let mut rope = super::Xl121Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_121_rope_char_at() {
        let rope = super::Xl121Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_121_rope_split_concat() {
        let rope = super::Xl121Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_121_rope_line_count() {
        let rope = super::Xl121Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_121_rope_line_at() {
        let rope = super::Xl121Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_121_sa_build_and_search() {
        let sa = super::Xl121SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_121_sa_count() {
        let sa = super::Xl121SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_121_sa_longest_repeated() {
        let sa = super::Xl121SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_121_sa_all_positions() {
        let sa = super::Xl121SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_121_sa_len() {
        let sa = super::Xl121SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_121_sa_empty() {
        let sa = super::Xl121SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_121_rope_slice() {
        let rope = super::Xl121Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_121_sa_search_start() {
        let sa = super::Xl121SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_121_sparse_set_get() {
        let mut m = super::Xm121MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_121_sparse_row_col() {
        let mut m = super::Xm121MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_121_sparse_transpose() {
        let mut m = super::Xm121MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_121_sparse_multiply_vec() {
        let mut m = super::Xm121MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_121_sparse_nnz_density() {
        let mut m = super::Xm121MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_121_sparse_clear() {
        let mut m = super::Xm121MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_121_sparse_overwrite_zero() {
        let mut m = super::Xm121MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_121_tokenizer_basic() {
        let t = super::Xm121Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_121_tokenizer_count() {
        let t = super::Xm121Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_121_tokenizer_unique() {
        let t = super::Xm121Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_121_tokenizer_frequency() {
        let t = super::Xm121Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_121_tokenizer_delimiter() {
        let t = super::Xm121Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_121_tokenizer_whitespace() {
        let t = super::Xm121Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_121_tokenizer_empty() {
        let t = super::Xm121Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 121 ----

    #[test]
    fn xn_121_fenwick_prefix_sum() {
        let mut ft = super::Xn121Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_121_fenwick_range_sum() {
        let mut ft = super::Xn121Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_121_fenwick_point_query() {
        let mut ft = super::Xn121Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_121_fenwick_len() {
        let ft = super::Xn121Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_121_fenwick_multiple_updates() {
        let mut ft = super::Xn121Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_121_fenwick_single_element() {
        let mut ft = super::Xn121Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_121_fenwick_find_kth() {
        let mut ft = super::Xn121Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_121_fenwick_negative_delta() {
        let mut ft = super::Xn121Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 121 ----

    #[test]
    fn xn_121_avl_insert_get() {
        let mut m = super::Xn121AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_121_avl_remove() {
        let mut m = super::Xn121AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_121_avl_in_order() {
        let mut m = super::Xn121AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_121_avl_min_max() {
        let mut m = super::Xn121AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_121_avl_floor_ceiling() {
        let mut m = super::Xn121AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_121_avl_height_balanced() {
        let mut m = super::Xn121AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_121_avl_overwrite() {
        let mut m = super::Xn121AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_121_avl_empty() {
        let m: super::Xn121AVL<i32, i32> = super::Xn121AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo121RedBlack tests ---

    #[test]
    fn xo_121_rb_insert_and_get() {
        let mut tree = super::Xo121RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_121_rb_len_and_empty() {
        let mut tree = super::Xo121RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_121_rb_min_max() {
        let mut tree = super::Xo121RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_121_rb_contains() {
        let mut tree = super::Xo121RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_121_rb_remove() {
        let mut tree = super::Xo121RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_121_rb_in_order() {
        let mut tree = super::Xo121RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_121_rb_black_height() {
        let mut tree = super::Xo121RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_121_rb_overwrite() {
        let mut tree = super::Xo121RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo121ConsistentHash tests ---

    #[test]
    fn xo_121_ch_add_and_count() {
        let mut ring = super::Xo121ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_121_ch_remove_node() {
        let mut ring = super::Xo121ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_121_ch_get_node() {
        let mut ring = super::Xo121ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_121_ch_empty_ring() {
        let ring = super::Xo121ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_121_ch_distribution() {
        let mut ring = super::Xo121ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_121_ch_rebalance() {
        let mut ring = super::Xo121ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_121_ch_virtual_nodes() {
        let mut ring = super::Xo121ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_121_ch_consistent_lookup() {
        let mut ring = super::Xo121ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_121_splay_insert_get() {
        let mut t = super::Xp121SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_121_splay_remove() {
        let mut t = super::Xp121SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_121_splay_count_increases() {
        let mut t = super::Xp121SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_121_splay_depth() {
        let mut t = super::Xp121SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_121_splay_len_empty() {
        let t = super::Xp121SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_121_splay_min_max() {
        let mut t = super::Xp121SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_121_splay_overwrite() {
        let mut t = super::Xp121SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_121_splay_remove_missing() {
        let mut t = super::Xp121SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_121 treap tests ----
    #[test]
    fn xq_121_treap_empty() {
        let t = super::Xq121Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_121_treap_insert_get() {
        let mut t = super::Xq121Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_121_treap_overwrite() {
        let mut t = super::Xq121Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_121_treap_remove() {
        let mut t = super::Xq121Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_121_treap_min_max() {
        let mut t = super::Xq121Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_121_treap_rank() {
        let mut t = super::Xq121Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_121_treap_kth() {
        let mut t = super::Xq121Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_121_treap_in_order() {
        let mut t = super::Xq121Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_121 VEB tree tests ----
    #[test]
    fn xq_121_veb_empty() {
        let v = super::Xq121VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_121_veb_insert_contains() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_121_veb_min_max() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_121_veb_delete() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_121_veb_successor() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_121_veb_predecessor() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_121_veb_count() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_121_veb_duplicate_insert() {
        let mut v = super::Xq121VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_121_kdtree_empty() {
        let tree = super::Xr121KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_121_kdtree_insert_one() {
        let mut tree = super::Xr121KDTree::xr_new();
        tree.xr_insert(super::Xr121KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_121_kdtree_insert_multiple() {
        let mut tree = super::Xr121KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr121KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_121_kdtree_nearest_neighbor() {
        let mut tree = super::Xr121KDTree::xr_new();
        tree.xr_insert(super::Xr121KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr121KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr121KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_121_kdtree_nn_empty() {
        let tree = super::Xr121KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr121KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_121_kdtree_range_search() {
        let mut tree = super::Xr121KDTree::xr_new();
        tree.xr_insert(super::Xr121KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr121KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr121KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_121_kdtree_range_empty() {
        let mut tree = super::Xr121KDTree::xr_new();
        tree.xr_insert(super::Xr121KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_121_kdtree_all_points() {
        let mut tree = super::Xr121KDTree::xr_new();
        tree.xr_insert(super::Xr121KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr121KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_121_kdtree_depth() {
        let mut tree = super::Xr121KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr121KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_121_kdtree_bounding_box() {
        let mut tree = super::Xr121KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr121KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr121KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn xs_121_persistent_array_new() {
        let arr = super::Xs121PersistentArray::<i32>::xs_new();
        assert!(arr.xs_is_empty());
        assert_eq!(arr.xs_len(), 0);
        assert_eq!(arr.xs_version_count(), 1);
    }

    #[test]
    fn xs_121_persistent_array_push() {
        let mut arr = super::Xs121PersistentArray::<i32>::xs_new();
        let v1 = arr.xs_push(10);
        assert_eq!(v1, 1);
        assert_eq!(arr.xs_len(), 1);
        assert_eq!(arr.xs_get(0), Some(&10));
    }

    #[test]
    fn xs_121_persistent_array_set() {
        let mut arr = super::Xs121PersistentArray::xs_from_vec(vec![1, 2, 3]);
        let v = arr.xs_set(1, 20);
        assert!(v.is_some());
        assert_eq!(arr.xs_get(1), Some(&20));
        assert_eq!(arr.xs_get_version(0, 1), Some(&2));
    }

    #[test]
    fn xs_121_persistent_array_diff() {
        let mut arr = super::Xs121PersistentArray::xs_from_vec(vec![1, 2, 3]);
        arr.xs_set(0, 10);
        let diffs = arr.xs_diff(0, 1);
        assert_eq!(diffs, vec![0]);
    }

    #[test]
    fn xs_121_persistent_array_rollback() {
        let mut arr = super::Xs121PersistentArray::xs_from_vec(vec![1, 2]);
        arr.xs_push(3);
        arr.xs_rollback(0);
        assert_eq!(arr.xs_len(), 2);
        assert_eq!(arr.xs_as_slice(), &[1, 2]);
    }

    #[test]
    fn xs_121_persistent_array_history() {
        let mut arr = super::Xs121PersistentArray::xs_from_vec(vec![1]);
        arr.xs_push(2);
        let hist = arr.xs_history();
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], &[1]);
        assert_eq!(hist[1], &[1, 2]);
    }

    #[test]
    fn xs_121_persistent_array_set_out_of_bounds() {
        let mut arr = super::Xs121PersistentArray::xs_from_vec(vec![1]);
        assert!(arr.xs_set(5, 10).is_none());
    }

    #[test]
    fn xs_121_persistent_array_from_vec() {
        let arr = super::Xs121PersistentArray::xs_from_vec(vec![10, 20, 30]);
        assert_eq!(arr.xs_len(), 3);
        assert_eq!(arr.xs_get(2), Some(&30));
    }

    #[test]
    fn xs_121_concurrent_queue_new() {
        let q = super::Xs121ConcurrentQueue::<i32>::xs_new(10);
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_capacity(), 10);
    }

    #[test]
    fn xs_121_concurrent_queue_push_pop() {
        let mut q = super::Xs121ConcurrentQueue::xs_new(4);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert_eq!(q.xs_pop(), Some(1));
        assert_eq!(q.xs_pop(), Some(2));
        assert_eq!(q.xs_pop(), None);
    }

    #[test]
    fn xs_121_concurrent_queue_full() {
        let mut q = super::Xs121ConcurrentQueue::xs_new(2);
        assert!(q.xs_push(1));
        assert!(q.xs_push(2));
        assert!(!q.xs_push(3));
        assert!(q.xs_is_full());
    }

    #[test]
    fn xs_121_concurrent_queue_drain() {
        let mut q = super::Xs121ConcurrentQueue::xs_new(8);
        q.xs_push(10);
        q.xs_push(20);
        q.xs_push(30);
        let drained = q.xs_drain();
        assert_eq!(drained, vec![10, 20, 30]);
        assert!(q.xs_is_empty());
    }

    #[test]
    fn xs_121_concurrent_queue_try_pop() {
        let mut q = super::Xs121ConcurrentQueue::xs_new(4);
        assert_eq!(q.xs_try_pop(), None);
        q.xs_push(42);
        assert_eq!(q.xs_try_pop(), Some(42));
    }

    #[test]
    fn xs_121_concurrent_queue_clear() {
        let mut q = super::Xs121ConcurrentQueue::xs_new(4);
        q.xs_push(1);
        q.xs_push(2);
        q.xs_clear();
        assert!(q.xs_is_empty());
        assert_eq!(q.xs_len(), 0);
    }

    #[test]
    fn xs_121_range_map_new() {
        let rm = super::Xs121RangeMap::<String>::xs_new();
        assert!(rm.xs_is_empty());
        assert_eq!(rm.xs_len(), 0);
    }

    #[test]
    fn xs_121_range_map_insert_get() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        assert_eq!(rm.xs_get(5), Some(&"a"));
        assert_eq!(rm.xs_get(10), None);
    }

    #[test]
    fn xs_121_range_map_overlap() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_insert(5, 15, "b");
        assert_eq!(rm.xs_get(3), None);
        assert_eq!(rm.xs_get(7), Some(&"b"));
    }

    #[test]
    fn xs_121_range_map_remove() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        let removed = rm.xs_remove(5);
        assert_eq!(removed, Some("a"));
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_121_range_map_gaps() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(2, 5, "a");
        rm.xs_insert(8, 12, "b");
        let gaps = rm.xs_gaps(0, 15);
        assert_eq!(gaps, vec![(0, 2), (5, 8), (12, 15)]);
    }

    #[test]
    fn xs_121_range_map_coverage() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(0, 5, "a");
        rm.xs_insert(10, 20, "b");
        assert_eq!(rm.xs_total_coverage(), 15);
        assert_eq!(rm.xs_covered_ranges().len(), 2);
    }

    #[test]
    fn xs_121_range_map_contains() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(5, 10, 42);
        assert!(rm.xs_contains(7));
        assert!(!rm.xs_contains(4));
        assert!(!rm.xs_contains(10));
    }

    #[test]
    fn xs_121_range_map_clear() {
        let mut rm = super::Xs121RangeMap::xs_new();
        rm.xs_insert(0, 10, "a");
        rm.xs_clear();
        assert!(rm.xs_is_empty());
    }

    #[test]
    fn xs_121_circular_buffer_new() {
        let buf = super::Xs121CircularBuffer::<i32>::xs_new(5);
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_capacity(), 5);
    }

    #[test]
    fn xs_121_circular_buffer_push_pop() {
        let mut buf = super::Xs121CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert_eq!(buf.xs_pop_front(), Some(1));
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), None);
    }

    #[test]
    fn xs_121_circular_buffer_overwrite() {
        let mut buf = super::Xs121CircularBuffer::xs_new(2);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        assert_eq!(buf.xs_len(), 2);
        assert_eq!(buf.xs_pop_front(), Some(2));
        assert_eq!(buf.xs_pop_front(), Some(3));
    }

    #[test]
    fn xs_121_circular_buffer_peek() {
        let mut buf = super::Xs121CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        assert_eq!(buf.xs_peek_front(), Some(&10));
        assert_eq!(buf.xs_peek_back(), Some(&20));
    }

    #[test]
    fn xs_121_circular_buffer_is_full() {
        let mut buf = super::Xs121CircularBuffer::xs_new(2);
        assert!(!buf.xs_is_full());
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        assert!(buf.xs_is_full());
    }

    #[test]
    fn xs_121_circular_buffer_iter() {
        let mut buf = super::Xs121CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_push_back(3);
        let items: Vec<&i32> = buf.xs_iter();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn xs_121_circular_buffer_clear() {
        let mut buf = super::Xs121CircularBuffer::xs_new(4);
        buf.xs_push_back(1);
        buf.xs_push_back(2);
        buf.xs_clear();
        assert!(buf.xs_is_empty());
        assert_eq!(buf.xs_len(), 0);
    }

    #[test]
    fn xs_121_circular_buffer_to_vec() {
        let mut buf = super::Xs121CircularBuffer::xs_new(4);
        buf.xs_push_back(10);
        buf.xs_push_back(20);
        let v = buf.xs_to_vec();
        assert_eq!(v, vec![10, 20]);
    }


    // --- xt_ Fibonacci Heap tests ---

    #[test]
    fn xt_fib_heap_new() {
        let h = super::XtFibonacciHeap::<i32, &str>::xt_new();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_len(), 0);
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_insert_find_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(5, "five");
        h.xt_insert(3, "three");
        h.xt_insert(7, "seven");
        assert_eq!(h.xt_len(), 3);
        assert_eq!(h.xt_find_min(), Some((&3, &"three")));
    }

    #[test]
    fn xt_fib_heap_extract_min() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "ten");
        h.xt_insert(2, "two");
        h.xt_insert(8, "eight");
        h.xt_insert(1, "one");
        assert_eq!(h.xt_extract_min(), Some((1, "one")));
        assert_eq!(h.xt_extract_min(), Some((2, "two")));
        assert_eq!(h.xt_len(), 2);
    }

    #[test]
    fn xt_fib_heap_extract_all_sorted() {
        let mut h = super::XtFibonacciHeap::xt_new();
        for v in [5, 3, 8, 1, 9, 2, 7, 4, 6] {
            h.xt_insert(v, v * 10);
        }
        let sorted = h.xt_drain_sorted();
        let keys: Vec<i32> = sorted.iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn xt_fib_heap_decrease_key() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(10, "a");
        let idx = h.xt_insert(20, "b");
        h.xt_insert(15, "c");
        h.xt_decrease_key(idx, 5);
        assert_eq!(h.xt_find_min(), Some((&5, &"b")));
    }

    #[test]
    fn xt_fib_heap_merge() {
        let mut h1 = super::XtFibonacciHeap::xt_new();
        h1.xt_insert(3, "three");
        h1.xt_insert(7, "seven");
        let mut h2 = super::XtFibonacciHeap::xt_new();
        h2.xt_insert(1, "one");
        h2.xt_insert(5, "five");
        h1.xt_merge(&mut h2);
        assert_eq!(h1.xt_len(), 4);
        assert_eq!(h1.xt_find_min(), Some((&1, &"one")));
        assert!(h2.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_clear() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "a");
        h.xt_insert(2, "b");
        h.xt_clear();
        assert!(h.xt_is_empty());
        assert_eq!(h.xt_find_min(), None);
    }

    #[test]
    fn xt_fib_heap_single_element() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(42, "answer");
        assert_eq!(h.xt_extract_min(), Some((42, "answer")));
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_heap_display() {
        let mut h = super::XtFibonacciHeap::xt_new();
        h.xt_insert(1, "one");
        let s = format!("{}", h);
        assert!(s.contains("FibHeap"));
    }

    #[test]
    fn xt_fib_heap_default() {
        let h = super::XtFibonacciHeap::<i32, i32>::default();
        assert!(h.xt_is_empty());
    }

    #[test]
    fn xt_fib_node_display() {
        let n = super::XtFibNode::xt_new(10, "ten");
        let s = format!("{}", n);
        assert!(s.contains("FibNode"));
    }

    // --- xt_ Doubly-Linked List tests ---

    #[test]
    fn xt_dll_new() {
        let dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert!(dll.xt_is_empty());
        assert_eq!(dll.xt_len(), 0);
    }

    #[test]
    fn xt_dll_push_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_front(1);
        dll.xt_push_front(2);
        dll.xt_push_front(3);
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_push_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_pop_front() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_front(), Some(10));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_pop_back() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_pop_back(), Some(20));
        assert_eq!(dll.xt_len(), 1);
    }

    #[test]
    fn xt_dll_insert_after() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(3);
        dll.xt_insert_after(a, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_insert_before() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let b = dll.xt_push_back(3);
        dll.xt_insert_before(b, 2);
        assert_eq!(dll.xt_to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn xt_dll_remove_middle() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let mid = dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_remove(mid);
        assert_eq!(dll.xt_to_vec(), vec![1, 3]);
    }

    #[test]
    fn xt_dll_peek() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        assert_eq!(dll.xt_peek_front(), Some(&10));
        assert_eq!(dll.xt_peek_back(), Some(&20));
    }

    #[test]
    fn xt_dll_get() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let idx = dll.xt_push_back(42);
        assert_eq!(dll.xt_get(idx), Some(&42));
        assert_eq!(dll.xt_get(999), None);
    }

    #[test]
    fn xt_dll_iter_backward() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        let rev: Vec<&i32> = dll.xt_iter_backward();
        assert_eq!(rev, vec![&3, &2, &1]);
    }

    #[test]
    fn xt_dll_cursor_navigation() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(10);
        dll.xt_push_back(20);
        dll.xt_push_back(30);
        let c = dll.xt_head_cursor().unwrap();
        assert_eq!(dll.xt_get(c), Some(&10));
        let c2 = dll.xt_cursor_next(c).unwrap();
        assert_eq!(dll.xt_get(c2), Some(&20));
        let c3 = dll.xt_cursor_next(c2).unwrap();
        assert_eq!(dll.xt_get(c3), Some(&30));
        assert_eq!(dll.xt_cursor_next(c3), None);
        let c2b = dll.xt_cursor_prev(c3).unwrap();
        assert_eq!(dll.xt_get(c2b), Some(&20));
    }

    #[test]
    fn xt_dll_reverse() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_push_back(3);
        dll.xt_reverse();
        assert_eq!(dll.xt_to_vec(), vec![3, 2, 1]);
    }

    #[test]
    fn xt_dll_clear() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_clear();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_default() {
        let dll = super::XtDoublyLinkedList::<i32>::default();
        assert!(dll.xt_is_empty());
    }

    #[test]
    fn xt_dll_display() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        let s = format!("{}", dll);
        assert!(s.contains("DLL"));
    }

    #[test]
    fn xt_dll_reuse_freed_slots() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        let a = dll.xt_push_back(1);
        dll.xt_push_back(2);
        dll.xt_remove(a);
        let c = dll.xt_push_back(3);
        assert_eq!(c, a);
        assert_eq!(dll.xt_to_vec(), vec![2, 3]);
    }

    #[test]
    fn xt_dll_tail_cursor() {
        let mut dll = super::XtDoublyLinkedList::xt_new();
        dll.xt_push_back(1);
        dll.xt_push_back(2);
        let tc = dll.xt_tail_cursor().unwrap();
        assert_eq!(dll.xt_get(tc), Some(&2));
    }

    #[test]
    fn xt_dll_empty_operations() {
        let mut dll = super::XtDoublyLinkedList::<i32>::xt_new();
        assert_eq!(dll.xt_pop_front(), None);
        assert_eq!(dll.xt_pop_back(), None);
        assert_eq!(dll.xt_peek_front(), None);
        assert_eq!(dll.xt_peek_back(), None);
        assert_eq!(dll.xt_head_cursor(), None);
        assert_eq!(dll.xt_tail_cursor(), None);
    }

}
