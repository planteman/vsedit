//! Ext API: Tree views.
//!
//! RPC bridge between the extension host and the main thread for the TreeView API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_treeview";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TreeViewMessage {
    RegisterProvider {
        view_id: String,
    },
    UnregisterProvider {
        view_id: String,
    },
    GetChildren {
        view_id: String,
        element: Option<String>,
    },
    Reveal {
        view_id: String,
        element: String,
        select: bool,
        focus: bool,
    },
    SetMessage {
        view_id: String,
        message: Option<String>,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TreeItemCollapsibleState {
    None,
    Collapsed,
    Expanded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeItem {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub tooltip: Option<String>,
    pub icon_id: Option<String>,
    pub collapsible_state: TreeItemCollapsibleState,
    pub command: Option<String>,
    pub context_value: Option<String>,
    #[serde(default)]
    pub children: Vec<TreeItem>,
}

/// A trait representing tree data providers.
pub trait TreeDataProvider {
    fn get_children(&self, element: Option<&str>) -> Vec<TreeItem>;
    fn get_tree_item(&self, element: &str) -> Option<TreeItem>;
}

// ── Bridge ──

pub struct TreeViewBridge {
    views: Vec<String>,
}

impl TreeViewBridge {
    pub fn new() -> Self {
        Self { views: Vec::new() }
    }

    pub fn register_view(&mut self, view_id: &str) {
        if !self.views.contains(&view_id.to_string()) {
            self.views.push(view_id.to_string());
        }
    }

    pub fn unregister_view(&mut self, view_id: &str) {
        self.views.retain(|v| v != view_id);
    }

    pub fn has_view(&self, view_id: &str) -> bool {
        self.views.iter().any(|v| v == view_id)
    }

    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    pub fn handle_message(&mut self, msg: &TreeViewMessage) -> serde_json::Value {
        match msg {
            TreeViewMessage::RegisterProvider { view_id } => {
                self.register_view(view_id);
                serde_json::json!({"registered": true})
            }
            TreeViewMessage::UnregisterProvider { view_id } => {
                self.unregister_view(view_id);
                serde_json::json!({"unregistered": true})
            }
            TreeViewMessage::GetChildren { view_id, element } => {
                let found = self.has_view(view_id);
                serde_json::json!({"found": found, "element": element, "children": []})
            }
            TreeViewMessage::Reveal {
                view_id,
                element,
                select,
                focus,
            } => {
                serde_json::json!({"view": view_id, "element": element, "select": select, "focus": focus})
            }
            TreeViewMessage::SetMessage { view_id, message } => {
                serde_json::json!({"view": view_id, "message": message})
            }
        }
    }
}

impl Default for TreeViewBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Additional Types ──

/// Options for configuring a tree view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TreeViewOptions {
    pub can_select_many: bool,
    pub show_collapse_all: bool,
}

impl Default for TreeViewOptions {
    fn default() -> Self {
        Self {
            can_select_many: false,
            show_collapse_all: false,
        }
    }
}

/// Controller for drag-and-drop operations within a tree view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DragAndDropController {
    pub drag_mime_types: Vec<String>,
    pub drop_mime_types: Vec<String>,
}

impl DragAndDropController {
    pub fn new(drag_mime_types: Vec<String>, drop_mime_types: Vec<String>) -> Self {
        Self {
            drag_mime_types,
            drop_mime_types,
        }
    }
}

// ── Item Store ──

/// Per-view storage of tree items.
pub struct TreeItemStore {
    items: HashMap<String, Vec<TreeItem>>,
}

impl TreeItemStore {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    pub fn set_items(&mut self, view_id: &str, items: Vec<TreeItem>) {
        self.items.insert(view_id.to_string(), items);
    }

    pub fn get_items(&self, view_id: &str) -> &[TreeItem] {
        self.items.get(view_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn clear_items(&mut self, view_id: &str) {
        self.items.remove(view_id);
    }
}

impl Default for TreeItemStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tree Utilities ──

/// Recursively search for an item by id in a tree.
pub fn find_item<'a>(items: &'a [TreeItem], id: &str) -> Option<&'a TreeItem> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if let Some(found) = find_item(&item.children, id) {
            return Some(found);
        }
    }
    None
}

/// Recursively count all items (including nested children).
pub fn count_items(items: &[TreeItem]) -> usize {
    items
        .iter()
        .fold(0, |acc, item| acc + 1 + count_items(&item.children))
}

/// Flatten a tree into a depth-first ordered list of references.
pub fn flatten_tree<'a>(items: &'a [TreeItem]) -> Vec<&'a TreeItem> {
    let mut result = Vec::new();
    for item in items {
        result.push(item);
        result.extend(flatten_tree(&item.children));
    }
    result
}

/// Filter items by label using case-insensitive substring matching.
/// Parent items are preserved if any of their descendants match.
pub fn filter_items(items: &[TreeItem], query: &str) -> Vec<TreeItem> {
    let query_lower = query.to_lowercase();
    items
        .iter()
        .filter_map(|item| {
            let filtered_children = filter_items(&item.children, query);
            let label_matches = item.label.to_lowercase().contains(&query_lower);
            if label_matches || !filtered_children.is_empty() {
                let mut cloned = item.clone();
                cloned.children = filtered_children;
                Some(cloned)
            } else {
                None
            }
        })
        .collect()
}

/// Initialize the treeview extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = TreeViewMessage::GetChildren {
            view_id: "explorer".into(),
            element: Some("src".into()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: TreeViewMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn tree_item_serialization() {
        let item = TreeItem {
            id: "src".into(),
            label: "src".into(),
            description: Some("directory".into()),
            tooltip: None,
            icon_id: Some("folder".into()),
            collapsible_state: TreeItemCollapsibleState::Collapsed,
            command: None,
            context_value: Some("directory".into()),
            children: vec![],
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: TreeItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_register_and_unregister() {
        let mut bridge = TreeViewBridge::new();
        bridge.register_view("explorer");
        assert!(bridge.has_view("explorer"));
        bridge.unregister_view("explorer");
        assert!(!bridge.has_view("explorer"));
    }

    #[test]
    fn bridge_get_children_unknown() {
        let mut bridge = TreeViewBridge::new();
        let result = bridge.handle_message(&TreeViewMessage::GetChildren {
            view_id: "nope".into(),
            element: None,
        });
        assert_eq!(result["found"], false);
    }

    #[test]
    fn bridge_duplicate_register() {
        let mut bridge = TreeViewBridge::new();
        bridge.register_view("x");
        bridge.register_view("x");
        assert_eq!(bridge.views.len(), 1);
    }

    // ── Helper ──

    fn make_item(id: &str, label: &str, children: Vec<TreeItem>) -> TreeItem {
        TreeItem {
            id: id.into(),
            label: label.into(),
            description: None,
            tooltip: None,
            icon_id: None,
            collapsible_state: if children.is_empty() {
                TreeItemCollapsibleState::None
            } else {
                TreeItemCollapsibleState::Collapsed
            },
            command: None,
            context_value: None,
            children,
        }
    }

    fn sample_tree() -> Vec<TreeItem> {
        vec![
            make_item(
                "src",
                "src",
                vec![
                    make_item("main", "main.rs", vec![]),
                    make_item(
                        "lib",
                        "lib.rs",
                        vec![make_item("nested", "nested.rs", vec![])],
                    ),
                ],
            ),
            make_item("readme", "README.md", vec![]),
        ]
    }

    // ── New Tests ──

    #[test]
    fn bridge_view_count() {
        let mut bridge = TreeViewBridge::new();
        assert_eq!(bridge.view_count(), 0);
        bridge.register_view("a");
        bridge.register_view("b");
        assert_eq!(bridge.view_count(), 2);
        bridge.unregister_view("a");
        assert_eq!(bridge.view_count(), 1);
    }

    #[test]
    fn item_store_set_get_clear() {
        let mut store = TreeItemStore::new();
        assert!(store.get_items("v1").is_empty());
        store.set_items("v1", vec![make_item("a", "A", vec![])]);
        assert_eq!(store.get_items("v1").len(), 1);
        store.clear_items("v1");
        assert!(store.get_items("v1").is_empty());
    }

    #[test]
    fn item_store_multiple_views() {
        let mut store = TreeItemStore::new();
        store.set_items("v1", vec![make_item("a", "A", vec![])]);
        store.set_items("v2", vec![make_item("b", "B", vec![]), make_item("c", "C", vec![])]);
        assert_eq!(store.get_items("v1").len(), 1);
        assert_eq!(store.get_items("v2").len(), 2);
    }

    #[test]
    fn find_item_top_level() {
        let tree = sample_tree();
        let found = find_item(&tree, "readme");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "README.md");
    }

    #[test]
    fn find_item_nested() {
        let tree = sample_tree();
        let found = find_item(&tree, "nested");
        assert!(found.is_some());
        assert_eq!(found.unwrap().label, "nested.rs");
    }

    #[test]
    fn find_item_missing() {
        let tree = sample_tree();
        assert!(find_item(&tree, "nonexistent").is_none());
    }

    #[test]
    fn count_items_recursive() {
        let tree = sample_tree();
        // src(1) + main(1) + lib(1) + nested(1) + readme(1) = 5
        assert_eq!(count_items(&tree), 5);
    }

    #[test]
    fn flatten_tree_order() {
        let tree = sample_tree();
        let flat = flatten_tree(&tree);
        let ids: Vec<&str> = flat.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["src", "main", "lib", "nested", "readme"]);
    }

    #[test]
    fn filter_items_by_label() {
        let tree = sample_tree();
        let filtered = filter_items(&tree, ".rs");
        // "src" is preserved because it has matching descendants
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "src");
        assert_eq!(filtered[0].children.len(), 2);
    }

    #[test]
    fn filter_items_case_insensitive() {
        let tree = sample_tree();
        let filtered = filter_items(&tree, "README");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "readme");
    }

    #[test]
    fn filter_items_no_match() {
        let tree = sample_tree();
        let filtered = filter_items(&tree, "zzz_nothing");
        assert!(filtered.is_empty());
    }

    #[test]
    fn tree_view_options_defaults() {
        let opts = TreeViewOptions::default();
        assert!(!opts.can_select_many);
        assert!(!opts.show_collapse_all);
    }

    #[test]
    fn drag_and_drop_controller_new() {
        let ctrl = DragAndDropController::new(
            vec!["text/plain".into()],
            vec!["text/uri-list".into()],
        );
        assert_eq!(ctrl.drag_mime_types, vec!["text/plain"]);
        assert_eq!(ctrl.drop_mime_types, vec!["text/uri-list"]);
    }
}
