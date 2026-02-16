//! Menu bar service.

use std::collections::HashMap;

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

/// A single entry within a menu.
#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub command_id: String,
    pub title: String,
    pub group: Option<String>,
    pub order: i32,
    pub when: Option<String>,
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
}
