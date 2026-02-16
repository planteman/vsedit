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

// ── Error types ──

/// Errors that can occur during window operations.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowError {
    /// A required field was empty or missing.
    EmptyField(&'static str),
    /// A filter contained no extensions.
    EmptyFilterExtensions(String),
    /// The status bar item ID was not found.
    StatusBarItemNotFound(String),
    /// The output channel name was not found.
    OutputChannelNotFound(String),
    /// Priority value is out of the allowed range.
    PriorityOutOfRange(i32),
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "required field is empty: {field}"),
            Self::EmptyFilterExtensions(name) => {
                write!(f, "dialog filter '{name}' has no extensions")
            }
            Self::StatusBarItemNotFound(id) => write!(f, "status bar item not found: {id}"),
            Self::OutputChannelNotFound(name) => write!(f, "output channel not found: {name}"),
            Self::PriorityOutOfRange(p) => {
                write!(f, "priority {p} out of allowed range [-1000, 1000]")
            }
        }
    }
}

impl std::error::Error for WindowError {}

// ── Display impls ──

impl std::fmt::Display for StatusBarAlignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Left => write!(f, "Left"),
            Self::Right => write!(f, "Right"),
        }
    }
}

impl std::fmt::Display for QuickPickItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)?;
        if let Some(ref desc) = self.description {
            write!(f, " — {desc}")?;
        }
        Ok(())
    }
}

impl std::fmt::Display for DialogFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.extensions.join(", "))
    }
}

// ── Builders ──

/// Builder for creating [`QuickPickItem`] values.
#[derive(Debug, Clone, Default)]
pub struct QuickPickItemBuilder {
    label: String,
    description: Option<String>,
    detail: Option<String>,
    picked: bool,
}

impl QuickPickItemBuilder {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), ..Default::default() }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn picked(mut self, picked: bool) -> Self {
        self.picked = picked;
        self
    }

    pub fn build(self) -> Result<QuickPickItem, WindowError> {
        if self.label.is_empty() {
            return Err(WindowError::EmptyField("label"));
        }
        Ok(QuickPickItem {
            label: self.label,
            description: self.description,
            detail: self.detail,
            picked: self.picked,
        })
    }
}

/// Builder for creating [`InputBoxOptions`].
#[derive(Debug, Clone, Default)]
pub struct InputBoxOptionsBuilder {
    prompt: Option<String>,
    placeholder: Option<String>,
    password: bool,
    value: Option<String>,
    validation_message: Option<String>,
}

impl InputBoxOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn placeholder(mut self, ph: impl Into<String>) -> Self {
        self.placeholder = Some(ph.into());
        self
    }

    pub fn password(mut self, pw: bool) -> Self {
        self.password = pw;
        self
    }

    pub fn value(mut self, val: impl Into<String>) -> Self {
        self.value = Some(val.into());
        self
    }

    pub fn build(self) -> InputBoxOptions {
        InputBoxOptions {
            prompt: self.prompt,
            placeholder: self.placeholder,
            password: self.password,
            value: self.value,
            validation_message: self.validation_message,
        }
    }
}

// ── Validation helpers ──

impl DialogFilter {
    /// Validate that this filter has a non-empty name and at least one extension.
    pub fn validate(&self) -> Result<(), WindowError> {
        if self.name.is_empty() {
            return Err(WindowError::EmptyField("name"));
        }
        if self.extensions.is_empty() {
            return Err(WindowError::EmptyFilterExtensions(self.name.clone()));
        }
        Ok(())
    }

    /// Check whether a filename matches any of this filter's extensions.
    pub fn matches_filename(&self, filename: &str) -> bool {
        self.extensions.iter().any(|ext| {
            filename
                .rsplit_once('.')
                .map_or(false, |(_, file_ext)| file_ext.eq_ignore_ascii_case(ext))
        })
    }
}

impl WindowMessage {
    /// Validate the message payload, returning an error for invalid data.
    pub fn validate(&self) -> Result<(), WindowError> {
        match self {
            Self::ShowInformationMessage { message, .. }
            | Self::ShowWarningMessage { message, .. }
            | Self::ShowErrorMessage { message, .. } => {
                if message.is_empty() {
                    return Err(WindowError::EmptyField("message"));
                }
            }
            Self::CreateStatusBarItem { id, priority, .. } => {
                if id.is_empty() {
                    return Err(WindowError::EmptyField("id"));
                }
                if let Some(p) = priority {
                    if !(-1000..=1000).contains(p) {
                        return Err(WindowError::PriorityOutOfRange(*p));
                    }
                }
            }
            Self::ShowOpenDialog { filters, .. } | Self::ShowSaveDialog { filters, .. } => {
                for filter in filters {
                    filter.validate()?;
                }
            }
            Self::CreateOutputChannel { name } | Self::CreateTerminal { name, .. } => {
                if name.is_empty() {
                    return Err(WindowError::EmptyField("name"));
                }
            }
            Self::CreateWebviewPanel { view_type, title, .. } => {
                if view_type.is_empty() {
                    return Err(WindowError::EmptyField("view_type"));
                }
                if title.is_empty() {
                    return Err(WindowError::EmptyField("title"));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// ── Additional WindowBridge methods ──

impl WindowBridge {
    /// Process a message with validation, returning a `WindowError` if the payload is invalid.
    pub fn handle_validated(&mut self, msg: WindowMessage) -> Result<WindowResponse, WindowError> {
        msg.validate()?;
        Ok(self.handle(msg))
    }

    /// Returns whether a status bar item with the given ID is tracked.
    pub fn has_status_bar_item(&self, id: &str) -> bool {
        self.status_bar_items.iter().any(|s| s == id)
    }

    /// Remove a status bar item by ID.
    pub fn remove_status_bar_item(&mut self, id: &str) -> Result<(), WindowError> {
        let pos = self
            .status_bar_items
            .iter()
            .position(|s| s == id)
            .ok_or_else(|| WindowError::StatusBarItemNotFound(id.to_string()))?;
        self.status_bar_items.remove(pos);
        Ok(())
    }

    /// Returns the number of output channels registered.
    pub fn output_channel_count(&self) -> usize {
        self.output_channels.len()
    }

    /// Returns whether an output channel with the given name exists.
    pub fn has_output_channel(&self, name: &str) -> bool {
        self.output_channels.iter().any(|n| n == name)
    }

    /// Remove an output channel by name.
    pub fn remove_output_channel(&mut self, name: &str) -> Result<(), WindowError> {
        let pos = self
            .output_channels
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| WindowError::OutputChannelNotFound(name.to_string()))?;
        self.output_channels.remove(pos);
        Ok(())
    }

    /// Reset bridge state, clearing all tracked items and resetting the ID counter.
    pub fn reset(&mut self) {
        self.status_bar_items.clear();
        self.output_channels.clear();
        self.next_id = 0;
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

    // ── Additional tests ──

    #[test]
    fn quick_pick_item_builder_ok() {
        let item = QuickPickItemBuilder::new("Open file")
            .description("Opens a file from disk")
            .detail("Ctrl+O")
            .picked(true)
            .build()
            .unwrap();
        assert_eq!(item.label, "Open file");
        assert!(item.picked);
        assert!(item.description.is_some());
    }

    #[test]
    fn quick_pick_item_builder_empty_label() {
        let result = QuickPickItemBuilder::new("").build();
        assert_eq!(result, Err(WindowError::EmptyField("label")));
    }

    #[test]
    fn input_box_options_builder() {
        let opts = InputBoxOptionsBuilder::new()
            .prompt("Enter your name")
            .placeholder("John Doe")
            .password(true)
            .value("default")
            .build();
        assert_eq!(opts.prompt.as_deref(), Some("Enter your name"));
        assert!(opts.password);
        assert_eq!(opts.value.as_deref(), Some("default"));
    }

    #[test]
    fn dialog_filter_validate_ok() {
        let f = DialogFilter {
            name: "Images".into(),
            extensions: vec!["png".into(), "jpg".into()],
        };
        assert!(f.validate().is_ok());
    }

    #[test]
    fn dialog_filter_validate_empty_exts() {
        let f = DialogFilter { name: "Empty".into(), extensions: vec![] };
        assert_eq!(
            f.validate(),
            Err(WindowError::EmptyFilterExtensions("Empty".into()))
        );
    }

    #[test]
    fn dialog_filter_matches_filename() {
        let f = DialogFilter {
            name: "Images".into(),
            extensions: vec!["png".into(), "jpg".into()],
        };
        assert!(f.matches_filename("photo.png"));
        assert!(f.matches_filename("PHOTO.JPG"));
        assert!(!f.matches_filename("document.pdf"));
        assert!(!f.matches_filename("noextension"));
    }

    #[test]
    fn window_message_validate_empty_message() {
        let msg = WindowMessage::ShowInformationMessage {
            message: String::new(),
            items: vec![],
        };
        assert_eq!(msg.validate(), Err(WindowError::EmptyField("message")));
    }

    #[test]
    fn window_message_validate_priority_out_of_range() {
        let msg = WindowMessage::CreateStatusBarItem {
            id: "test".into(),
            alignment: StatusBarAlignment::Left,
            priority: Some(5000),
        };
        assert_eq!(msg.validate(), Err(WindowError::PriorityOutOfRange(5000)));
    }

    #[test]
    fn handle_validated_rejects_invalid() {
        let mut bridge = WindowBridge::new();
        let msg = WindowMessage::CreateOutputChannel { name: String::new() };
        assert!(bridge.handle_validated(msg).is_err());
    }

    #[test]
    fn bridge_remove_status_bar_item() {
        let mut bridge = WindowBridge::new();
        bridge.handle(WindowMessage::CreateStatusBarItem {
            id: "item1".into(),
            alignment: StatusBarAlignment::Right,
            priority: None,
        });
        assert!(bridge.has_status_bar_item("item1"));
        bridge.remove_status_bar_item("item1").unwrap();
        assert!(!bridge.has_status_bar_item("item1"));
        assert_eq!(
            bridge.remove_status_bar_item("item1"),
            Err(WindowError::StatusBarItemNotFound("item1".into()))
        );
    }

    #[test]
    fn bridge_output_channel_lifecycle() {
        let mut bridge = WindowBridge::new();
        bridge.handle(WindowMessage::CreateOutputChannel { name: "Logs".into() });
        assert_eq!(bridge.output_channel_count(), 1);
        assert!(bridge.has_output_channel("Logs"));
        bridge.remove_output_channel("Logs").unwrap();
        assert_eq!(bridge.output_channel_count(), 0);
    }

    #[test]
    fn bridge_reset() {
        let mut bridge = WindowBridge::new();
        bridge.handle(WindowMessage::CreateStatusBarItem {
            id: "s1".into(),
            alignment: StatusBarAlignment::Left,
            priority: None,
        });
        bridge.handle(WindowMessage::CreateOutputChannel { name: "ch".into() });
        bridge.handle(WindowMessage::CreateTerminal { name: "t".into(), shell_path: None });
        assert!(bridge.status_bar_count() > 0);
        bridge.reset();
        assert_eq!(bridge.status_bar_count(), 0);
        assert_eq!(bridge.output_channel_count(), 0);
    }

    #[test]
    fn display_impls() {
        assert_eq!(StatusBarAlignment::Left.to_string(), "Left");
        assert_eq!(StatusBarAlignment::Right.to_string(), "Right");

        let item = QuickPickItem {
            label: "File".into(),
            description: Some("Open file".into()),
            detail: None,
            picked: false,
        };
        assert_eq!(item.to_string(), "File — Open file");

        let filter = DialogFilter {
            name: "Images".into(),
            extensions: vec!["png".into(), "jpg".into()],
        };
        assert_eq!(filter.to_string(), "Images (png, jpg)");
    }

    #[test]
    fn window_error_display() {
        let e = WindowError::EmptyField("id");
        assert_eq!(e.to_string(), "required field is empty: id");
        let e2 = WindowError::PriorityOutOfRange(-2000);
        assert!(e2.to_string().contains("-2000"));
    }
}
