//! Ext API: Tree views.
//!
//! RPC bridge between the extension host and the main thread for the TreeView API.

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
}
