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
}

impl fmt::Display for MenuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuError::MenuNotFound(id) => write!(f, "menu not found: {}", id),
            MenuError::EntryNotFound(id) => write!(f, "entry not found: {}", id),
            MenuError::DuplicateEntry(id) => write!(f, "duplicate entry: {}", id),
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
}

impl MenuEntry {
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
            MenuEntry { command_id: "newFile".into(), title: "New File".into(), group: Some("1_new".into()), order: 1, when: None },
            MenuEntry { command_id: "openFile".into(), title: "Open File".into(), group: Some("1_new".into()), order: 2, when: None },
            MenuEntry { command_id: "save".into(), title: "Save".into(), group: Some("2_save".into()), order: 1, when: None },
            MenuEntry { command_id: "saveAs".into(), title: "Save As...".into(), group: Some("2_save".into()), order: 2, when: None },
            MenuEntry { command_id: "close".into(), title: "Close".into(), group: Some("3_close".into()), order: 1, when: None },
        ];
        for e in file_entries {
            self.add_entry(&MenuId::File, e);
        }

        let edit_entries = vec![
            MenuEntry { command_id: "undo".into(), title: "Undo".into(), group: Some("1_undo".into()), order: 1, when: None },
            MenuEntry { command_id: "redo".into(), title: "Redo".into(), group: Some("1_undo".into()), order: 2, when: None },
            MenuEntry { command_id: "cut".into(), title: "Cut".into(), group: Some("2_clipboard".into()), order: 1, when: None },
            MenuEntry { command_id: "copy".into(), title: "Copy".into(), group: Some("2_clipboard".into()), order: 2, when: None },
            MenuEntry { command_id: "paste".into(), title: "Paste".into(), group: Some("2_clipboard".into()), order: 3, when: None },
        ];
        for e in edit_entries {
            self.add_entry(&MenuId::Edit, e);
        }

        let view_entries = vec![
            MenuEntry { command_id: "toggleSidebar".into(), title: "Toggle Sidebar".into(), group: Some("1_layout".into()), order: 1, when: None },
            MenuEntry { command_id: "togglePanel".into(), title: "Toggle Panel".into(), group: Some("1_layout".into()), order: 2, when: None },
            MenuEntry { command_id: "zoomIn".into(), title: "Zoom In".into(), group: Some("2_zoom".into()), order: 1, when: None },
            MenuEntry { command_id: "zoomOut".into(), title: "Zoom Out".into(), group: Some("2_zoom".into()), order: 2, when: None },
        ];
        for e in view_entries {
            self.add_entry(&MenuId::View, e);
        }
    }
}

impl Default for MenuBarService {
    fn default() -> Self {
        Self::new()
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
}
