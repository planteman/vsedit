//! Ext API: Quick open.
//!
//! RPC bridge between the extension host and the main thread for QuickPick/InputBox.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_quickopen";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuickOpenMessage {
    ShowQuickPick {
        items: Vec<QuickPickItem>,
        options: QuickPickOptions,
    },
    ShowInputBox {
        options: InputBoxOptions,
    },
    Hide,
    SetItems {
        items: Vec<QuickPickItem>,
    },
    ItemSelected {
        index: usize,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub picked: bool,
    pub always_show: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickOptions {
    pub placeholder: Option<String>,
    pub can_pick_many: bool,
    pub match_on_description: bool,
    pub match_on_detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputBoxOptions {
    pub prompt: Option<String>,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub password: bool,
}

// ── Bridge ──

pub struct QuickOpenBridge {
    is_visible: bool,
    current_items: Vec<QuickPickItem>,
}

impl QuickOpenBridge {
    pub fn new() -> Self {
        Self {
            is_visible: false,
            current_items: Vec::new(),
        }
    }

    pub fn show(&mut self, items: Vec<QuickPickItem>) {
        self.current_items = items;
        self.is_visible = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
        self.current_items.clear();
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn select_item(&self, index: usize) -> Option<&QuickPickItem> {
        self.current_items.get(index)
    }

    pub fn handle_message(&mut self, msg: &QuickOpenMessage) -> serde_json::Value {
        match msg {
            QuickOpenMessage::ShowQuickPick { items, .. } => {
                self.show(items.clone());
                serde_json::json!({"shown": true, "count": items.len()})
            }
            QuickOpenMessage::ShowInputBox { options } => {
                self.is_visible = true;
                serde_json::json!({"shown": true, "prompt": options.prompt})
            }
            QuickOpenMessage::Hide => {
                self.hide();
                serde_json::json!({"hidden": true})
            }
            QuickOpenMessage::SetItems { items } => {
                self.current_items = items.clone();
                serde_json::json!({"updated": items.len()})
            }
            QuickOpenMessage::ItemSelected { index } => {
                let label = self.select_item(*index).map(|i| i.label.clone());
                serde_json::json!({"selected": label})
            }
        }
    }
}

impl Default for QuickOpenBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the quickopen extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_item(label: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.into(),
            description: None,
            detail: None,
            picked: false,
            always_show: false,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = QuickOpenMessage::ShowInputBox {
            options: InputBoxOptions {
                prompt: Some("Enter name".into()),
                placeholder: None,
                value: None,
                password: false,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: QuickOpenMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn quick_pick_item_serialization() {
        let item = test_item("Open File");
        let json = serde_json::to_string(&item).unwrap();
        let back: QuickPickItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_show_and_hide() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("a"), test_item("b")]);
        assert!(bridge.is_visible());
        bridge.hide();
        assert!(!bridge.is_visible());
    }

    #[test]
    fn bridge_select_item() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("first"), test_item("second")]);
        assert_eq!(bridge.select_item(0).unwrap().label, "first");
        assert_eq!(bridge.select_item(1).unwrap().label, "second");
        assert!(bridge.select_item(5).is_none());
    }

    #[test]
    fn bridge_handle_hide() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("a")]);
        bridge.handle_message(&QuickOpenMessage::Hide);
        assert!(!bridge.is_visible());
    }
}
