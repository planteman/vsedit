//! Menu bar service.

use std::collections::HashMap;
use std::fmt;

/// Identifies a standard or custom menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuId {
    File,
    Edit,
    Selection,
    View,
    Go,
    Run,
    Terminal,
    Help,
    Custom(String),
}

impl MenuId {
    /// Returns the string key used for HashMap storage.
    pub fn key(&self) -> String {
        match self {
            MenuId::File => "file".to_string(),
            MenuId::Edit => "edit".to_string(),
            MenuId::Selection => "selection".to_string(),
            MenuId::View => "view".to_string(),
            MenuId::Go => "go".to_string(),
            MenuId::Run => "run".to_string(),
            MenuId::Terminal => "terminal".to_string(),
            MenuId::Help => "help".to_string(),
            MenuId::Custom(s) => s.clone(),
        }
    }

    /// Parses a string into a `MenuId`, matching known names case-insensitively
    /// and falling back to `Custom` for unrecognized values.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "file" => MenuId::File,
            "edit" => MenuId::Edit,
            "selection" => MenuId::Selection,
            "view" => MenuId::View,
            "go" => MenuId::Go,
            "run" => MenuId::Run,
            "terminal" => MenuId::Terminal,
            "help" => MenuId::Help,
            other => MenuId::Custom(other.to_string()),
        }
    }

    /// Returns true for built-in standard menus, false for `Custom`.
    pub fn is_standard(&self) -> bool {
        !matches!(self, MenuId::Custom(_))
    }

    /// Returns a human-readable label for display in the menu bar.
    pub fn label(&self) -> &str {
        match self {
            MenuId::File => "File",
            MenuId::Edit => "Edit",
            MenuId::Selection => "Selection",
            MenuId::View => "View",
            MenuId::Go => "Go",
            MenuId::Run => "Run",
            MenuId::Terminal => "Terminal",
            MenuId::Help => "Help",
            MenuId::Custom(s) => s.as_str(),
        }
    }
}

impl fmt::Display for MenuId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuId::Custom(s) => write!(f, "Custom({})", s),
            other => write!(f, "{}", other.key()),
        }
    }
}

/// Errors that can occur during menu bar operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuError {
    MenuNotFound(String),
    EntryNotFound(String),
    DuplicateEntry(String),
    InvalidShortcut(String),
}

impl fmt::Display for MenuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuError::MenuNotFound(id) => write!(f, "menu not found: {}", id),
            MenuError::EntryNotFound(id) => write!(f, "entry not found: {}", id),
            MenuError::DuplicateEntry(id) => write!(f, "duplicate entry: {}", id),
            MenuError::InvalidShortcut(s) => write!(f, "invalid shortcut: {}", s),
        }
    }
}

/// A single entry within a menu.
#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub command_id: String,
    pub title: String,
    pub group: Option<String>,
    pub order: i32,
    pub when: Option<String>,
    pub enabled: bool,
    pub shortcut: Option<String>,
}

impl MenuEntry {
    /// Returns whether this entry is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns whether this entry has a keyboard shortcut assigned.
    pub fn has_shortcut(&self) -> bool {
        self.shortcut.is_some()
    }

    /// Builder method to set the group.
    pub fn with_group(mut self, group: &str) -> Self {
        self.group = Some(group.to_string());
        self
    }

    /// Builder method to set the when clause.
    pub fn with_when(mut self, when: &str) -> Self {
        self.when = Some(when.to_string());
        self
    }

    /// Builder method to set the shortcut.
    pub fn with_shortcut(mut self, shortcut: &str) -> Self {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    /// Builder method to set the enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Returns a display string showing the title with an optional shortcut.
    /// e.g. "Save          Ctrl+S" or just "Save" if no shortcut is set.
    pub fn display_with_shortcut(&self) -> String {
        match &self.shortcut {
            Some(sc) => format!("{}    {}", self.title, sc),
            None => self.title.clone(),
        }
    }
}

impl fmt::Display for MenuEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.title, self.command_id)
    }
}

/// Service for managing menu bar menus and entries.
pub struct MenuBarService {
    menus: HashMap<String, Vec<MenuEntry>>,
}

impl MenuBarService {
    pub fn new() -> Self {
        Self {
            menus: HashMap::new(),
        }
    }

    pub fn add_entry(&mut self, menu_id: &MenuId, entry: MenuEntry) {
        self.menus.entry(menu_id.key()).or_default().push(entry);
    }

    pub fn remove_entry(&mut self, menu_id: &MenuId, command_id: &str) -> bool {
        if let Some(entries) = self.menus.get_mut(&menu_id.key()) {
            let len = entries.len();
            entries.retain(|e| e.command_id != command_id);
            entries.len() < len
        } else {
            false
        }
    }

    /// Returns entries for a menu sorted by order.
    pub fn get_entries(&self, menu_id: &MenuId) -> Vec<&MenuEntry> {
        let mut entries: Vec<&MenuEntry> = self
            .menus
            .get(&menu_id.key())
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        entries.sort_by_key(|e| e.order);
        entries
    }

    pub fn menu_count(&self) -> usize {
        self.menus.len()
    }

    pub fn entry_count(&self) -> usize {
        self.menus.values().map(|v| v.len()).sum()
    }

    /// Returns true if there are no menus or all menus are empty.
    pub fn is_empty(&self) -> bool {
        self.menus.values().all(|v| v.is_empty())
    }

    /// Returns the list of menu IDs (as keys) that have at least one entry.
    pub fn get_menu_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .menus
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, _)| k.clone())
            .collect();
        ids.sort();
        ids
    }

    /// Removes all entries from a menu. Returns an error if the menu does not exist.
    pub fn clear_menu(&mut self, menu_id: &MenuId) -> Result<(), MenuError> {
        match self.menus.get_mut(&menu_id.key()) {
            Some(entries) => {
                entries.clear();
                Ok(())
            }
            None => Err(MenuError::MenuNotFound(menu_id.key())),
        }
    }

    /// Finds an entry by command_id across all menus.
    /// Returns the menu key and a reference to the entry.
    pub fn find_entry(&self, command_id: &str) -> Option<(String, &MenuEntry)> {
        for (key, entries) in &self.menus {
            for entry in entries {
                if entry.command_id == command_id {
                    return Some((key.clone(), entry));
                }
            }
        }
        None
    }

    /// Returns entries within a menu filtered by group, sorted by order.
    pub fn get_entries_by_group(&self, menu_id: &MenuId, group: &str) -> Vec<&MenuEntry> {
        let mut entries: Vec<&MenuEntry> = self
            .menus
            .get(&menu_id.key())
            .map(|v| {
                v.iter()
                    .filter(|e| e.group.as_deref() == Some(group))
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by_key(|e| e.order);
        entries
    }

    /// Populates File, Edit, and View menus with common default entries.
    pub fn create_default_menus(&mut self) {
        let file_entries = vec![
            MenuEntry { command_id: "newFile".into(), title: "New File".into(), group: Some("1_new".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+N".into()) },
            MenuEntry { command_id: "openFile".into(), title: "Open File".into(), group: Some("1_new".into()), order: 2, when: None, enabled: true, shortcut: Some("Ctrl+O".into()) },
            MenuEntry { command_id: "save".into(), title: "Save".into(), group: Some("2_save".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+S".into()) },
            MenuEntry { command_id: "saveAs".into(), title: "Save As...".into(), group: Some("2_save".into()), order: 2, when: None, enabled: true, shortcut: Some("Ctrl+Shift+S".into()) },
            MenuEntry { command_id: "close".into(), title: "Close".into(), group: Some("3_close".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+W".into()) },
        ];
        for e in file_entries {
            self.add_entry(&MenuId::File, e);
        }

        let edit_entries = vec![
            MenuEntry { command_id: "undo".into(), title: "Undo".into(), group: Some("1_undo".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+Z".into()) },
            MenuEntry { command_id: "redo".into(), title: "Redo".into(), group: Some("1_undo".into()), order: 2, when: None, enabled: true, shortcut: Some("Ctrl+Y".into()) },
            MenuEntry { command_id: "cut".into(), title: "Cut".into(), group: Some("2_clipboard".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+X".into()) },
            MenuEntry { command_id: "copy".into(), title: "Copy".into(), group: Some("2_clipboard".into()), order: 2, when: None, enabled: true, shortcut: Some("Ctrl+C".into()) },
            MenuEntry { command_id: "paste".into(), title: "Paste".into(), group: Some("2_clipboard".into()), order: 3, when: None, enabled: true, shortcut: Some("Ctrl+V".into()) },
        ];
        for e in edit_entries {
            self.add_entry(&MenuId::Edit, e);
        }

        let view_entries = vec![
            MenuEntry { command_id: "toggleSidebar".into(), title: "Toggle Sidebar".into(), group: Some("1_layout".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+B".into()) },
            MenuEntry { command_id: "togglePanel".into(), title: "Toggle Panel".into(), group: Some("1_layout".into()), order: 2, when: None, enabled: true, shortcut: Some("Ctrl+J".into()) },
            MenuEntry { command_id: "zoomIn".into(), title: "Zoom In".into(), group: Some("2_zoom".into()), order: 1, when: None, enabled: true, shortcut: Some("Ctrl+=".into()) },
            MenuEntry { command_id: "zoomOut".into(), title: "Zoom Out".into(), group: Some("2_zoom".into()), order: 2, when: None, enabled: true, shortcut: Some("Ctrl+-".into()) },
        ];
        for e in view_entries {
            self.add_entry(&MenuId::View, e);
        }
    }

    /// Returns all entries across all menus as (menu_key, &MenuEntry) pairs.
    pub fn flatten_items(&self) -> Vec<(String, &MenuEntry)> {
        let mut items: Vec<(String, &MenuEntry)> = Vec::new();
        let mut keys: Vec<&String> = self.menus.keys().collect();
        keys.sort();
        for key in keys {
            if let Some(entries) = self.menus.get(key) {
                let mut sorted: Vec<&MenuEntry> = entries.iter().collect();
                sorted.sort_by_key(|e| e.order);
                for entry in sorted {
                    items.push((key.clone(), entry));
                }
            }
        }
        items
    }

    /// Finds an entry by command_id across all menus.
    /// Alias with a different return shape returning just the entry reference.
    pub fn find_item_by_command(&self, command_id: &str) -> Option<&MenuEntry> {
        self.find_entry(command_id).map(|(_, e)| e)
    }

    /// Returns the total count of entries across all menus.
    pub fn total_item_count(&self) -> usize {
        self.entry_count()
    }

    /// Returns all enabled entries across all menus as (menu_key, &MenuEntry) pairs.
    pub fn enabled_items(&self) -> Vec<(String, &MenuEntry)> {
        self.flatten_items()
            .into_iter()
            .filter(|(_, e)| e.enabled)
            .collect()
    }

    /// Toggles the enabled state of an entry identified by command_id.
    /// Returns `Ok(bool)` with the new enabled state, or an error if not found.
    pub fn toggle_item_enabled(&mut self, command_id: &str) -> Result<bool, MenuError> {
        for entries in self.menus.values_mut() {
            for entry in entries.iter_mut() {
                if entry.command_id == command_id {
                    entry.enabled = !entry.enabled;
                    return Ok(entry.enabled);
                }
            }
        }
        Err(MenuError::EntryNotFound(command_id.to_string()))
    }

    /// Returns the display path of an item, e.g. "File > Save As...".
    /// Uses the MenuId label for known menus.
    pub fn item_path(&self, command_id: &str) -> Option<String> {
        for (key, entries) in &self.menus {
            for entry in entries {
                if entry.command_id == command_id {
                    let menu_label = MenuId::from_str(key).label().to_string();
                    return Some(format!("{} > {}", menu_label, entry.title));
                }
            }
        }
        None
    }

    /// Creates a snapshot of the current menu bar state that can be restored later.
    pub fn snapshot(&self) -> MenuBarSnapshot {
        MenuBarSnapshot {
            menus: self.menus.clone(),
        }
    }

    /// Restores the menu bar state from a previously captured snapshot.
    pub fn restore(&mut self, snapshot: &MenuBarSnapshot) {
        self.menus = snapshot.menus.clone();
    }

    /// Returns the total number of entries across all menus.
    pub fn total_entry_count(&self) -> usize {
        self.menus.values().map(|v| v.len()).sum()
    }

    /// Finds entries across all menus where `command_id` starts with the given prefix.
    pub fn find_entries_by_prefix(&self, prefix: &str) -> Vec<&MenuEntry> {
        let mut result: Vec<&MenuEntry> = self
            .menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|e| e.command_id.starts_with(prefix))
            .collect();
        result.sort_by_key(|e| &e.command_id);
        result
    }

    /// Returns `(command_id, shortcut)` pairs for all entries that have shortcuts.
    pub fn get_all_shortcuts(&self) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = self
            .menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter_map(|e| {
                e.shortcut
                    .as_deref()
                    .map(|sc| (e.command_id.as_str(), sc))
            })
            .collect();
        pairs.sort_by_key(|(cmd, _)| *cmd);
        pairs
    }

    /// Returns the number of enabled entries across all menus.
    pub fn enabled_entry_count(&self) -> usize {
        self.menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|e| e.enabled)
            .count()
    }

    /// Disables an entry by `command_id`. Returns `true` if the entry was found and disabled.
    pub fn disable_entry(&mut self, command_id: &str) -> bool {
        for entries in self.menus.values_mut() {
            for entry in entries.iter_mut() {
                if entry.command_id == command_id {
                    entry.enabled = false;
                    return true;
                }
            }
        }
        false
    }
}

/// A captured snapshot of the menu bar state, used for save/restore operations.
#[derive(Debug, Clone)]
pub struct MenuBarSnapshot {
    menus: HashMap<String, Vec<MenuEntry>>,
}

impl MenuBarSnapshot {
    /// Returns the number of menus in this snapshot.
    pub fn menu_count(&self) -> usize {
        self.menus.len()
    }

    /// Returns the total entry count in this snapshot.
    pub fn entry_count(&self) -> usize {
        self.menus.values().map(|v| v.len()).sum()
    }

    /// Returns the menu keys present in this snapshot.
    pub fn menu_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.menus.keys().cloned().collect();
        keys.sort();
        keys
    }
}

impl Default for MenuBarService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Menu bar rendering state
// ---------------------------------------------------------------------------

/// The standard menu order for the top bar.
pub const MENU_ORDER: &[MenuId] = &[
    MenuId::File,
    MenuId::Edit,
    MenuId::Selection,
    MenuId::View,
    MenuId::Go,
    MenuId::Run,
    MenuId::Terminal,
    MenuId::Help,
];

/// State for rendering the menu bar and its dropdown overlays.
pub struct MenuBarState {
    /// Which menu title is focused (None = menu bar not active).
    pub active_menu: Option<usize>,
    /// Which item in the dropdown is highlighted.
    pub selected_item: usize,
    /// Whether the dropdown is open.
    pub dropdown_open: bool,
}

impl MenuBarState {
    pub fn new() -> Self {
        Self {
            active_menu: None,
            selected_item: 0,
            dropdown_open: false,
        }
    }

    /// Activate the menu bar (e.g. via Alt key), focusing the first menu.
    pub fn activate(&mut self) {
        self.active_menu = Some(0);
        self.selected_item = 0;
        self.dropdown_open = false;
    }

    /// Deactivate the menu bar entirely.
    pub fn deactivate(&mut self) {
        self.active_menu = None;
        self.selected_item = 0;
        self.dropdown_open = false;
    }

    /// Open the dropdown for the currently active menu.
    pub fn open_dropdown(&mut self) {
        self.dropdown_open = true;
        self.selected_item = 0;
    }

    /// Close the dropdown but keep the menu bar active.
    pub fn close_dropdown(&mut self) {
        self.dropdown_open = false;
        self.selected_item = 0;
    }

    /// Move to the next menu (right arrow).
    pub fn next_menu(&mut self, menu_count: usize) {
        if let Some(ref mut idx) = self.active_menu {
            if menu_count > 0 {
                *idx = (*idx + 1) % menu_count;
                self.selected_item = 0;
            }
        }
    }

    /// Move to the previous menu (left arrow).
    pub fn prev_menu(&mut self, menu_count: usize) {
        if let Some(ref mut idx) = self.active_menu {
            if menu_count > 0 {
                *idx = if *idx == 0 { menu_count - 1 } else { *idx - 1 };
                self.selected_item = 0;
            }
        }
    }

    /// Move selection down within the dropdown.
    pub fn next_item(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_item = (self.selected_item + 1) % item_count;
        }
    }

    /// Move selection up within the dropdown.
    pub fn prev_item(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_item = if self.selected_item == 0 {
                item_count - 1
            } else {
                self.selected_item - 1
            };
        }
    }

    /// Returns whether the menu bar is active (has focus).
    pub fn is_active(&self) -> bool {
        self.active_menu.is_some()
    }

    /// Activate a specific menu by index (e.g., Alt+F → index 0).
    pub fn activate_menu(&mut self, index: usize) {
        self.active_menu = Some(index);
        self.selected_item = 0;
        self.dropdown_open = true;
    }

    /// Try to activate a menu by Alt+letter. Returns true if matched.
    pub fn activate_by_letter(&mut self, letter: char) -> bool {
        let upper = letter.to_ascii_uppercase();
        for (i, menu_id) in MENU_ORDER.iter().enumerate() {
            if menu_id.label().starts_with(upper) {
                self.activate_menu(i);
                return true;
            }
        }
        false
    }
}

impl Default for MenuBarState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Menu bar render helpers
// ---------------------------------------------------------------------------

/// A pre-computed layout for a single menu title in the top bar.
#[derive(Debug, Clone)]
pub struct MenuTitleLayout {
    pub label: String,
    pub x: u16,
    pub width: u16,
}

/// Compute the layout of menu titles across the top bar.
pub fn compute_menu_title_layout(service: &MenuBarService) -> Vec<MenuTitleLayout> {
    let mut layouts = Vec::new();
    let mut x: u16 = 1; // 1 char left padding

    for menu_id in MENU_ORDER {
        let entries = service.get_entries(menu_id);
        if entries.is_empty() {
            continue;
        }
        let label = menu_id.label().to_string();
        let width = label.len() as u16 + 2; // 1 char padding each side
        layouts.push(MenuTitleLayout { label, x, width });
        x += width + 1; // 1 char gap between menus
    }

    layouts
}

/// A pre-computed layout for a dropdown menu overlay.
#[derive(Debug, Clone)]
pub struct DropdownLayout {
    /// The x position of the dropdown (aligned to menu title).
    pub x: u16,
    /// Width of the dropdown (max entry width + shortcut + padding).
    pub width: u16,
    /// Height of the dropdown (number of entries + borders).
    pub height: u16,
    /// The entries to render (title, shortcut, is_separator, is_enabled).
    pub items: Vec<DropdownItem>,
}

/// A single item in a dropdown menu.
#[derive(Debug, Clone)]
pub struct DropdownItem {
    pub title: String,
    pub shortcut: Option<String>,
    pub is_separator: bool,
    pub enabled: bool,
    pub command_id: String,
}

/// Compute the dropdown layout for a specific menu.
pub fn compute_dropdown_layout(
    service: &MenuBarService,
    menu_id: &MenuId,
    menu_x: u16,
) -> DropdownLayout {
    let entries = service.get_entries(menu_id);
    let mut items = Vec::new();
    let mut prev_group: Option<&str> = None;

    for entry in &entries {
        // Insert separator between groups
        if let Some(group) = &entry.group {
            if let Some(prev) = prev_group {
                if prev != group.as_str() {
                    items.push(DropdownItem {
                        title: String::new(),
                        shortcut: None,
                        is_separator: true,
                        enabled: false,
                        command_id: String::new(),
                    });
                }
            }
            prev_group = Some(group.as_str());
        }

        items.push(DropdownItem {
            title: entry.title.clone(),
            shortcut: entry.shortcut.clone(),
            is_separator: false,
            enabled: entry.enabled,
            command_id: entry.command_id.clone(),
        });
    }

    let max_title: u16 = items.iter()
        .filter(|i| !i.is_separator)
        .map(|i| i.title.len() as u16)
        .max()
        .unwrap_or(10);
    let max_shortcut: u16 = items.iter()
        .filter_map(|i| i.shortcut.as_ref())
        .map(|s| s.len() as u16)
        .max()
        .unwrap_or(0);
    let width = max_title + max_shortcut + 6; // padding + gap
    let height = items.len() as u16 + 2; // top + bottom border

    DropdownLayout {
        x: menu_x,
        width,
        height,
        items,
    }
}

// ── MenuAccelerator ──

/// Represents a keyboard accelerator (shortcut) for a menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuAccelerator {
    pub modifiers: Vec<String>,
    pub key: String,
}

impl MenuAccelerator {
    /// Parse a shortcut string like "Ctrl+Shift+S" into modifiers and key.
    pub fn parse(shortcut: &str) -> Result<Self, MenuError> {
        let parts: Vec<&str> = shortcut.split('+').collect();
        if parts.is_empty() || parts.last().map_or(true, |k| k.is_empty()) {
            return Err(MenuError::InvalidShortcut(shortcut.to_string()));
        }
        let key = parts.last().unwrap().to_string();
        let modifiers: Vec<String> = parts[..parts.len() - 1]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Ok(Self { modifiers, key })
    }

    /// Returns the canonical string form, e.g. "Ctrl+Shift+S".
    pub fn to_string_repr(&self) -> String {
        if self.modifiers.is_empty() {
            self.key.clone()
        } else {
            format!("{}+{}", self.modifiers.join("+"), self.key)
        }
    }

    /// Returns true if the accelerator uses the Ctrl modifier.
    pub fn has_ctrl(&self) -> bool {
        self.modifiers.iter().any(|m| m.eq_ignore_ascii_case("ctrl"))
    }

    /// Returns true if the accelerator uses the Shift modifier.
    pub fn has_shift(&self) -> bool {
        self.modifiers.iter().any(|m| m.eq_ignore_ascii_case("shift"))
    }

    /// Returns true if the accelerator uses the Alt modifier.
    pub fn has_alt(&self) -> bool {
        self.modifiers.iter().any(|m| m.eq_ignore_ascii_case("alt"))
    }

    /// Returns the number of modifiers.
    pub fn modifier_count(&self) -> usize {
        self.modifiers.len()
    }
}

impl fmt::Display for MenuAccelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string_repr())
    }
}

// ── MenuSearchIndex ──

/// Index for searching menu items by label substring.
pub struct MenuSearchIndex {
    entries: Vec<(String, String, String)>, // (menu_key, command_id, lowercase_title)
}

impl MenuSearchIndex {
    /// Build a search index from a `MenuBarService`.
    pub fn from_service(svc: &MenuBarService) -> Self {
        let mut entries = Vec::new();
        for (menu_key, entry) in svc.flatten_items() {
            entries.push((
                menu_key,
                entry.command_id.clone(),
                entry.title.to_lowercase(),
            ));
        }
        Self { entries }
    }

    /// Search for entries whose title contains `query` (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<(&str, &str)> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|(_, _, title)| title.contains(&q))
            .map(|(menu, cmd, _)| (menu.as_str(), cmd.as_str()))
            .collect()
    }

    /// Returns the total number of indexed entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── MenuBreadcrumb ──

/// Represents a breadcrumb trail for navigating nested menus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuBreadcrumb {
    segments: Vec<String>,
}

impl MenuBreadcrumb {
    /// Create an empty breadcrumb.
    pub fn new() -> Self {
        Self { segments: Vec::new() }
    }

    /// Push a new segment onto the trail.
    pub fn push(&mut self, segment: impl Into<String>) {
        self.segments.push(segment.into());
    }

    /// Pop the last segment from the trail.
    pub fn pop(&mut self) -> Option<String> {
        self.segments.pop()
    }

    /// Returns the full trail as "A > B > C".
    pub fn display(&self) -> String {
        self.segments.join(" > ")
    }

    /// Returns the current (last) segment, if any.
    pub fn current(&self) -> Option<&str> {
        self.segments.last().map(|s| s.as_str())
    }

    /// Returns the depth (number of segments).
    pub fn depth(&self) -> usize {
        self.segments.len()
    }

    /// Returns true if the breadcrumb is at the root (empty).
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Build a breadcrumb from a menu path string like "File > Save As...".
    pub fn from_path(path: &str) -> Self {
        let segments: Vec<String> = path
            .split('>')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Self { segments }
    }
}

impl Default for MenuBreadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MenuBreadcrumb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ── Recent items tracking on MenuBarService ──

impl MenuBarService {
    /// Collect all accelerators from entries that have shortcuts.
    pub fn collect_accelerators(&self) -> Vec<(String, MenuAccelerator)> {
        let mut result = Vec::new();
        for (_, entry) in self.flatten_items() {
            if let Some(sc) = &entry.shortcut {
                if let Ok(accel) = MenuAccelerator::parse(sc) {
                    result.push((entry.command_id.clone(), accel));
                }
            }
        }
        result
    }

    /// Build a search index from the current state of this service.
    pub fn build_search_index(&self) -> MenuSearchIndex {
        MenuSearchIndex::from_service(self)
    }

    /// Returns a breadcrumb for the given command_id, if found.
    pub fn breadcrumb_for(&self, command_id: &str) -> Option<MenuBreadcrumb> {
        self.item_path(command_id).map(|p| MenuBreadcrumb::from_path(&p))
    }
}

/// Summary statistics for the menu bar.
pub struct MenuBarStats {
    pub menu_count: usize,
    pub total_entries: usize,
    pub enabled_entries: usize,
    pub disabled_entries: usize,
    pub entries_with_shortcuts: usize,
}

impl MenuBarService {
    /// Compute summary statistics across all menus.
    pub fn stats(&self) -> MenuBarStats {
        let total = self.total_entry_count();
        let enabled = self.enabled_entry_count();
        let with_shortcuts = self
            .flatten_items()
            .iter()
            .filter(|(_, e)| e.has_shortcut())
            .count();
        MenuBarStats {
            menu_count: self.menu_count(),
            total_entries: total,
            enabled_entries: enabled,
            disabled_entries: total - enabled,
            entries_with_shortcuts: with_shortcuts,
        }
    }

    /// Return all command ids across every menu.
    pub fn all_command_ids(&self) -> Vec<String> {
        self.flatten_items()
            .iter()
            .map(|(_, e)| e.command_id.clone())
            .collect()
    }

    /// Return menus that have at least one entry.
    pub fn non_empty_menu_ids(&self) -> Vec<String> {
        self.menus
            .iter()
            .filter(|(_, entries)| !entries.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Return entries across all menus that have a `when` clause set.
    pub fn entries_with_when_clause(&self) -> Vec<&MenuEntry> {
        self.menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|e| e.when.is_some())
            .collect()
    }

    /// Count how many distinct groups exist across all menus.
    pub fn distinct_group_count(&self) -> usize {
        let mut groups = std::collections::HashSet::new();
        for entries in self.menus.values() {
            for e in entries {
                if let Some(ref g) = e.group {
                    groups.insert(g.clone());
                }
            }
        }
        groups.len()
    }

    /// Enable all entries across every menu. Returns how many were changed.
    pub fn enable_all_entries(&mut self) -> usize {
        let mut count = 0;
        for entries in self.menus.values_mut() {
            for e in entries.iter_mut() {
                if !e.enabled {
                    e.enabled = true;
                    count += 1;
                }
            }
        }
        count
    }

    /// Disable all entries across every menu. Returns how many were changed.
    pub fn disable_all_entries(&mut self) -> usize {
        let mut count = 0;
        for entries in self.menus.values_mut() {
            for e in entries.iter_mut() {
                if e.enabled {
                    e.enabled = false;
                    count += 1;
                }
            }
        }
        count
    }

    /// Return a sorted list of all unique shortcuts across all menus.
    pub fn all_shortcuts(&self) -> Vec<String> {
        let mut shortcuts: Vec<String> = self
            .menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter_map(|e| e.shortcut.clone())
            .collect();
        shortcuts.sort();
        shortcuts.dedup();
        shortcuts
    }

    /// Find entries whose title contains the given query (case-insensitive).
    pub fn search_entries(&self, query: &str) -> Vec<(String, &MenuEntry)> {
        let lower_q = query.to_lowercase();
        let mut results = Vec::new();
        for (key, entries) in &self.menus {
            for e in entries {
                if e.title.to_lowercase().contains(&lower_q) {
                    results.push((key.clone(), e));
                }
            }
        }
        results
    }

    /// Return the total number of disabled entries across all menus.
    pub fn disabled_entry_count(&self) -> usize {
        self.menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|e| !e.enabled)
            .count()
    }

    /// Return the total number of entries that have shortcuts.
    pub fn shortcut_count(&self) -> usize {
        self.menus
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|e| e.shortcut.is_some())
            .count()
    }
}

impl MenuEntry {
    /// Return the group name or "ungrouped" if none.
    pub fn group_label(&self) -> &str {
        self.group.as_deref().unwrap_or("ungrouped")
    }

    /// Return a summary string: "command_id (order)".
    pub fn summary(&self) -> String {
        format!("{} (order={})", self.command_id, self.order)
    }
}

impl MenuId {
    /// Return the standard menu ordering index for sorting.
    /// Custom menus sort after all standard menus.
    pub fn sort_order(&self) -> u8 {
        match self {
            MenuId::File => 0,
            MenuId::Edit => 1,
            MenuId::Selection => 2,
            MenuId::View => 3,
            MenuId::Go => 4,
            MenuId::Run => 5,
            MenuId::Terminal => 6,
            MenuId::Help => 7,
            MenuId::Custom(_) => 8,
        }
    }
}

// ---------------------------------------------------------------------------
// MenuBarSubmenuBuilder – nested menu construction
// ---------------------------------------------------------------------------

/// Builder for constructing nested submenu hierarchies.
#[derive(Debug, Clone)]
pub struct MenuBarSubmenuBuilder {
    label: String,
    entries: Vec<SubmenuItem>,
}

/// An item in a submenu – either a command or a nested submenu.
#[derive(Debug, Clone)]
pub enum SubmenuItem {
    Entry(MenuEntry),
    Separator,
    Submenu(MenuBarSubmenuBuilder),
}

impl MenuBarSubmenuBuilder {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            entries: Vec::new(),
        }
    }

    /// Add a command entry to the submenu.
    pub fn add_entry(mut self, entry: MenuEntry) -> Self {
        self.entries.push(SubmenuItem::Entry(entry));
        self
    }

    /// Add a separator.
    pub fn add_separator(mut self) -> Self {
        self.entries.push(SubmenuItem::Separator);
        self
    }

    /// Add a nested submenu.
    pub fn add_submenu(mut self, submenu: MenuBarSubmenuBuilder) -> Self {
        self.entries.push(SubmenuItem::Submenu(submenu));
        self
    }

    /// Total items including separators.
    pub fn item_count(&self) -> usize {
        self.entries.len()
    }

    /// Count of command entries (not separators or submenus).
    pub fn command_count(&self) -> usize {
        self.entries.iter().filter(|e| matches!(e, SubmenuItem::Entry(_))).count()
    }

    /// The submenu label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Depth of the deepest nested submenu.
    pub fn max_depth(&self) -> usize {
        let mut max = 0;
        for item in &self.entries {
            if let SubmenuItem::Submenu(sub) = item {
                let d = sub.max_depth() + 1;
                if d > max { max = d; }
            }
        }
        max
    }
}

impl fmt::Display for MenuBarSubmenuBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Submenu({}, {} items)", self.label, self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// MenuBarMnemonics – Alt+key shortcut navigation
// ---------------------------------------------------------------------------

/// Manages mnemonic (Alt+key) shortcuts for menu items.
#[derive(Debug, Clone)]
pub struct MenuBarMnemonics {
    /// Map from character to menu ID key.
    mappings: HashMap<char, String>,
}

impl MenuBarMnemonics {
    pub fn new() -> Self {
        Self { mappings: HashMap::new() }
    }

    /// Register a mnemonic for a menu.
    pub fn register(&mut self, key: char, menu_key: impl Into<String>) {
        self.mappings.insert(key.to_ascii_lowercase(), menu_key.into());
    }

    /// Look up which menu corresponds to a given key.
    pub fn lookup(&self, key: char) -> Option<&str> {
        self.mappings.get(&key.to_ascii_lowercase()).map(|s| s.as_str())
    }

    /// Auto-assign mnemonics from menu labels (first unused letter).
    pub fn auto_assign(&mut self, labels: &[(&str, &str)]) {
        for (menu_key, label) in labels {
            for ch in label.chars() {
                if ch.is_alphabetic() && !self.mappings.contains_key(&ch.to_ascii_lowercase()) {
                    self.register(ch, *menu_key);
                    break;
                }
            }
        }
    }

    /// Number of registered mnemonics.
    pub fn count(&self) -> usize {
        self.mappings.len()
    }

    /// All registered keys.
    pub fn keys(&self) -> Vec<char> {
        self.mappings.keys().copied().collect()
    }
}

impl Default for MenuBarMnemonics {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for MenuBarMnemonics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MenuBarMnemonics({} keys)", self.mappings.len())
    }
}

// ---------------------------------------------------------------------------
// Menu item separator logic
// ---------------------------------------------------------------------------

/// Remove duplicate or leading/trailing separators from a list of submenu items.
pub fn clean_separators(items: &[SubmenuItem]) -> Vec<SubmenuItem> {
    let mut result = Vec::new();
    let mut last_was_separator = true; // true to strip leading separators

    for item in items {
        match item {
            SubmenuItem::Separator => {
                if !last_was_separator {
                    result.push(SubmenuItem::Separator);
                    last_was_separator = true;
                }
            }
            other => {
                result.push(other.clone());
                last_was_separator = false;
            }
        }
    }

    // Remove trailing separator
    if let Some(SubmenuItem::Separator) = result.last() {
        result.pop();
    }
    result
}

// ---------------------------------------------------------------------------
// Menu bar overflow handling
// ---------------------------------------------------------------------------

/// Computes which menus overflow when the bar is too narrow.
#[derive(Debug, Clone)]
pub struct MenuBarOverflow {
    /// Maximum number of visible top-level menus.
    pub max_visible: usize,
}

impl MenuBarOverflow {
    pub fn new(max_visible: usize) -> Self {
        Self { max_visible }
    }

    /// Split menu IDs into visible and overflow groups.
    pub fn partition<'a>(&self, menu_ids: &'a [String]) -> (&'a [String], &'a [String]) {
        if menu_ids.len() <= self.max_visible {
            (menu_ids, &[])
        } else {
            menu_ids.split_at(self.max_visible)
        }
    }

    /// Whether there are overflowing menus.
    pub fn has_overflow(&self, total_menus: usize) -> bool {
        total_menus > self.max_visible
    }

    /// Number of overflowing menus.
    pub fn overflow_count(&self, total_menus: usize) -> usize {
        total_menus.saturating_sub(self.max_visible)
    }
}

impl fmt::Display for MenuBarOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MenuBarOverflow(max={})", self.max_visible)
    }
}


// ---------------------------------------------------------------------------
// MenuBarRecentlyUsed – tracks recently used menu commands
// ---------------------------------------------------------------------------

/// Tracks recently used commands in the menu bar for quick access / ordering.
#[derive(Debug, Clone)]
pub struct MenuBarRecentlyUsed {
    /// Ordered list of command IDs (most recent first).
    entries: Vec<String>,
    /// Maximum number of entries to keep.
    max_entries: usize,
}

impl MenuBarRecentlyUsed {
    /// Create a new tracker with the given capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Record that a command was used.  If the command is already in the list,
    /// it is moved to the front.
    pub fn record(&mut self, command_id: &str) {
        self.entries.retain(|e| e != command_id);
        self.entries.insert(0, command_id.to_string());
        if self.entries.len() > self.max_entries {
            self.entries.truncate(self.max_entries);
        }
    }

    /// Return the list of recently used command IDs (most recent first).
    pub fn list(&self) -> &[String] {
        &self.entries
    }

    /// Number of tracked entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the tracker is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Whether a given command was recently used.
    pub fn contains(&self, command_id: &str) -> bool {
        self.entries.iter().any(|e| e == command_id)
    }

    /// Return the rank (0-based) of a command, or `None` if not in the list.
    pub fn rank(&self, command_id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e == command_id)
    }

    /// Remove a specific command from the history.
    pub fn remove(&mut self, command_id: &str) {
        self.entries.retain(|e| e != command_id);
    }

    /// Return the most recently used command, if any.
    pub fn most_recent(&self) -> Option<&str> {
        self.entries.first().map(|s| s.as_str())
    }

    /// Maximum capacity of the tracker.
    pub fn capacity(&self) -> usize {
        self.max_entries
    }
}

// ---------------------------------------------------------------------------
// MenuBarCommandSearch – search across all menu entries
// ---------------------------------------------------------------------------

/// Searches across all registered menus for entries matching a query string.
#[derive(Debug, Clone)]
pub struct MenuBarCommandSearchResult {
    /// The menu this entry belongs to.
    pub menu_key: String,
    /// The matching entry.
    pub command_id: String,
    pub title: String,
    pub shortcut: Option<String>,
    /// How well the query matched (higher = better).
    pub score: i64,
}

/// Provides command-palette-style search over all menu entries.
pub struct MenuBarCommandSearch;

impl MenuBarCommandSearch {
    /// Search all menus in the service for entries matching `query`.
    /// Results are sorted by score (descending).
    pub fn search(service: &MenuBarService, query: &str) -> Vec<MenuBarCommandSearchResult> {
        let q_lower = query.to_lowercase();
        let mut results = Vec::new();

        for (menu_key, entry) in service.flatten_items() {
            let title_lower = entry.title.to_lowercase();
            let cmd_lower = entry.command_id.to_lowercase();

            let mut score: i64 = 0;
            let mut matched = false;

            // Exact prefix match on title
            if title_lower.starts_with(&q_lower) {
                score += 100;
                matched = true;
            } else if title_lower.contains(&q_lower) {
                score += 50;
                matched = true;
            }

            // Also check command ID
            if cmd_lower.starts_with(&q_lower) {
                score += 80;
                matched = true;
            } else if cmd_lower.contains(&q_lower) {
                score += 30;
                matched = true;
            }

            if matched {
                results.push(MenuBarCommandSearchResult {
                    menu_key,
                    command_id: entry.command_id.clone(),
                    title: entry.title.clone(),
                    shortcut: entry.shortcut.clone(),
                    score,
                });
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score).then(a.title.cmp(&b.title)));
        results
    }

    /// Like `search`, but limits results to a maximum count.
    pub fn search_top(service: &MenuBarService, query: &str, max: usize) -> Vec<MenuBarCommandSearchResult> {
        let mut results = Self::search(service, query);
        results.truncate(max);
        results
    }

    /// Search only within a specific menu.
    pub fn search_in_menu(service: &MenuBarService, menu_id: &MenuId, query: &str) -> Vec<MenuBarCommandSearchResult> {
        let key = menu_id.key();
        Self::search(service, query).into_iter()
            .filter(|r| r.menu_key == key)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// MenuBarSeparatorOptimizer – removes redundant separators
// ---------------------------------------------------------------------------

/// Represents a visual element in a rendered menu: either a concrete entry
/// or a group separator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuVisualItem {
    Entry(String),   // command_id
    Separator,
}

/// Cleans up a list of visual items by removing leading separators, trailing
/// separators, and consecutive duplicate separators.
pub struct MenuBarSeparatorOptimizer;

impl MenuBarSeparatorOptimizer {
    /// Optimize a list of visual items.
    pub fn optimize(items: &[MenuVisualItem]) -> Vec<MenuVisualItem> {
        let mut result: Vec<MenuVisualItem> = Vec::new();

        for item in items {
            match item {
                MenuVisualItem::Separator => {
                    // Only add if the last item is not already a separator
                    if let Some(last) = result.last() {
                        if *last != MenuVisualItem::Separator {
                            result.push(MenuVisualItem::Separator);
                        }
                    }
                    // Don't add separator at the beginning
                }
                MenuVisualItem::Entry(id) => {
                    result.push(MenuVisualItem::Entry(id.clone()));
                }
            }
        }

        // Remove trailing separator
        if let Some(MenuVisualItem::Separator) = result.last() {
            result.pop();
        }

        result
    }

    /// Build visual items from menu entries, inserting separators between groups.
    pub fn build_from_entries(entries: &[MenuEntry]) -> Vec<MenuVisualItem> {
        let mut items = Vec::new();
        let mut last_group: Option<&str> = None;

        let mut sorted: Vec<&MenuEntry> = entries.iter().collect();
        sorted.sort_by(|a, b| {
            let ga = a.group.as_deref().unwrap_or("");
            let gb = b.group.as_deref().unwrap_or("");
            ga.cmp(gb).then(a.order.cmp(&b.order))
        });

        for entry in &sorted {
            let group = entry.group.as_deref().unwrap_or("");
            if let Some(lg) = last_group {
                if lg != group {
                    items.push(MenuVisualItem::Separator);
                }
            }
            items.push(MenuVisualItem::Entry(entry.command_id.clone()));
            last_group = Some(group);
        }

        Self::optimize(&items)
    }

    /// Count the number of separators in an optimized list.
    pub fn separator_count(items: &[MenuVisualItem]) -> usize {
        items.iter().filter(|i| matches!(i, MenuVisualItem::Separator)).count()
    }

    /// Count the number of entries in a list.
    pub fn entry_count(items: &[MenuVisualItem]) -> usize {
        items.iter().filter(|i| matches!(i, MenuVisualItem::Entry(_))).count()
    }
}

// ---------------------------------------------------------------------------
// MenuBarRoleFilter – filter entries based on user roles
// ---------------------------------------------------------------------------

/// User role for role-based menu filtering.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UserRole {
    Admin,
    Developer,
    Viewer,
    Custom(String),
}

impl UserRole {
    pub fn label(&self) -> &str {
        match self {
            UserRole::Admin => "admin",
            UserRole::Developer => "developer",
            UserRole::Viewer => "viewer",
            UserRole::Custom(s) => s.as_str(),
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Filters menu entries based on user roles.  Each entry may have a `when`
/// clause containing "role:<name>".  If present, only users with that role
/// see the entry.
pub struct MenuBarRoleFilter;

impl MenuBarRoleFilter {
    /// Filter entries to only those accessible by the given roles.
    pub fn filter<'a>(entries: &'a [MenuEntry], roles: &[UserRole]) -> Vec<&'a MenuEntry> {
        entries.iter().filter(|e| Self::is_allowed(e, roles)).collect()
    }

    /// Whether a single entry is allowed for the given roles.
    pub fn is_allowed(entry: &MenuEntry, roles: &[UserRole]) -> bool {
        let when = match &entry.when {
            Some(w) => w,
            None => return true, // no constraint
        };
        if !when.contains("role:") {
            return true; // not a role constraint
        }
        for role in roles {
            let needle = format!("role:{}", role.label());
            if when.contains(&needle) {
                return true;
            }
        }
        false
    }

    /// Count how many entries are hidden for a given set of roles.
    pub fn hidden_count(entries: &[MenuEntry], roles: &[UserRole]) -> usize {
        entries.len() - Self::filter(entries, roles).len()
    }

    /// Return the command IDs of hidden entries.
    pub fn hidden_commands(entries: &[MenuEntry], roles: &[UserRole]) -> Vec<String> {
        entries.iter()
            .filter(|e| !Self::is_allowed(e, roles))
            .map(|e| e.command_id.clone())
            .collect()
    }
}


/// Workbench menubar configuration manager.
#[derive(Debug, Clone)]
pub struct WbMenubarConfig {
    entries: Vec<WbMenubarEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench menubar entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbMenubarEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbMenubarEntry {
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

impl WbMenubarConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbMenubarEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&WbMenubarEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbMenubarEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbMenubarEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&WbMenubarEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbMenubarEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<WbMenubarEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Menu bar structure and actions — extended utilities (qm)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_menu operations.
#[derive(Debug, Clone)]
pub struct QmMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QmMetrics {
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

/// Sliding-window rate counter for wb_menu.
#[derive(Debug, Clone)]
pub struct QmRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QmRateWindow {
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

/// A small LRU-style cache for wb_menu lookups.
#[derive(Debug, Clone)]
pub struct QmLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QmLruCache {
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
// xa_ extended helpers for wb_menubar
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaWbMenubarRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaWbMenubarRingBuf {
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
pub struct XaWbMenubarCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaWbMenubarCounter {
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

impl Default for XaWbMenubarCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 216
// ---------------------------------------------------------------------------

/// Generic object pool `Xc216Pool<T>`.
pub struct Xc216Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc216Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc216PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc216Pool<T> {
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
    pub fn stats(&self) -> Xc216PoolStats {
        Xc216PoolStats {
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

impl<T> Default for Xc216Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc216Scheduler`.
pub struct Xc216Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc216Scheduler {
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

impl Default for Xc216Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_216 hash for the given byte slice.
pub fn xc_216_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_216 convention.
pub fn xc_216_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe16 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe16Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe16PipelineError {
    pub stage: Xe16Stage,
    pub message: String,
}

impl std::fmt::Display for Xe16PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe16Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe16Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError>>>,
    stage_names: Vec<Xe16Stage>,
}

impl Xe16Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe16Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe16Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe16Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe16Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> {
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

    pub fn compose(mut self, other: Xe16Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe16CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe16CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe16Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe16CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe16CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe16Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe16CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_16_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe16CacheEntry {
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

    fn xe_16_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe16CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_16_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> {
    Ok(data)
}

pub fn xe_16_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_16_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_16_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_16_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe16PipelineError> {
    Err(Xe16PipelineError {
        stage: Xe16Stage::Parse,
        message: "intentional failure".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(cmd: &str, order: i32) -> MenuEntry {
        MenuEntry {
            command_id: cmd.to_string(),
            title: cmd.to_string(),
            group: None,
            order,
            when: None,
            enabled: true,
            shortcut: None,
        }
    }

    fn titled_entry(cmd: &str, title: &str, order: i32) -> MenuEntry {
        MenuEntry {
            command_id: cmd.to_string(),
            title: title.to_string(),
            group: None,
            order,
            when: None,
            enabled: true,
            shortcut: None,
        }
    }

    #[test]
    fn add_and_get_sorted() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("save", 2));
        svc.add_entry(&MenuId::File, entry("open", 1));
        let entries = svc.get_entries(&MenuId::File);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command_id, "open");
        assert_eq!(entries[1].command_id, "save");
    }

    #[test]
    fn remove_entry() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        assert!(svc.remove_entry(&MenuId::Edit, "undo"));
        assert!(!svc.remove_entry(&MenuId::Edit, "undo"));
    }

    #[test]
    fn counts() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        assert_eq!(svc.menu_count(), 2);
        assert_eq!(svc.entry_count(), 2);
    }

    #[test]
    fn is_empty_on_new_service() {
        let svc = MenuBarService::new();
        assert!(svc.is_empty());
    }

    #[test]
    fn is_empty_after_adding_entry() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        assert!(!svc.is_empty());
    }

    #[test]
    fn get_menu_ids() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let ids = svc.get_menu_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"file".to_string()));
        assert!(ids.contains(&"edit".to_string()));
    }

    #[test]
    fn clear_menu_success() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::File, entry("save", 2));
        assert!(svc.clear_menu(&MenuId::File).is_ok());
        assert_eq!(svc.get_entries(&MenuId::File).len(), 0);
    }

    #[test]
    fn clear_menu_not_found() {
        let mut svc = MenuBarService::new();
        let result = svc.clear_menu(&MenuId::Help);
        assert_eq!(result, Err(MenuError::MenuNotFound("help".to_string())));
    }

    #[test]
    fn find_entry_across_menus() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let (menu_key, found) = svc.find_entry("undo").unwrap();
        assert_eq!(menu_key, "edit");
        assert_eq!(found.command_id, "undo");
    }

    #[test]
    fn find_entry_missing() {
        let svc = MenuBarService::new();
        assert!(svc.find_entry("nonexistent").is_none());
    }

    #[test]
    fn get_entries_by_group() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1).with_group("io"));
        svc.add_entry(&MenuId::File, entry("save", 2).with_group("io"));
        svc.add_entry(&MenuId::File, entry("close", 3).with_group("lifecycle"));
        let io_entries = svc.get_entries_by_group(&MenuId::File, "io");
        assert_eq!(io_entries.len(), 2);
        assert_eq!(io_entries[0].command_id, "open");
        assert_eq!(io_entries[1].command_id, "save");
    }

    #[test]
    fn with_group_builder() {
        let e = entry("cmd", 1).with_group("nav");
        assert_eq!(e.group.as_deref(), Some("nav"));
    }

    #[test]
    fn with_when_builder() {
        let e = entry("cmd", 1).with_when("editorFocus");
        assert_eq!(e.when.as_deref(), Some("editorFocus"));
    }

    #[test]
    fn create_default_menus() {
        let mut svc = MenuBarService::new();
        svc.create_default_menus();
        assert_eq!(svc.menu_count(), 3);
        assert_eq!(svc.get_entries(&MenuId::File).len(), 5);
        assert_eq!(svc.get_entries(&MenuId::Edit).len(), 5);
        assert_eq!(svc.get_entries(&MenuId::View).len(), 4);
        assert_eq!(svc.entry_count(), 14);
    }

    #[test]
    fn menu_id_display() {
        assert_eq!(format!("{}", MenuId::File), "file");
        assert_eq!(format!("{}", MenuId::Custom("tools".into())), "Custom(tools)");
    }

    #[test]
    fn menu_entry_display() {
        let e = entry("save", 1);
        assert_eq!(format!("{}", e), "save (save)");
    }

    #[test]
    fn menu_error_display() {
        let err = MenuError::MenuNotFound("file".into());
        assert_eq!(format!("{}", err), "menu not found: file");
        let err = MenuError::EntryNotFound("cmd".into());
        assert_eq!(format!("{}", err), "entry not found: cmd");
        let err = MenuError::DuplicateEntry("cmd".into());
        assert_eq!(format!("{}", err), "duplicate entry: cmd");
    }

    // --- New tests ---

    #[test]
    fn menu_id_from_str_known() {
        assert_eq!(MenuId::from_str("file"), MenuId::File);
        assert_eq!(MenuId::from_str("FILE"), MenuId::File);
        assert_eq!(MenuId::from_str("Edit"), MenuId::Edit);
        assert_eq!(MenuId::from_str("selection"), MenuId::Selection);
        assert_eq!(MenuId::from_str("View"), MenuId::View);
        assert_eq!(MenuId::from_str("go"), MenuId::Go);
        assert_eq!(MenuId::from_str("RUN"), MenuId::Run);
        assert_eq!(MenuId::from_str("Terminal"), MenuId::Terminal);
        assert_eq!(MenuId::from_str("help"), MenuId::Help);
    }

    #[test]
    fn menu_id_from_str_custom() {
        assert_eq!(MenuId::from_str("tools"), MenuId::Custom("tools".to_string()));
        assert_eq!(MenuId::from_str("debug"), MenuId::Custom("debug".to_string()));
    }

    #[test]
    fn menu_id_label() {
        assert_eq!(MenuId::File.label(), "File");
        assert_eq!(MenuId::Edit.label(), "Edit");
        assert_eq!(MenuId::Custom("Tools".into()).label(), "Tools");
    }

    #[test]
    fn display_with_shortcut_present() {
        let e = entry("save", 1).with_shortcut("Ctrl+S");
        assert_eq!(e.display_with_shortcut(), "save    Ctrl+S");
    }

    #[test]
    fn display_with_shortcut_absent() {
        let e = entry("save", 1);
        assert_eq!(e.display_with_shortcut(), "save");
    }

    #[test]
    fn flatten_items_across_menus() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::File, entry("save", 2));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let flat = svc.flatten_items();
        assert_eq!(flat.len(), 3);
        // edit comes before file alphabetically
        assert_eq!(flat[0].0, "edit");
        assert_eq!(flat[0].1.command_id, "undo");
        assert_eq!(flat[1].0, "file");
        assert_eq!(flat[1].1.command_id, "open");
        assert_eq!(flat[2].0, "file");
        assert_eq!(flat[2].1.command_id, "save");
    }

    #[test]
    fn flatten_items_empty() {
        let svc = MenuBarService::new();
        assert!(svc.flatten_items().is_empty());
    }

    #[test]
    fn find_item_by_command_found() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let item = svc.find_item_by_command("undo").unwrap();
        assert_eq!(item.command_id, "undo");
    }

    #[test]
    fn find_item_by_command_missing() {
        let svc = MenuBarService::new();
        assert!(svc.find_item_by_command("nonexistent").is_none());
    }

    #[test]
    fn total_item_count() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        svc.add_entry(&MenuId::Edit, entry("redo", 2));
        assert_eq!(svc.total_item_count(), 3);
    }

    #[test]
    fn enabled_items_all_enabled() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::File, entry("save", 2));
        assert_eq!(svc.enabled_items().len(), 2);
    }

    #[test]
    fn enabled_items_with_disabled() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::File, entry("save", 2).with_enabled(false));
        let enabled = svc.enabled_items();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].1.command_id, "open");
    }

    #[test]
    fn toggle_item_enabled_success() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("save", 1));
        // starts enabled, toggle to disabled
        let new_state = svc.toggle_item_enabled("save").unwrap();
        assert!(!new_state);
        assert!(!svc.find_item_by_command("save").unwrap().enabled);
        // toggle back to enabled
        let new_state = svc.toggle_item_enabled("save").unwrap();
        assert!(new_state);
    }

    #[test]
    fn toggle_item_enabled_not_found() {
        let mut svc = MenuBarService::new();
        let result = svc.toggle_item_enabled("nonexistent");
        assert_eq!(result, Err(MenuError::EntryNotFound("nonexistent".into())));
    }

    #[test]
    fn item_path_found() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, titled_entry("saveAs", "Save As...", 2));
        assert_eq!(svc.item_path("saveAs").unwrap(), "File > Save As...");
    }

    #[test]
    fn item_path_custom_menu() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::Custom("tools".into()), titled_entry("lint", "Lint", 1));
        assert_eq!(svc.item_path("lint").unwrap(), "tools > Lint");
    }

    #[test]
    fn item_path_not_found() {
        let svc = MenuBarService::new();
        assert!(svc.item_path("nonexistent").is_none());
    }

    #[test]
    fn snapshot_and_restore() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let snap = svc.snapshot();
        assert_eq!(snap.menu_count(), 2);
        assert_eq!(snap.entry_count(), 2);

        // mutate
        svc.add_entry(&MenuId::File, entry("save", 2));
        assert_eq!(svc.entry_count(), 3);

        // restore
        svc.restore(&snap);
        assert_eq!(svc.entry_count(), 2);
        assert_eq!(svc.menu_count(), 2);
    }

    #[test]
    fn snapshot_menu_keys() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::View, entry("zoom", 1));
        let snap = svc.snapshot();
        let keys = snap.menu_keys();
        assert_eq!(keys, vec!["file".to_string(), "view".to_string()]);
    }

    #[test]
    fn with_shortcut_builder() {
        let e = entry("cmd", 1).with_shortcut("Ctrl+P");
        assert_eq!(e.shortcut.as_deref(), Some("Ctrl+P"));
    }

    #[test]
    fn with_enabled_builder() {
        let e = entry("cmd", 1).with_enabled(false);
        assert!(!e.enabled);
    }

    #[test]
    fn create_default_menus_has_shortcuts() {
        let mut svc = MenuBarService::new();
        svc.create_default_menus();
        let save = svc.find_item_by_command("save").unwrap();
        assert_eq!(save.shortcut.as_deref(), Some("Ctrl+S"));
        assert_eq!(save.display_with_shortcut(), "Save    Ctrl+S");
    }

    #[test]
    fn flatten_items_sorted_within_menu() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("z_last", 10));
        svc.add_entry(&MenuId::File, entry("a_first", 1));
        let flat = svc.flatten_items();
        assert_eq!(flat[0].1.command_id, "a_first");
        assert_eq!(flat[1].1.command_id, "z_last");
    }

    // --- MenuBarState tests ---

    #[test]
    fn menu_bar_state_new_inactive() {
        let state = MenuBarState::new();
        assert!(!state.is_active());
        assert!(state.active_menu.is_none());
        assert!(!state.dropdown_open);
    }

    #[test]
    fn menu_bar_state_activate() {
        let mut state = MenuBarState::new();
        state.activate();
        assert!(state.is_active());
        assert_eq!(state.active_menu, Some(0));
        assert!(!state.dropdown_open);
    }

    #[test]
    fn menu_bar_state_deactivate() {
        let mut state = MenuBarState::new();
        state.activate();
        state.deactivate();
        assert!(!state.is_active());
    }

    #[test]
    fn menu_bar_state_next_prev_menu() {
        let mut state = MenuBarState::new();
        state.activate();
        state.next_menu(4);
        assert_eq!(state.active_menu, Some(1));
        state.next_menu(4);
        assert_eq!(state.active_menu, Some(2));
        state.prev_menu(4);
        assert_eq!(state.active_menu, Some(1));
        // Wrap around
        state.active_menu = Some(3);
        state.next_menu(4);
        assert_eq!(state.active_menu, Some(0));
        state.prev_menu(4);
        assert_eq!(state.active_menu, Some(3));
    }

    #[test]
    fn menu_bar_state_next_prev_item() {
        let mut state = MenuBarState::new();
        state.activate();
        state.open_dropdown();
        state.next_item(5);
        assert_eq!(state.selected_item, 1);
        state.prev_item(5);
        assert_eq!(state.selected_item, 0);
        state.prev_item(5); // wrap
        assert_eq!(state.selected_item, 4);
    }

    #[test]
    fn menu_bar_state_activate_by_letter() {
        let mut state = MenuBarState::new();
        assert!(state.activate_by_letter('f'));
        assert_eq!(state.active_menu, Some(0)); // File
        assert!(state.dropdown_open);

        assert!(state.activate_by_letter('e'));
        assert_eq!(state.active_menu, Some(1)); // Edit

        assert!(state.activate_by_letter('h'));
        assert_eq!(state.active_menu, Some(7)); // Help

        assert!(!state.activate_by_letter('z')); // no match
    }

    #[test]
    fn menu_bar_state_open_close_dropdown() {
        let mut state = MenuBarState::new();
        state.activate();
        assert!(!state.dropdown_open);
        state.open_dropdown();
        assert!(state.dropdown_open);
        state.close_dropdown();
        assert!(!state.dropdown_open);
        assert!(state.is_active()); // still active
    }

    #[test]
    fn menu_bar_state_activate_menu_by_index() {
        let mut state = MenuBarState::new();
        state.activate_menu(3);
        assert_eq!(state.active_menu, Some(3));
        assert!(state.dropdown_open);
    }

    // --- Render helper tests ---

    #[test]
    fn compute_menu_title_layout_default_menus() {
        let mut svc = MenuBarService::new();
        svc.create_default_menus();
        let layouts = compute_menu_title_layout(&svc);
        // File, Edit, View are populated
        assert_eq!(layouts.len(), 3);
        assert_eq!(layouts[0].label, "File");
        assert!(layouts[0].x >= 1);
        assert!(layouts[0].width > 0);
    }

    #[test]
    fn compute_menu_title_layout_empty() {
        let svc = MenuBarService::new();
        let layouts = compute_menu_title_layout(&svc);
        assert!(layouts.is_empty());
    }

    #[test]
    fn compute_dropdown_layout_basic() {
        let mut svc = MenuBarService::new();
        svc.create_default_menus();
        let layout = compute_dropdown_layout(&svc, &MenuId::File, 1);
        assert!(!layout.items.is_empty());
        assert!(layout.width > 10);
        assert!(layout.height >= 3);
    }

    #[test]
    fn compute_dropdown_layout_has_separators() {
        let mut svc = MenuBarService::new();
        svc.create_default_menus();
        let layout = compute_dropdown_layout(&svc, &MenuId::File, 1);
        let sep_count = layout.items.iter().filter(|i| i.is_separator).count();
        // Separators inserted between group boundaries in sorted order
        assert!(sep_count > 0);
    }

    #[test]
    fn compute_dropdown_layout_items_have_shortcuts() {
        let mut svc = MenuBarService::new();
        svc.create_default_menus();
        let layout = compute_dropdown_layout(&svc, &MenuId::Edit, 1);
        let with_shortcut = layout.items.iter().filter(|i| i.shortcut.is_some()).count();
        assert!(with_shortcut > 0);
    }

    #[test]
    fn menu_order_has_eight_entries() {
        assert_eq!(MENU_ORDER.len(), 8);
        assert_eq!(MENU_ORDER[0], MenuId::File);
        assert_eq!(MENU_ORDER[7], MenuId::Help);
    }

    // --- New functionality tests ---

    #[test]
    fn menu_entry_is_enabled() {
        let e = entry("cmd", 1);
        assert!(e.is_enabled());
        let e2 = entry("cmd", 1).with_enabled(false);
        assert!(!e2.is_enabled());
    }

    #[test]
    fn menu_entry_has_shortcut() {
        let e = entry("cmd", 1);
        assert!(!e.has_shortcut());
        let e2 = entry("cmd", 1).with_shortcut("Ctrl+S");
        assert!(e2.has_shortcut());
    }

    #[test]
    fn menu_id_is_standard() {
        assert!(MenuId::File.is_standard());
        assert!(MenuId::Edit.is_standard());
        assert!(MenuId::Selection.is_standard());
        assert!(MenuId::View.is_standard());
        assert!(MenuId::Go.is_standard());
        assert!(MenuId::Run.is_standard());
        assert!(MenuId::Terminal.is_standard());
        assert!(MenuId::Help.is_standard());
        assert!(!MenuId::Custom("tools".into()).is_standard());
    }

    #[test]
    fn total_entry_count() {
        let mut svc = MenuBarService::new();
        assert_eq!(svc.total_entry_count(), 0);
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        svc.add_entry(&MenuId::Edit, entry("redo", 2));
        assert_eq!(svc.total_entry_count(), 3);
    }

    #[test]
    fn find_entries_by_prefix() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("file.open", 1));
        svc.add_entry(&MenuId::File, entry("file.save", 2));
        svc.add_entry(&MenuId::Edit, entry("edit.undo", 1));
        let found = svc.find_entries_by_prefix("file.");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].command_id, "file.open");
        assert_eq!(found[1].command_id, "file.save");
        assert!(svc.find_entries_by_prefix("nonexistent").is_empty());
    }

    #[test]
    fn get_all_shortcuts() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1).with_shortcut("Ctrl+O"));
        svc.add_entry(&MenuId::File, entry("save", 2).with_shortcut("Ctrl+S"));
        svc.add_entry(&MenuId::Edit, entry("undo", 1)); // no shortcut
        let shortcuts = svc.get_all_shortcuts();
        assert_eq!(shortcuts.len(), 2);
        assert_eq!(shortcuts[0], ("open", "Ctrl+O"));
        assert_eq!(shortcuts[1], ("save", "Ctrl+S"));
    }

    #[test]
    fn enabled_entry_count() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::File, entry("save", 2).with_enabled(false));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        assert_eq!(svc.enabled_entry_count(), 2);
    }

    #[test]
    fn disable_entry_found() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("save", 1));
        assert!(svc.disable_entry("save"));
        assert!(!svc.find_item_by_command("save").unwrap().enabled);
    }

    #[test]
    fn disable_entry_not_found() {
        let mut svc = MenuBarService::new();
        assert!(!svc.disable_entry("nonexistent"));
    }

    #[test]
    fn invalid_shortcut_error_display() {
        let err = MenuError::InvalidShortcut("bad+key".into());
        assert_eq!(format!("{}", err), "invalid shortcut: bad+key");
    }

    // ── New tests ──

    #[test]
    fn accelerator_parse_ctrl_s() {
        let accel = MenuAccelerator::parse("Ctrl+S").unwrap();
        assert_eq!(accel.modifiers, vec!["Ctrl"]);
        assert_eq!(accel.key, "S");
        assert!(accel.has_ctrl());
        assert!(!accel.has_shift());
        assert!(!accel.has_alt());
        assert_eq!(accel.to_string_repr(), "Ctrl+S");
    }

    #[test]
    fn accelerator_parse_multi_modifier() {
        let accel = MenuAccelerator::parse("Ctrl+Shift+S").unwrap();
        assert_eq!(accel.modifier_count(), 2);
        assert!(accel.has_ctrl());
        assert!(accel.has_shift());
        assert_eq!(format!("{}", accel), "Ctrl+Shift+S");
    }

    #[test]
    fn accelerator_parse_bare_key() {
        let accel = MenuAccelerator::parse("F5").unwrap();
        assert!(accel.modifiers.is_empty());
        assert_eq!(accel.key, "F5");
        assert_eq!(accel.modifier_count(), 0);
    }

    #[test]
    fn accelerator_parse_empty_fails() {
        assert!(MenuAccelerator::parse("").is_err());
    }

    #[test]
    fn search_index_finds_entries() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("openFile", 1));
        svc.add_entry(&MenuId::File, titled_entry("save", "Save File", 2));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let idx = svc.build_search_index();
        assert_eq!(idx.len(), 3);
        let results = idx.search("file");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_index_case_insensitive() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, titled_entry("zoomIn", "Zoom In", 1));
        let idx = svc.build_search_index();
        let results = idx.search("ZOOM");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, "zoomIn");
    }

    #[test]
    fn breadcrumb_from_path() {
        let bc = MenuBreadcrumb::from_path("File > Save As...");
        assert_eq!(bc.depth(), 2);
        assert_eq!(bc.current(), Some("Save As..."));
        assert_eq!(bc.display(), "File > Save As...");
    }

    #[test]
    fn breadcrumb_push_pop() {
        let mut bc = MenuBreadcrumb::new();
        assert!(bc.is_root());
        bc.push("File");
        bc.push("Recent");
        assert_eq!(bc.depth(), 2);
        assert_eq!(bc.pop(), Some("Recent".to_string()));
        assert_eq!(bc.depth(), 1);
        assert_eq!(bc.current(), Some("File"));
    }

    #[test]
    fn collect_accelerators_from_service() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1).with_shortcut("Ctrl+O"));
        svc.add_entry(&MenuId::File, entry("save", 2).with_shortcut("Ctrl+S"));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let accels = svc.collect_accelerators();
        assert_eq!(accels.len(), 2);
        assert!(accels.iter().all(|(_, a)| a.has_ctrl()));
    }

    #[test]
    fn breadcrumb_for_command() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, titled_entry("saveAs", "Save As...", 2));
        let bc = svc.breadcrumb_for("saveAs").unwrap();
        assert_eq!(bc.depth(), 2);
        assert_eq!(bc.current(), Some("Save As..."));
    }

    #[test]
    fn menu_bar_stats_empty() {
        let svc = MenuBarService::new();
        let s = svc.stats();
        assert_eq!(s.menu_count, 0);
        assert_eq!(s.total_entries, 0);
        assert_eq!(s.enabled_entries, 0);
        assert_eq!(s.disabled_entries, 0);
        assert_eq!(s.entries_with_shortcuts, 0);
    }

    #[test]
    fn menu_bar_stats_with_entries() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1).with_shortcut("Ctrl+O"));
        svc.add_entry(&MenuId::File, entry("close", 2));
        let s = svc.stats();
        assert_eq!(s.total_entries, 2);
        assert_eq!(s.entries_with_shortcuts, 1);
    }

    #[test]
    fn all_command_ids_lists_all() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        svc.add_entry(&MenuId::Edit, entry("undo", 1));
        let ids = svc.all_command_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"open".to_string()));
        assert!(ids.contains(&"undo".to_string()));
    }

    #[test]
    fn non_empty_menu_ids_filters() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("open", 1));
        let non_empty = svc.non_empty_menu_ids();
        assert_eq!(non_empty.len(), 1);
    }

    #[test]
    fn entries_with_when_clause_finds_matching() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1).with_when("editorFocus"));
        svc.add_entry(&MenuId::File, entry("b", 2));
        let with_when = svc.entries_with_when_clause();
        assert_eq!(with_when.len(), 1);
        assert_eq!(with_when[0].command_id, "a");
    }

    #[test]
    fn distinct_group_count_counts_unique() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1).with_group("io"));
        svc.add_entry(&MenuId::File, entry("b", 2).with_group("io"));
        svc.add_entry(&MenuId::Edit, entry("c", 1).with_group("clipboard"));
        assert_eq!(svc.distinct_group_count(), 2);
    }

    #[test]
    fn distinct_group_count_empty() {
        let svc = MenuBarService::new();
        assert_eq!(svc.distinct_group_count(), 0);
    }

    #[test]
    fn enable_all_entries_re_enables_disabled() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1).with_enabled(false));
        svc.add_entry(&MenuId::File, entry("b", 2));
        let changed = svc.enable_all_entries();
        assert_eq!(changed, 1);
        assert_eq!(svc.enabled_entry_count(), 2);
    }

    #[test]
    fn enable_all_entries_none_disabled() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1));
        let changed = svc.enable_all_entries();
        assert_eq!(changed, 0);
    }

    #[test]
    fn disable_all_entries() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1));
        svc.add_entry(&MenuId::Edit, entry("b", 2));
        let changed = svc.disable_all_entries();
        assert_eq!(changed, 2);
        assert_eq!(svc.disabled_entry_count(), 2);
    }

    #[test]
    fn all_shortcuts_deduped() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1).with_shortcut("Ctrl+S"));
        svc.add_entry(&MenuId::File, entry("b", 2).with_shortcut("Ctrl+O"));
        svc.add_entry(&MenuId::Edit, entry("c", 1).with_shortcut("Ctrl+S"));
        let shortcuts = svc.all_shortcuts();
        assert_eq!(shortcuts.len(), 2);
        assert!(shortcuts.contains(&"Ctrl+S".to_string()));
        assert!(shortcuts.contains(&"Ctrl+O".to_string()));
    }

    #[test]
    fn search_entries_case_insensitive() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, titled_entry("save", "Save File", 1));
        svc.add_entry(&MenuId::Edit, titled_entry("undo", "Undo Action", 1));
        let results = svc.search_entries("save");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.command_id, "save");
    }

    #[test]
    fn search_entries_no_match() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1));
        let results = svc.search_entries("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn shortcut_count() {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, entry("a", 1).with_shortcut("Ctrl+S"));
        svc.add_entry(&MenuId::File, entry("b", 2));
        assert_eq!(svc.shortcut_count(), 1);
    }

    #[test]
    fn entry_group_label() {
        let e = entry("a", 1).with_group("navigation");
        assert_eq!(e.group_label(), "navigation");
        let e2 = entry("b", 1);
        assert_eq!(e2.group_label(), "ungrouped");
    }

    #[test]
    fn entry_summary() {
        let e = entry("save", 3);
        let s = e.summary();
        assert!(s.contains("save"));
        assert!(s.contains("order=3"));
    }

    #[test]
    fn menu_id_sort_order() {
        assert!(MenuId::File.sort_order() < MenuId::Edit.sort_order());
        assert!(MenuId::Help.sort_order() < MenuId::Custom("x".into()).sort_order());
    }

    // -- MenuBarSubmenuBuilder ---------------------------------------------

    #[test]
    fn submenu_builder_basic() {
        let sub = MenuBarSubmenuBuilder::new("File")
            .add_entry(entry("open", 1))
            .add_separator()
            .add_entry(entry("save", 2));
        assert_eq!(sub.item_count(), 3);
        assert_eq!(sub.command_count(), 2);
        assert_eq!(sub.label(), "File");
    }

    #[test]
    fn submenu_builder_nested_depth() {
        let inner = MenuBarSubmenuBuilder::new("Inner");
        let outer = MenuBarSubmenuBuilder::new("Outer").add_submenu(inner);
        assert_eq!(outer.max_depth(), 1);
    }

    #[test]
    fn submenu_builder_display() {
        let sub = MenuBarSubmenuBuilder::new("Edit");
        assert!(format!("{sub}").contains("Edit"));
    }

    // -- MenuBarMnemonics --------------------------------------------------

    #[test]
    fn mnemonics_register_and_lookup() {
        let mut mn = MenuBarMnemonics::new();
        mn.register('f', "file");
        assert_eq!(mn.lookup('f'), Some("file"));
        assert_eq!(mn.lookup('F'), Some("file"));
        assert_eq!(mn.lookup('x'), None);
    }

    #[test]
    fn mnemonics_auto_assign() {
        let mut mn = MenuBarMnemonics::new();
        mn.auto_assign(&[("file", "File"), ("edit", "Edit")]);
        assert_eq!(mn.count(), 2);
        assert_eq!(mn.lookup('f'), Some("file"));
        assert_eq!(mn.lookup('e'), Some("edit"));
    }

    #[test]
    fn mnemonics_display() {
        let mn = MenuBarMnemonics::default();
        assert!(format!("{mn}").contains("0 keys"));
    }

    // -- clean_separators --------------------------------------------------

    #[test]
    fn clean_separators_removes_duplicates() {
        let items = vec![
            SubmenuItem::Entry(entry("a", 1)),
            SubmenuItem::Separator,
            SubmenuItem::Separator,
            SubmenuItem::Entry(entry("b", 2)),
        ];
        let cleaned = clean_separators(&items);
        assert_eq!(cleaned.len(), 3);
    }

    #[test]
    fn clean_separators_removes_leading_trailing() {
        let items = vec![
            SubmenuItem::Separator,
            SubmenuItem::Entry(entry("a", 1)),
            SubmenuItem::Separator,
        ];
        let cleaned = clean_separators(&items);
        assert_eq!(cleaned.len(), 1);
    }

    // -- MenuBarOverflow ---------------------------------------------------

    #[test]
    fn overflow_no_overflow() {
        let ov = MenuBarOverflow::new(8);
        let menus = vec!["file".to_string(), "edit".to_string()];
        let (visible, overflow) = ov.partition(&menus);
        assert_eq!(visible.len(), 2);
        assert!(overflow.is_empty());
        assert!(!ov.has_overflow(2));
    }

    #[test]
    fn overflow_with_overflow() {
        let ov = MenuBarOverflow::new(2);
        let menus = vec!["file".to_string(), "edit".to_string(), "view".to_string(), "help".to_string()];
        let (visible, overflow) = ov.partition(&menus);
        assert_eq!(visible.len(), 2);
        assert_eq!(overflow.len(), 2);
        assert_eq!(ov.overflow_count(4), 2);
    }

    #[test]
    fn overflow_display() {
        let ov = MenuBarOverflow::new(5);
        assert!(format!("{ov}").contains("max=5"));
    }

    // -- MenuBarRecentlyUsed tests --

    #[test]
    fn recently_used_empty() {
        let ru = MenuBarRecentlyUsed::new(5);
        assert!(ru.is_empty());
        assert_eq!(ru.len(), 0);
        assert_eq!(ru.most_recent(), None);
        assert_eq!(ru.capacity(), 5);
    }

    #[test]
    fn recently_used_record() {
        let mut ru = MenuBarRecentlyUsed::new(3);
        ru.record("file.save");
        ru.record("edit.copy");
        ru.record("edit.paste");

        assert_eq!(ru.len(), 3);
        assert_eq!(ru.most_recent(), Some("edit.paste"));
        assert_eq!(ru.rank("edit.paste"), Some(0));
        assert_eq!(ru.rank("edit.copy"), Some(1));
        assert_eq!(ru.rank("file.save"), Some(2));
    }

    #[test]
    fn recently_used_dedup_and_reorder() {
        let mut ru = MenuBarRecentlyUsed::new(5);
        ru.record("a");
        ru.record("b");
        ru.record("a"); // should move to front
        assert_eq!(ru.len(), 2);
        assert_eq!(ru.most_recent(), Some("a"));
        assert_eq!(ru.rank("b"), Some(1));
    }

    #[test]
    fn recently_used_max_capacity() {
        let mut ru = MenuBarRecentlyUsed::new(2);
        ru.record("a");
        ru.record("b");
        ru.record("c"); // pushes "a" out
        assert_eq!(ru.len(), 2);
        assert!(!ru.contains("a"));
        assert!(ru.contains("b"));
        assert!(ru.contains("c"));
    }

    #[test]
    fn recently_used_remove() {
        let mut ru = MenuBarRecentlyUsed::new(5);
        ru.record("a");
        ru.record("b");
        ru.remove("a");
        assert!(!ru.contains("a"));
        assert_eq!(ru.len(), 1);
    }

    #[test]
    fn recently_used_clear() {
        let mut ru = MenuBarRecentlyUsed::new(5);
        ru.record("x");
        ru.clear();
        assert!(ru.is_empty());
    }

    // -- MenuBarCommandSearch tests --

    fn build_search_service() -> MenuBarService {
        let mut svc = MenuBarService::new();
        svc.add_entry(&MenuId::File, MenuEntry {
            command_id: "file.save".to_string(),
            title: "Save".to_string(),
            group: None, order: 0, when: None, enabled: true,
            shortcut: Some("Ctrl+S".to_string()),
        });
        svc.add_entry(&MenuId::File, MenuEntry {
            command_id: "file.saveAs".to_string(),
            title: "Save As".to_string(),
            group: None, order: 1, when: None, enabled: true,
            shortcut: None,
        });
        svc.add_entry(&MenuId::Edit, MenuEntry {
            command_id: "edit.copy".to_string(),
            title: "Copy".to_string(),
            group: None, order: 0, when: None, enabled: true,
            shortcut: Some("Ctrl+C".to_string()),
        });
        svc
    }

    #[test]
    fn command_search_basic() {
        let svc = build_search_service();
        let results = MenuBarCommandSearch::search(&svc, "save");
        assert_eq!(results.len(), 2);
        // Higher score first: "Save" title starts with query
        assert!(results[0].score >= results[1].score);
    }

    #[test]
    fn command_search_no_results() {
        let svc = build_search_service();
        let results = MenuBarCommandSearch::search(&svc, "zzzzz");
        assert!(results.is_empty());
    }

    #[test]
    fn command_search_top() {
        let svc = build_search_service();
        let results = MenuBarCommandSearch::search_top(&svc, "save", 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn command_search_in_menu() {
        let svc = build_search_service();
        let results = MenuBarCommandSearch::search_in_menu(&svc, &MenuId::Edit, "copy");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command_id, "edit.copy");
    }

    // -- MenuBarSeparatorOptimizer tests --

    #[test]
    fn separator_optimize_removes_leading() {
        let items = vec![
            MenuVisualItem::Separator,
            MenuVisualItem::Entry("a".into()),
        ];
        let opt = MenuBarSeparatorOptimizer::optimize(&items);
        assert_eq!(opt, vec![MenuVisualItem::Entry("a".into())]);
    }

    #[test]
    fn separator_optimize_removes_trailing() {
        let items = vec![
            MenuVisualItem::Entry("a".into()),
            MenuVisualItem::Separator,
        ];
        let opt = MenuBarSeparatorOptimizer::optimize(&items);
        assert_eq!(opt, vec![MenuVisualItem::Entry("a".into())]);
    }

    #[test]
    fn separator_optimize_dedup_consecutive() {
        let items = vec![
            MenuVisualItem::Entry("a".into()),
            MenuVisualItem::Separator,
            MenuVisualItem::Separator,
            MenuVisualItem::Entry("b".into()),
        ];
        let opt = MenuBarSeparatorOptimizer::optimize(&items);
        assert_eq!(opt, vec![
            MenuVisualItem::Entry("a".into()),
            MenuVisualItem::Separator,
            MenuVisualItem::Entry("b".into()),
        ]);
    }

    #[test]
    fn separator_build_from_entries() {
        let entries = vec![
            MenuEntry {
                command_id: "a".into(), title: "A".into(),
                group: Some("1".into()), order: 0, when: None,
                enabled: true, shortcut: None,
            },
            MenuEntry {
                command_id: "b".into(), title: "B".into(),
                group: Some("2".into()), order: 0, when: None,
                enabled: true, shortcut: None,
            },
        ];
        let items = MenuBarSeparatorOptimizer::build_from_entries(&entries);
        assert_eq!(MenuBarSeparatorOptimizer::separator_count(&items), 1);
        assert_eq!(MenuBarSeparatorOptimizer::entry_count(&items), 2);
    }

    // -- MenuBarRoleFilter tests --

    #[test]
    fn role_filter_no_when() {
        let entry = MenuEntry {
            command_id: "open".into(), title: "Open".into(),
            group: None, order: 0, when: None,
            enabled: true, shortcut: None,
        };
        assert!(MenuBarRoleFilter::is_allowed(&entry, &[UserRole::Viewer]));
    }

    #[test]
    fn role_filter_role_match() {
        let entry = MenuEntry {
            command_id: "deploy".into(), title: "Deploy".into(),
            group: None, order: 0,
            when: Some("role:admin".into()),
            enabled: true, shortcut: None,
        };
        assert!(MenuBarRoleFilter::is_allowed(&entry, &[UserRole::Admin]));
        assert!(!MenuBarRoleFilter::is_allowed(&entry, &[UserRole::Viewer]));
    }

    #[test]
    fn role_filter_non_role_when() {
        let entry = MenuEntry {
            command_id: "x".into(), title: "X".into(),
            group: None, order: 0,
            when: Some("editorLangId == rust".into()),
            enabled: true, shortcut: None,
        };
        // Non-role when clauses always pass
        assert!(MenuBarRoleFilter::is_allowed(&entry, &[UserRole::Viewer]));
    }

    #[test]
    fn role_filter_hidden_count() {
        let entries = vec![
            MenuEntry {
                command_id: "a".into(), title: "A".into(),
                group: None, order: 0, when: Some("role:admin".into()),
                enabled: true, shortcut: None,
            },
            MenuEntry {
                command_id: "b".into(), title: "B".into(),
                group: None, order: 0, when: None,
                enabled: true, shortcut: None,
            },
        ];
        assert_eq!(MenuBarRoleFilter::hidden_count(&entries, &[UserRole::Viewer]), 1);
        assert_eq!(MenuBarRoleFilter::hidden_count(&entries, &[UserRole::Admin]), 0);
    }

    #[test]
    fn role_filter_hidden_commands() {
        let entries = vec![
            MenuEntry {
                command_id: "secret".into(), title: "Secret".into(),
                group: None, order: 0, when: Some("role:admin".into()),
                enabled: true, shortcut: None,
            },
        ];
        let hidden = MenuBarRoleFilter::hidden_commands(&entries, &[UserRole::Developer]);
        assert_eq!(hidden, vec!["secret".to_string()]);
    }

    #[test]
    fn user_role_display() {
        assert_eq!(format!("{}", UserRole::Admin), "admin");
        assert_eq!(format!("{}", UserRole::Developer), "developer");
        assert_eq!(format!("{}", UserRole::Custom("tester".into())), "tester");
    }


    #[test]
    fn wb_menubar_entry_creation() {
        let e = WbMenubarEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_menubar_entry_with_priority() {
        let e = WbMenubarEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_menubar_entry_metadata() {
        let e = WbMenubarEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_menubar_entry_remove_meta() {
        let mut e = WbMenubarEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_menubar_entry_activate_deactivate() {
        let mut e = WbMenubarEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_menubar_config_add_sorted() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("lo", "Lo").with_priority(1));
        c.add(WbMenubarEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_menubar_config_capacity() {
        let mut c = WbMenubarConfig::new(1);
        assert!(c.add(WbMenubarEntry::new("a", "A")));
        assert!(!c.add(WbMenubarEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_menubar_config_remove() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_menubar_config_get() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_menubar_config_active_entries() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A"));
        c.add(WbMenubarEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_menubar_config_enable_disable() {
        let mut c = WbMenubarConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_menubar_config_clear() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_menubar_config_find_by_label() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_menubar_config_top_n() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A").with_priority(1));
        c.add(WbMenubarEntry::new("b", "B").with_priority(2));
        c.add(WbMenubarEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_menubar_config_deactivate_activate_all() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A"));
        c.add(WbMenubarEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_menubar_config_highest_priority() {
        let mut c = WbMenubarConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbMenubarEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_menubar_config_contains() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_menubar_config_labels() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "Alpha"));
        c.add(WbMenubarEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_menubar_config_drain_inactive() {
        let mut c = WbMenubarConfig::new(10);
        c.add(WbMenubarEntry::new("a", "A"));
        c.add(WbMenubarEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn qm_metrics_empty() {
        let m = QmMetrics::new("wb_menu");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qm_metrics_record_and_mean() {
        let mut m = QmMetrics::new("wb_menu");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qm_metrics_min_max() {
        let mut m = QmMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qm_metrics_variance_and_std() {
        let mut m = QmMetrics::new("v");
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
    fn qm_metrics_percentile() {
        let mut m = QmMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qm_metrics_merge() {
        let mut a = QmMetrics::new("a");
        a.record(1.0);
        let mut b = QmMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qm_metrics_reset() {
        let mut m = QmMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qm_rate_window_empty() {
        let rw = QmRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qm_rate_window_tick_and_rate() {
        let mut rw = QmRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qm_lru_cache_basic() {
        let mut c = QmLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qm_lru_cache_contains_and_keys() {
        let mut c = QmLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qm_lru_cache_remove() {
        let mut c = QmLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qm_metrics_sum() {
        let mut m = QmMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qm_metrics_label() {
        let m = QmMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qm_lru_cache_clear() {
        let mut c = QmLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for wb_menubar
    #[test]
    fn xa_wb_menubar_ring_new() {
        let rb = super::XaWbMenubarRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_wb_menubar_ring_push_len() {
        let mut rb = super::XaWbMenubarRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_wb_menubar_ring_wrap() {
        let mut rb = super::XaWbMenubarRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_wb_menubar_ring_mean_empty() {
        let rb = super::XaWbMenubarRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_wb_menubar_ring_mean_values() {
        let mut rb = super::XaWbMenubarRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_wb_menubar_ring_min_max() {
        let mut rb = super::XaWbMenubarRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_wb_menubar_ring_iter() {
        let mut rb = super::XaWbMenubarRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_wb_menubar_counter_new() {
        let c = super::XaWbMenubarCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_menubar_counter_inc() {
        let mut c = super::XaWbMenubarCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_wb_menubar_counter_inc_by() {
        let mut c = super::XaWbMenubarCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_wb_menubar_counter_reset() {
        let mut c = super::XaWbMenubarCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_wb_menubar_counter_clear() {
        let mut c = super::XaWbMenubarCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_wb_menubar_counter_default() {
        let c = super::XaWbMenubarCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 216 ----

    #[test]
    fn xc_216_pool_new_empty() {
        let pool: super::Xc216Pool<i32> = super::Xc216Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_216_pool_release_acquire() {
        let mut pool = super::Xc216Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_216_pool_acquire_empty() {
        let mut pool: super::Xc216Pool<i32> = super::Xc216Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_216_pool_full() {
        let mut pool = super::Xc216Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_216_pool_drain() {
        let mut pool = super::Xc216Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_216_pool_stats() {
        let mut pool = super::Xc216Pool::new(8);
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
    fn xc_216_pool_clear() {
        let mut pool = super::Xc216Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_216_pool_shrink() {
        let mut pool = super::Xc216Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_216_pool_default() {
        let pool: super::Xc216Pool<String> = super::Xc216Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_216_pool_extend() {
        let mut pool = super::Xc216Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_216_pool_retain() {
        let mut pool = super::Xc216Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_216_scheduler_round_robin() {
        let mut sched = super::Xc216Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_216_scheduler_empty() {
        let mut sched = super::Xc216Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_216_scheduler_reset() {
        let mut sched = super::Xc216Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_216_scheduler_add_remove() {
        let mut sched = super::Xc216Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_216_scheduler_targets() {
        let sched = super::Xc216Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_216_hash_empty() {
        assert_eq!(super::xc_216_hash(b""), 5381);
    }

    #[test]
    fn xc_216_hash_data() {
        let h = super::xc_216_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_216_hash(b"hello"), h);
    }

    #[test]
    fn xc_216_reverse_str() {
        assert_eq!(super::xc_216_reverse("abc"), "cba");
        assert_eq!(super::xc_216_reverse(""), "");
    }


    #[test]
    fn xe_16_pipeline_empty() {
        let p = super::Xe16Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_16_pipeline_parse_stage() {
        let p = super::Xe16Pipeline::new()
            .add_parse(super::xe_16_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_16_pipeline_transform_double() {
        let p = super::Xe16Pipeline::new()
            .add_transform(super::xe_16_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_16_pipeline_validate_reverse() {
        let p = super::Xe16Pipeline::new()
            .add_validate(super::xe_16_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_16_pipeline_emit_filter() {
        let p = super::Xe16Pipeline::new()
            .add_emit(super::xe_16_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_16_pipeline_multi_stage() {
        let p = super::Xe16Pipeline::new()
            .add_parse(super::xe_16_pipeline_identity)
            .add_transform(super::xe_16_pipeline_double)
            .add_validate(super::xe_16_pipeline_reverse)
            .add_emit(super::xe_16_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_16_pipeline_error_propagation() {
        let p = super::Xe16Pipeline::new()
            .add_parse(super::xe_16_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe16Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_16_pipeline_compose() {
        let p1 = super::Xe16Pipeline::new()
            .add_parse(super::xe_16_pipeline_identity);
        let p2 = super::Xe16Pipeline::new()
            .add_transform(super::xe_16_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_16_pipeline_error_display() {
        let e = super::Xe16PipelineError {
            stage: super::Xe16Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_16_cache_put_get() {
        let mut c = super::Xe16Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_16_cache_miss() {
        let mut c: super::Xe16Cache<&str, i32> = super::Xe16Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_16_cache_ttl_expiry() {
        let mut c = super::Xe16Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_16_cache_evict() {
        let mut c = super::Xe16Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_16_cache_capacity() {
        let mut c = super::Xe16Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_16_cache_stats() {
        let mut c = super::Xe16Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_16_cache_clear() {
        let mut c = super::Xe16Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
