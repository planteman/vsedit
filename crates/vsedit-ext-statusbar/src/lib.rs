//! Ext API: Status bar.
//!
//! RPC bridge between the extension host and the main thread for status bar items.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_statusbar";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StatusBarMessage {
    CreateItem {
        id: String,
        alignment: StatusBarAlignment,
        priority: i32,
    },
    UpdateItem {
        id: String,
        text: Option<String>,
        tooltip: Option<String>,
        command: Option<String>,
    },
    ShowItem {
        id: String,
    },
    HideItem {
        id: String,
    },
    DisposeItem {
        id: String,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum StatusBarAlignment {
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub command: Option<String>,
    pub alignment: StatusBarAlignment,
    pub priority: i32,
    pub is_visible: bool,
}

// ── Bridge ──

pub struct StatusBarBridge {
    items: Vec<StatusBarItem>,
}

impl StatusBarBridge {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn create_item(&mut self, id: &str, alignment: StatusBarAlignment, priority: i32) {
        if !self.items.iter().any(|i| i.id == id) {
            self.items.push(StatusBarItem {
                id: id.to_string(),
                text: String::new(),
                tooltip: None,
                command: None,
                alignment,
                priority,
                is_visible: false,
            });
        }
    }

    pub fn get_item(&self, id: &str) -> Option<&StatusBarItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn show_item(&mut self, id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.is_visible = true;
        }
    }

    pub fn hide_item(&mut self, id: &str) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.is_visible = false;
        }
    }

    pub fn dispose_item(&mut self, id: &str) {
        self.items.retain(|i| i.id != id);
    }

    pub fn handle_message(&mut self, msg: &StatusBarMessage) -> serde_json::Value {
        match msg {
            StatusBarMessage::CreateItem {
                id,
                alignment,
                priority,
            } => {
                self.create_item(id, *alignment, *priority);
                serde_json::json!({"created": true})
            }
            StatusBarMessage::UpdateItem {
                id,
                text,
                tooltip,
                command,
            } => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == *id) {
                    if let Some(t) = text {
                        item.text = t.clone();
                    }
                    if tooltip.is_some() {
                        item.tooltip = tooltip.clone();
                    }
                    if command.is_some() {
                        item.command = command.clone();
                    }
                    serde_json::json!({"updated": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
            StatusBarMessage::ShowItem { id } => {
                self.show_item(id);
                serde_json::json!({"shown": true})
            }
            StatusBarMessage::HideItem { id } => {
                self.hide_item(id);
                serde_json::json!({"hidden": true})
            }
            StatusBarMessage::DisposeItem { id } => {
                self.dispose_item(id);
                serde_json::json!({"disposed": true})
            }
        }
    }
}

impl Default for StatusBarBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the statusbar extension API bridge.
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
        let msg = StatusBarMessage::CreateItem {
            id: "sb1".into(),
            alignment: StatusBarAlignment::Left,
            priority: 100,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: StatusBarMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn item_serialization() {
        let item = StatusBarItem {
            id: "sb1".into(),
            text: "$(sync~spin)".into(),
            tooltip: Some("Syncing".into()),
            command: Some("sync.run".into()),
            alignment: StatusBarAlignment::Right,
            priority: 50,
            is_visible: true,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: StatusBarItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_create_show_hide() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Left, 100);
        assert!(!bridge.get_item("sb1").unwrap().is_visible);
        bridge.show_item("sb1");
        assert!(bridge.get_item("sb1").unwrap().is_visible);
        bridge.hide_item("sb1");
        assert!(!bridge.get_item("sb1").unwrap().is_visible);
    }

    #[test]
    fn bridge_dispose() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Right, 0);
        bridge.dispose_item("sb1");
        assert!(bridge.get_item("sb1").is_none());
    }

    #[test]
    fn bridge_update_text() {
        let mut bridge = StatusBarBridge::new();
        bridge.create_item("sb1", StatusBarAlignment::Left, 0);
        let msg = StatusBarMessage::UpdateItem {
            id: "sb1".into(),
            text: Some("Ready".into()),
            tooltip: None,
            command: None,
        };
        bridge.handle_message(&msg);
        assert_eq!(bridge.get_item("sb1").unwrap().text, "Ready");
    }
}
