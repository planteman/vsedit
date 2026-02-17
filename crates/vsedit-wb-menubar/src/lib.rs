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
}
