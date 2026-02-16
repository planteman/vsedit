//! Ext API: Window.
//!
//! RPC bridge between the extension host and the main thread for window.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_window";

// ── RPC message types ──

/// Messages exchanged for the `window` API surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WindowMessage {
    ShowInformationMessage { message: String, items: Vec<String> },
    ShowWarningMessage { message: String, items: Vec<String> },
    ShowErrorMessage { message: String, items: Vec<String> },
    ShowQuickPick { items: Vec<QuickPickItem>, options: Option<QuickPickOptions> },
    ShowInputBox { options: InputBoxOptions },
    CreateStatusBarItem { id: String, alignment: StatusBarAlignment, priority: Option<i32> },
    ShowTextDocument { uri: String },
    CreateTerminal { name: String, shell_path: Option<String> },
    ShowOpenDialog { filters: Vec<DialogFilter>, can_select_many: bool },
    ShowSaveDialog { filters: Vec<DialogFilter>, default_uri: Option<String> },
    CreateOutputChannel { name: String },
    SetStatusBarMessage { text: String, timeout_ms: Option<u64> },
    CreateWebviewPanel { view_type: String, title: String },
}

/// An item in a quick pick list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickItem {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub picked: bool,
}

/// Options for a quick pick dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickPickOptions {
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub can_pick_many: bool,
}

/// Options for an input box dialog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputBoxOptions {
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub password: bool,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub validation_message: Option<String>,
}

/// Status bar item alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusBarAlignment {
    Left,
    Right,
}

/// File dialog filter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Response payload for window operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WindowResponse {
    MessageResult { selected: Option<String> },
    QuickPickResult { selected: Vec<QuickPickItem> },
    InputResult { value: Option<String> },
    StatusBarItemId { id: String },
    TerminalId { id: String },
    DialogResult { uris: Vec<String> },
    OutputChannelId { id: String },
    WebviewPanelId { id: String },
    Ok,
}

// ── Bridge ──

/// Processes window messages from the extension host.
#[derive(Debug, Default)]
pub struct WindowBridge {
    status_bar_items: Vec<String>,
    output_channels: Vec<String>,
    next_id: u64,
}

impl WindowBridge {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}-{}", self.next_id);
        self.next_id += 1;
        id
    }

    /// Process an incoming window message and return a response.
    pub fn handle(&mut self, msg: WindowMessage) -> WindowResponse {
        match msg {
            WindowMessage::ShowInformationMessage { .. }
            | WindowMessage::ShowWarningMessage { .. }
            | WindowMessage::ShowErrorMessage { .. } => {
                WindowResponse::MessageResult { selected: None }
            }
            WindowMessage::ShowQuickPick { .. } => {
                WindowResponse::QuickPickResult { selected: Vec::new() }
            }
            WindowMessage::ShowInputBox { .. } => {
                WindowResponse::InputResult { value: None }
            }
            WindowMessage::CreateStatusBarItem { id, .. } => {
                self.status_bar_items.push(id.clone());
                WindowResponse::StatusBarItemId { id }
            }
            WindowMessage::ShowTextDocument { .. } => WindowResponse::Ok,
            WindowMessage::CreateTerminal { .. } => {
                let id = self.next_id("terminal");
                WindowResponse::TerminalId { id }
            }
            WindowMessage::ShowOpenDialog { .. } | WindowMessage::ShowSaveDialog { .. } => {
                WindowResponse::DialogResult { uris: Vec::new() }
            }
            WindowMessage::CreateOutputChannel { name } => {
                self.output_channels.push(name);
                let id = self.next_id("output");
                WindowResponse::OutputChannelId { id }
            }
            WindowMessage::SetStatusBarMessage { .. } => WindowResponse::Ok,
            WindowMessage::CreateWebviewPanel { .. } => {
                let id = self.next_id("webview");
                WindowResponse::WebviewPanelId { id }
            }
        }
    }

    pub fn status_bar_count(&self) -> usize {
        self.status_bar_items.len()
    }
}

/// Initialize the window extension API bridge.
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
    fn show_info_message() {
        let mut bridge = WindowBridge::new();
        let resp = bridge.handle(WindowMessage::ShowInformationMessage {
            message: "Hello".into(),
            items: vec!["OK".into()],
        });
        assert_eq!(resp, WindowResponse::MessageResult { selected: None });
    }

    #[test]
    fn create_status_bar_item() {
        let mut bridge = WindowBridge::new();
        let resp = bridge.handle(WindowMessage::CreateStatusBarItem {
            id: "myext.status".into(),
            alignment: StatusBarAlignment::Left,
            priority: Some(100),
        });
        assert_eq!(
            resp,
            WindowResponse::StatusBarItemId { id: "myext.status".into() }
        );
        assert_eq!(bridge.status_bar_count(), 1);
    }

    #[test]
    fn create_output_channel() {
        let mut bridge = WindowBridge::new();
        let resp = bridge.handle(WindowMessage::CreateOutputChannel {
            name: "My Extension".into(),
        });
        matches!(resp, WindowResponse::OutputChannelId { .. });
    }

    #[test]
    fn quick_pick_options_serde() {
        let item = QuickPickItem {
            label: "Pick me".into(),
            description: Some("desc".into()),
            detail: None,
            picked: true,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: QuickPickItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, parsed);
    }

    #[test]
    fn serde_round_trip() {
        let msg = WindowMessage::ShowInputBox {
            options: InputBoxOptions {
                prompt: Some("Enter name".into()),
                placeholder: Some("name".into()),
                password: false,
                value: None,
                validation_message: None,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: WindowMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, parsed);
    }
}
