//! Menu bar and context menu system.
//!
//! Provides data structures for application menu bars and right-click context
//! menus, along with lookup and mutation helpers.

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
}
