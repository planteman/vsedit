//! Ext API: Window.
//!
//! RPC bridge between the extension host and the main thread for window.

use std::collections::{HashMap, VecDeque};
use std::fmt;
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

/// Accumulated statistics for ext-window operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtWindowStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtWindowStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ExtWindowStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ExtWindowStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtWindowStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtWindowStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-window.
#[derive(Debug, Clone)]
pub struct ExtWindowValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtWindowValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ExtWindowValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Window state tracking
// ---------------------------------------------------------------------------

/// Tracks the current state of the application window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowState {
    /// Whether the window currently has focus.
    pub focused: bool,
    /// Whether the window is visible (not minimized/hidden).
    pub visible: bool,
    /// Whether the window is maximized.
    pub maximized: bool,
    /// Whether the window is in fullscreen mode.
    pub fullscreen: bool,
}

impl WindowState {
    /// A window that is focused, visible, and not maximized.
    pub fn active() -> Self {
        Self {
            focused: true,
            visible: true,
            maximized: false,
            fullscreen: false,
        }
    }

    /// A window that is not focused and not visible.
    pub fn inactive() -> Self {
        Self {
            focused: false,
            visible: false,
            maximized: false,
            fullscreen: false,
        }
    }

    /// Whether the window is currently active (focused and visible).
    pub fn is_active(&self) -> bool {
        self.focused && self.visible
    }

    /// Apply a state change event.
    pub fn apply_focus_change(&mut self, focused: bool) {
        self.focused = focused;
    }

    /// Apply a visibility change.
    pub fn apply_visibility_change(&mut self, visible: bool) {
        self.visible = visible;
        if !visible {
            self.focused = false;
        }
    }

    /// Toggle maximized state.
    pub fn toggle_maximized(&mut self) {
        self.maximized = !self.maximized;
        if self.maximized {
            self.fullscreen = false;
        }
    }

    /// Toggle fullscreen state.
    pub fn toggle_fullscreen(&mut self) {
        self.fullscreen = !self.fullscreen;
        if self.fullscreen {
            self.maximized = false;
        }
    }
}

impl Default for WindowState {
    fn default() -> Self {
        Self::active()
    }
}

impl fmt::Display for WindowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Window(focused={}, visible={}, maximized={}, fullscreen={})",
            self.focused, self.visible, self.maximized, self.fullscreen
        )
    }
}

// ── QuickPickItem extensions ──

impl QuickPickItem {
    pub fn matches_filter(&self, query: &str) -> bool {
        let query_lower = query.to_ascii_lowercase();
        if self.label.to_ascii_lowercase().contains(&query_lower) {
            return true;
        }
        if let Some(ref desc) = self.description {
            if desc.to_ascii_lowercase().contains(&query_lower) {
                return true;
            }
        }
        if let Some(ref detail) = self.detail {
            if detail.to_ascii_lowercase().contains(&query_lower) {
                return true;
            }
        }
        false
    }
}

// ── QuickPickOptions extensions ──

impl QuickPickOptions {
    pub fn is_multi_select(&self) -> bool {
        self.can_pick_many
    }

    pub fn has_placeholder(&self) -> bool {
        self.placeholder.as_ref().map_or(false, |p| !p.is_empty())
    }
}

// ── InputBoxOptions extensions ──

impl InputBoxOptions {
    pub fn has_validation(&self) -> bool {
        self.validation_message.is_some()
    }

    pub fn has_value(&self) -> bool {
        self.value.as_ref().map_or(false, |v| !v.is_empty())
    }

    pub fn is_password(&self) -> bool {
        self.password
    }
}

// ── WindowState extensions ──

impl WindowState {
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_maximized(&self) -> bool {
        self.maximized
    }

    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.focused {
            parts.push("focused");
        }
        if self.visible {
            parts.push("visible");
        }
        if self.maximized {
            parts.push("maximized");
        }
        if self.fullscreen {
            parts.push("fullscreen");
        }
        if parts.is_empty() {
            return "hidden".to_string();
        }
        parts.join(", ")
    }
}

// ── DialogFilter extensions ──

impl DialogFilter {
    pub fn accepts_extension(&self, ext: &str) -> bool {
        self.extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    }

    pub fn all_extensions(&self) -> Vec<&str> {
        self.extensions.iter().map(|e| e.as_str()).collect()
    }
}

// ── WindowBridge extensions ──

impl WindowBridge {
    pub fn message_count(&self) -> usize {
        self.status_bar_items.len() + self.output_channels.len()
    }

    pub fn has_pending_items(&self) -> bool {
        !self.status_bar_items.is_empty() || !self.output_channels.is_empty()
    }
}

// ── QuickPickItemSet ──

#[derive(Debug, Clone, Default)]
pub struct QuickPickItemSet {
    items: Vec<QuickPickItem>,
}

impl QuickPickItemSet {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn with_items(items: Vec<QuickPickItem>) -> Self {
        Self { items }
    }

    pub fn push(&mut self, item: QuickPickItem) {
        self.items.push(item);
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn filter(&self, query: &str) -> Vec<&QuickPickItem> {
        self.items.iter().filter(|i| i.matches_filter(query)).collect()
    }

    pub fn sort_by_label(&mut self) {
        self.items.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn picked(&self) -> Vec<&QuickPickItem> {
        self.items.iter().filter(|i| i.picked).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&QuickPickItem> {
        self.items.iter().find(|i| i.label == label)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.items.iter().map(|i| i.label.as_str()).collect()
    }
}

impl IntoIterator for QuickPickItemSet {
    type Item = QuickPickItem;
    type IntoIter = std::vec::IntoIter<QuickPickItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a QuickPickItemSet {
    type Item = &'a QuickPickItem;
    type IntoIter = std::slice::Iter<'a, QuickPickItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl fmt::Display for QuickPickItemSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuickPickItemSet({} items)", self.items.len())
    }
}

// ── Display for InputBoxOptions ──

impl fmt::Display for InputBoxOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prompt = self.prompt.as_deref().unwrap_or("<no prompt>");
        let pw = if self.password { ", password" } else { "" };
        write!(f, "InputBox({prompt}{pw})")
    }
}

// ── Display for QuickPickOptions ──

impl fmt::Display for QuickPickOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let multi = if self.can_pick_many { "multi" } else { "single" };
        write!(f, "QuickPick({multi})")
    }
}

// ── Split layout management ──

/// Direction of a window split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

impl fmt::Display for SplitDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Horizontal => write!(f, "horizontal"),
            Self::Vertical => write!(f, "vertical"),
        }
    }
}

/// Dimension constraints for a window pane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneConstraints {
    pub min_width: f64,
    pub max_width: f64,
    pub min_height: f64,
    pub max_height: f64,
}

impl PaneConstraints {
    pub fn new(min_width: f64, max_width: f64, min_height: f64, max_height: f64) -> Self {
        Self { min_width, max_width, min_height, max_height }
    }

    /// Clamp a proposed width to the constraint bounds.
    pub fn clamp_width(&self, width: f64) -> f64 {
        width.clamp(self.min_width, self.max_width)
    }

    /// Clamp a proposed height to the constraint bounds.
    pub fn clamp_height(&self, height: f64) -> f64 {
        height.clamp(self.min_height, self.max_height)
    }

    /// Check if a proposed size satisfies both constraints.
    pub fn satisfies(&self, width: f64, height: f64) -> bool {
        width >= self.min_width
            && width <= self.max_width
            && height >= self.min_height
            && height <= self.max_height
    }
}

impl Default for PaneConstraints {
    fn default() -> Self {
        Self { min_width: 80.0, max_width: f64::MAX, min_height: 40.0, max_height: f64::MAX }
    }
}

/// A single pane in a split layout tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pane {
    pub id: String,
    /// Proportional size weight relative to siblings (0.0–1.0).
    pub weight: f64,
    pub constraints: PaneConstraints,
}

impl Pane {
    pub fn new(id: impl Into<String>, weight: f64) -> Self {
        Self { id: id.into(), weight, constraints: PaneConstraints::default() }
    }

    pub fn with_constraints(mut self, constraints: PaneConstraints) -> Self {
        self.constraints = constraints;
        self
    }
}

/// A node in the split layout tree: either a leaf pane or a split container.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LayoutNode {
    Leaf { pane: Pane },
    Split { direction: SplitDirection, children: Vec<LayoutNode> },
}

impl LayoutNode {
    /// Collect all pane IDs in depth-first order.
    pub fn pane_ids(&self) -> Vec<&str> {
        match self {
            Self::Leaf { pane } => vec![pane.id.as_str()],
            Self::Split { children, .. } => {
                children.iter().flat_map(|c| c.pane_ids()).collect()
            }
        }
    }

    /// Count the total number of leaf panes.
    pub fn pane_count(&self) -> usize {
        match self {
            Self::Leaf { .. } => 1,
            Self::Split { children, .. } => children.iter().map(|c| c.pane_count()).sum(),
        }
    }

    /// Find a pane by ID.
    pub fn find_pane(&self, id: &str) -> Option<&Pane> {
        match self {
            Self::Leaf { pane } if pane.id == id => Some(pane),
            Self::Split { children, .. } => {
                children.iter().find_map(|c| c.find_pane(id))
            }
            _ => None,
        }
    }

    /// Split a leaf pane into two panes. Returns `false` if the pane was not found.
    pub fn split_pane(
        &mut self,
        target_id: &str,
        direction: SplitDirection,
        new_pane: Pane,
    ) -> bool {
        match self {
            Self::Leaf { pane } if pane.id == target_id => {
                let existing = pane.clone();
                // Each child gets half the weight.
                let mut left = Pane::new(&existing.id, 0.5);
                left.constraints = existing.constraints;
                let mut right = new_pane;
                right.weight = 0.5;
                *self = Self::Split {
                    direction,
                    children: vec![
                        Self::Leaf { pane: left },
                        Self::Leaf { pane: right },
                    ],
                };
                true
            }
            Self::Split { children, .. } => {
                children.iter_mut().any(|c| c.split_pane(target_id, direction, new_pane.clone()))
            }
            _ => false,
        }
    }

    /// Remove a pane by ID. Returns `true` if removed.
    /// When a split has only one child left, it collapses to that child.
    pub fn remove_pane(&mut self, target_id: &str) -> bool {
        match self {
            Self::Leaf { pane } if pane.id == target_id => {
                // Caller must handle root removal
                return false;
            }
            Self::Split { children, .. } => {
                // Remove direct leaf children matching the ID.
                let before = children.len();
                children.retain(|c| {
                    !matches!(c, Self::Leaf { pane } if pane.id == target_id)
                });
                let removed = children.len() < before;

                if !removed {
                    // Recurse into child splits.
                    for child in children.iter_mut() {
                        if child.remove_pane(target_id) {
                            break;
                        }
                    }
                }

                // Collapse single-child splits.
                if children.len() == 1 {
                    let only = children.remove(0);
                    *self = only;
                }
                removed || self.find_pane(target_id).is_none()
            }
            _ => false,
        }
    }

    /// Normalize child weights so they sum to 1.0.
    pub fn normalize_weights(&mut self) {
        if let Self::Split { children, .. } = self {
            let total: f64 = children.iter().map(|c| match c {
                Self::Leaf { pane } => pane.weight,
                _ => 1.0,
            }).sum();
            if total > 0.0 {
                for child in children.iter_mut() {
                    if let Self::Leaf { pane } = child {
                        pane.weight /= total;
                    }
                }
            }
            for child in children.iter_mut() {
                child.normalize_weights();
            }
        }
    }
}

// ── Tab group management ──

/// A tab group contains an ordered list of tab IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroup {
    pub id: String,
    pub tabs: Vec<String>,
    pub active_index: usize,
}

impl TabGroup {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into(), tabs: Vec::new(), active_index: 0 }
    }

    /// Add a tab at the end and make it active.
    pub fn add_tab(&mut self, tab_id: impl Into<String>) {
        self.tabs.push(tab_id.into());
        self.active_index = self.tabs.len() - 1;
    }

    /// Remove a tab by ID, adjusting the active index. Returns `true` if removed.
    pub fn remove_tab(&mut self, tab_id: &str) -> bool {
        if let Some(pos) = self.tabs.iter().position(|t| t == tab_id) {
            self.tabs.remove(pos);
            if self.tabs.is_empty() {
                self.active_index = 0;
            } else if self.active_index >= self.tabs.len() {
                self.active_index = self.tabs.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Move a tab from one position to another.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() || to >= self.tabs.len() {
            return false;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        self.active_index = to;
        true
    }

    /// Return the currently active tab ID, if any.
    pub fn active_tab(&self) -> Option<&str> {
        self.tabs.get(self.active_index).map(|s| s.as_str())
    }

    /// Select the next tab (wrapping around).
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_index = (self.active_index + 1) % self.tabs.len();
        }
    }

    /// Select the previous tab (wrapping around).
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_index = if self.active_index == 0 {
                self.tabs.len() - 1
            } else {
                self.active_index - 1
            };
        }
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}

impl fmt::Display for TabGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TabGroup({}, {} tabs)", self.id, self.tabs.len())
    }
}

// ── Focus history ──

/// Tracks the most-recently-focused pane IDs in a bounded ring buffer.
#[derive(Debug, Clone)]
pub struct FocusHistory {
    entries: VecDeque<String>,
    capacity: usize,
}

impl FocusHistory {
    pub fn new(capacity: usize) -> Self {
        Self { entries: VecDeque::with_capacity(capacity), capacity }
    }

    /// Record that a pane received focus. Duplicates are moved to the front.
    pub fn record_focus(&mut self, pane_id: impl Into<String>) {
        let id = pane_id.into();
        // Remove existing entry so the ID appears only once, at the front.
        self.entries.retain(|e| e != &id);
        if self.entries.len() >= self.capacity {
            self.entries.pop_back();
        }
        self.entries.push_front(id);
    }

    /// The most recently focused pane ID.
    pub fn current(&self) -> Option<&str> {
        self.entries.front().map(|s| s.as_str())
    }

    /// The previously focused pane ID (second in the stack).
    pub fn previous(&self) -> Option<&str> {
        self.entries.get(1).map(|s| s.as_str())
    }

    /// Return the full history from most-recent to least-recent.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove a pane from the history (e.g. when the pane is closed).
    pub fn remove(&mut self, pane_id: &str) {
        self.entries.retain(|e| e != pane_id);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ── Workspace layout snapshot for serialization / restore ──

/// Serialisable snapshot of the entire window workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub layout: LayoutNode,
    pub tab_groups: Vec<TabGroup>,
    pub window_state: WindowState,
}

impl WorkspaceSnapshot {
    pub fn new(layout: LayoutNode, tab_groups: Vec<TabGroup>, window_state: WindowState) -> Self {
        Self { layout, tab_groups, window_state }
    }

    /// Serialize the snapshot to a JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize a snapshot from a JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Return all pane IDs referenced in the layout.
    pub fn pane_ids(&self) -> Vec<&str> {
        self.layout.pane_ids()
    }
}


// ---------------------------------------------------------------------------
// WindowInputBoxWithValidation
// ---------------------------------------------------------------------------

pub struct WindowInputBoxWithValidation {
    options: InputBoxOptions,
    validator: Option<Box<dyn Fn(&str) -> Result<(), String>>>,
}

impl WindowInputBoxWithValidation {
    pub fn new(options: InputBoxOptions) -> Self {
        Self { options, validator: None }
    }

    pub fn with_validator<F: Fn(&str) -> Result<(), String> + 'static>(mut self, f: F) -> Self {
        self.validator = Some(Box::new(f));
        self
    }

    pub fn validate(&self, input: &str) -> Result<(), String> {
        if let Some(ref v) = self.validator { v(input) } else { Ok(()) }
    }

    pub fn prompt(&self) -> Option<&str> { self.options.prompt.as_deref() }
    pub fn is_password(&self) -> bool { self.options.password }
}

// ---------------------------------------------------------------------------
// WindowStatusBarManager
// ---------------------------------------------------------------------------

pub struct WindowStatusBarManager {
    items: Vec<(String, String, StatusBarAlignment)>,
}

impl WindowStatusBarManager {
    pub fn new() -> Self { Self { items: Vec::new() } }

    pub fn add_item(&mut self, id: impl Into<String>, text: impl Into<String>, alignment: StatusBarAlignment) {
        self.items.push((id.into(), text.into(), alignment));
    }

    pub fn remove_item(&mut self, id: &str) -> bool {
        if let Some(i) = self.items.iter().position(|(iid, _, _)| iid == id) {
            self.items.remove(i);
            true
        } else {
            false
        }
    }

    pub fn update_text(&mut self, id: &str, text: impl Into<String>) -> bool {
        if let Some(item) = self.items.iter_mut().find(|(iid, _, _)| iid == id) {
            item.1 = text.into();
            true
        } else {
            false
        }
    }

    pub fn get_text(&self, id: &str) -> Option<&str> {
        self.items.iter().find(|(iid, _, _)| iid == id).map(|(_, t, _)| t.as_str())
    }

    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn left_items(&self) -> Vec<(&str, &str)> {
        self.items.iter()
            .filter(|(_, _, a)| matches!(a, StatusBarAlignment::Left))
            .map(|(id, t, _)| (id.as_str(), t.as_str()))
            .collect()
    }
}

impl Default for WindowStatusBarManager { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// WindowOutputChannelFactory
// ---------------------------------------------------------------------------

pub struct WindowOutputChannelFactory {
    channels: Vec<String>,
}

impl WindowOutputChannelFactory {
    pub fn new() -> Self { Self { channels: Vec::new() } }

    pub fn create(&mut self, name: impl Into<String>) -> String {
        let name = name.into();
        self.channels.push(name.clone());
        name
    }

    pub fn has_channel(&self, name: &str) -> bool { self.channels.iter().any(|c| c == name) }
    pub fn channel_count(&self) -> usize { self.channels.len() }
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(i) = self.channels.iter().position(|c| c == name) { self.channels.remove(i); true } else { false }
    }
}

impl Default for WindowOutputChannelFactory { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// WindowActiveTheme
// ---------------------------------------------------------------------------

pub struct WindowActiveTheme {
    pub name: String,
    pub kind: String,
}

impl WindowActiveTheme {
    pub fn dark(name: impl Into<String>) -> Self { Self { name: name.into(), kind: "dark".into() } }
    pub fn light(name: impl Into<String>) -> Self { Self { name: name.into(), kind: "light".into() } }
    pub fn is_dark(&self) -> bool { self.kind == "dark" }
    pub fn is_light(&self) -> bool { self.kind == "light" }
}

impl std::fmt::Display for WindowActiveTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.kind)
    }
}


// ── Modal Dialog Handler ──

/// Possible states of a modal dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalDialogState {
    /// Dialog is not visible.
    Hidden,
    /// Dialog is visible and awaiting user input.
    Open,
    /// User confirmed the dialog.
    Confirmed,
    /// User dismissed the dialog.
    Dismissed,
    /// Dialog timed out without user action.
    TimedOut,
}

impl fmt::Display for ModalDialogState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModalDialogState::Hidden => write!(f, "hidden"),
            ModalDialogState::Open => write!(f, "open"),
            ModalDialogState::Confirmed => write!(f, "confirmed"),
            ModalDialogState::Dismissed => write!(f, "dismissed"),
            ModalDialogState::TimedOut => write!(f, "timed_out"),
        }
    }
}

/// A response from a modal dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalDialogResponse {
    pub dialog_id: String,
    pub selected_button: Option<String>,
    pub input_value: Option<String>,
    pub state: ModalDialogState,
}

impl ModalDialogResponse {
    pub fn confirmed(dialog_id: impl Into<String>, button: impl Into<String>) -> Self {
        Self {
            dialog_id: dialog_id.into(),
            selected_button: Some(button.into()),
            input_value: None,
            state: ModalDialogState::Confirmed,
        }
    }

    pub fn dismissed(dialog_id: impl Into<String>) -> Self {
        Self {
            dialog_id: dialog_id.into(),
            selected_button: None,
            input_value: None,
            state: ModalDialogState::Dismissed,
        }
    }

    pub fn is_confirmed(&self) -> bool {
        self.state == ModalDialogState::Confirmed
    }
}

/// Manages modal dialog state and queued responses.
pub struct WindowModalHandler {
    pending_dialogs: VecDeque<String>,
    responses: Vec<ModalDialogResponse>,
    max_queue_size: usize,
    current_state: ModalDialogState,
}

impl WindowModalHandler {
    pub fn new(max_queue_size: usize) -> Self {
        Self {
            pending_dialogs: VecDeque::new(),
            responses: Vec::new(),
            max_queue_size,
            current_state: ModalDialogState::Hidden,
        }
    }

    /// Enqueue a dialog. Returns false if queue is full.
    pub fn enqueue_dialog(&mut self, dialog_id: impl Into<String>) -> bool {
        if self.pending_dialogs.len() >= self.max_queue_size {
            return false;
        }
        self.pending_dialogs.push_back(dialog_id.into());
        if self.current_state == ModalDialogState::Hidden {
            self.current_state = ModalDialogState::Open;
        }
        true
    }

    /// Pop the next pending dialog to show.
    pub fn next_dialog(&mut self) -> Option<String> {
        let dialog = self.pending_dialogs.pop_front();
        if self.pending_dialogs.is_empty() && dialog.is_some() {
            self.current_state = ModalDialogState::Open;
        }
        dialog
    }

    /// Record a response for a dialog.
    pub fn record_response(&mut self, response: ModalDialogResponse) {
        self.current_state = response.state;
        self.responses.push(response);
        if !self.pending_dialogs.is_empty() {
            self.current_state = ModalDialogState::Open;
        } else {
            self.current_state = ModalDialogState::Hidden;
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending_dialogs.len()
    }

    pub fn response_count(&self) -> usize {
        self.responses.len()
    }

    pub fn state(&self) -> ModalDialogState {
        self.current_state
    }

    pub fn confirmed_responses(&self) -> Vec<&ModalDialogResponse> {
        self.responses.iter().filter(|r| r.is_confirmed()).collect()
    }

    pub fn clear_responses(&mut self) {
        self.responses.clear();
    }

    /// Find the response for a specific dialog id.
    pub fn find_response(&self, dialog_id: &str) -> Option<&ModalDialogResponse> {
        self.responses.iter().find(|r| r.dialog_id == dialog_id)
    }
}

// ── Tab Group Manager ──

/// Represents a single tab within a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabEntry {
    pub id: String,
    pub label: String,
    pub is_dirty: bool,
    pub is_pinned: bool,
    pub sort_order: u32,
}

impl TabEntry {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            is_dirty: false,
            is_pinned: false,
            sort_order: 0,
        }
    }

    pub fn with_pinned(mut self, pinned: bool) -> Self {
        self.is_pinned = pinned;
        self
    }

    pub fn with_order(mut self, order: u32) -> Self {
        self.sort_order = order;
        self
    }
}

/// Manages a collection of tab groups with focus tracking and ordering.
pub struct WindowTabGroupManager {
    groups: Vec<(String, Vec<TabEntry>)>,
    active_group_index: Option<usize>,
    active_tab_ids: Vec<Option<String>>,
}

impl WindowTabGroupManager {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            active_group_index: None,
            active_tab_ids: Vec::new(),
        }
    }

    /// Add a new tab group. Returns the group index.
    pub fn add_group(&mut self, name: impl Into<String>) -> usize {
        let idx = self.groups.len();
        self.groups.push((name.into(), Vec::new()));
        self.active_tab_ids.push(None);
        if self.active_group_index.is_none() {
            self.active_group_index = Some(idx);
        }
        idx
    }

    /// Add a tab to a group. Returns false if group doesn't exist.
    pub fn add_tab(&mut self, group_index: usize, tab: TabEntry) -> bool {
        if group_index >= self.groups.len() {
            return false;
        }
        let tab_id = tab.id.clone();
        self.groups[group_index].1.push(tab);
        if self.active_tab_ids[group_index].is_none() {
            self.active_tab_ids[group_index] = Some(tab_id);
        }
        true
    }

    /// Set the active tab in a group.
    pub fn set_active_tab(&mut self, group_index: usize, tab_id: &str) -> bool {
        if group_index >= self.groups.len() {
            return false;
        }
        let exists = self.groups[group_index].1.iter().any(|t| t.id == tab_id);
        if exists {
            self.active_tab_ids[group_index] = Some(tab_id.to_string());
            self.active_group_index = Some(group_index);
            true
        } else {
            false
        }
    }

    /// Set the active group.
    pub fn set_active_group(&mut self, group_index: usize) -> bool {
        if group_index < self.groups.len() {
            self.active_group_index = Some(group_index);
            true
        } else {
            false
        }
    }

    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    pub fn tab_count(&self, group_index: usize) -> usize {
        self.groups.get(group_index).map_or(0, |(_, tabs)| tabs.len())
    }

    pub fn active_group_index(&self) -> Option<usize> {
        self.active_group_index
    }

    pub fn active_tab_id(&self, group_index: usize) -> Option<&str> {
        self.active_tab_ids.get(group_index).and_then(|id| id.as_deref())
    }

    /// Remove a tab by id from a group. Returns the removed tab, if found.
    pub fn remove_tab(&mut self, group_index: usize, tab_id: &str) -> Option<TabEntry> {
        if group_index >= self.groups.len() {
            return None;
        }
        let tabs = &mut self.groups[group_index].1;
        let pos = tabs.iter().position(|t| t.id == tab_id)?;
        let removed = tabs.remove(pos);
        if self.active_tab_ids[group_index].as_deref() == Some(tab_id) {
            self.active_tab_ids[group_index] = tabs.first().map(|t| t.id.clone());
        }
        Some(removed)
    }

    /// Get sorted tabs in a group (pinned first, then by sort_order).
    pub fn sorted_tabs(&self, group_index: usize) -> Vec<&TabEntry> {
        let Some((_, tabs)) = self.groups.get(group_index) else {
            return Vec::new();
        };
        let mut sorted: Vec<&TabEntry> = tabs.iter().collect();
        sorted.sort_by(|a, b| {
            b.is_pinned.cmp(&a.is_pinned).then(a.sort_order.cmp(&b.sort_order))
        });
        sorted
    }

    /// Count total dirty tabs across all groups.
    pub fn dirty_tab_count(&self) -> usize {
        self.groups.iter().flat_map(|(_, tabs)| tabs).filter(|t| t.is_dirty).count()
    }
}



// ─── ExtWin Builder & Validator ─────────────────────────────

/// Builder for constructing window configurations.
#[derive(Debug, Clone)]
pub struct ExtWinBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl ExtWinBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<ExtWinCfg, ExtWinBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(ExtWinBuildErr { errors }); }
        Ok(ExtWinCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated window configuration.
#[derive(Debug, Clone)]
pub struct ExtWinCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl ExtWinCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &ExtWinCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for ExtWinCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtWinCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct ExtWinBuildErr { pub errors: Vec<String> }

impl fmt::Display for ExtWinBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExtWinBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for ExtWinBuildErr {}

// ─── ExtWin Formatter ───────────────────────────────────────

/// Formatting options for window output.
#[derive(Debug, Clone)]
pub struct ExtWinFmtOpts {
    pub indent: usize,
    pub max_width: usize,
    pub use_color: bool,
    pub separator: String,
    pub prefix_str: String,
}

impl Default for ExtWinFmtOpts {
    fn default() -> Self {
        Self { indent: 2, max_width: 120, use_color: false,
               separator: ", ".into(), prefix_str: String::new() }
    }
}

impl ExtWinFmtOpts {
    pub fn with_indent(mut self, indent: usize) -> Self { self.indent = indent; self }
    pub fn with_max_width(mut self, width: usize) -> Self { self.max_width = width; self }
    pub fn with_color(mut self) -> Self { self.use_color = true; self }
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self { self.separator = sep.into(); self }
    pub fn with_prefix(mut self, p: impl Into<String>) -> Self { self.prefix_str = p.into(); self }
}

/// Formatter for window data.
pub struct ExtWinFmt {
    options: ExtWinFmtOpts,
}

impl ExtWinFmt {
    pub fn new(options: ExtWinFmtOpts) -> Self { Self { options } }
    pub fn default_fmt() -> Self { Self { options: ExtWinFmtOpts::default() } }

    pub fn format_list(&self, items: &[&str]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut result = String::new();
        let mut line_len = 0usize;
        for (i, item) in items.iter().enumerate() {
            let formatted = if self.options.prefix_str.is_empty() {
                format!("{}{}", ind, item)
            } else {
                format!("{}{}{}", ind, self.options.prefix_str, item)
            };
            if i > 0 && line_len + formatted.len() > self.options.max_width {
                result.push('\n'); line_len = 0;
            } else if i > 0 {
                result.push_str(&self.options.separator);
                line_len += self.options.separator.len();
            }
            line_len += formatted.len();
            result.push_str(&formatted);
        }
        result
    }

    pub fn format_kv(&self, key: &str, value: &str) -> String {
        format!("{}{} = {}", " ".repeat(self.options.indent), key, value)
    }

    pub fn format_section(&self, heading: &str, lines: &[String]) -> String {
        let ind = " ".repeat(self.options.indent);
        let mut r = format!("[{}]\n", heading);
        for line in lines { r.push_str(&format!("{}{}\n", ind, line)); }
        r
    }

    pub fn truncate(&self, s: &str) -> String {
        if s.len() <= self.options.max_width { s.to_string() }
        else {
            let end = self.options.max_width.saturating_sub(3);
            format!("{}...", &s[..end])
        }
    }
}


/// Configuration manager for ext_window functionality.
pub struct ExtWindowConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtWindowConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &ExtWindowConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_window operations.
pub struct ExtWindowRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtWindowRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for ext_window.
pub struct ExtWindowValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtWindowValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &ExtWindowValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Extension window API — extended utilities (qw)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_win operations.
#[derive(Debug, Clone)]
pub struct QwMetrics {
    samples: Vec<f64>,
    label: String,
}

impl QwMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for ext_win.
#[derive(Debug, Clone)]
pub struct QwRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl QwRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for ext_win lookups.
#[derive(Debug, Clone)]
pub struct QwLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl QwLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 12
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer12 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer12 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_12(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_12<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_12<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_12(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_12(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 78
// ---------------------------------------------------------------------------

/// Generic object pool `Xc78Pool<T>`.
pub struct Xc78Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc78Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc78PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc78Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc78PoolStats {
        Xc78PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc78Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc78Scheduler`.
pub struct Xc78Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc78Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc78Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_78 hash for the given byte slice.
pub fn xc_78_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_78 convention.
pub fn xc_78_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe24 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe24Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe24PipelineError {
    pub stage: Xe24Stage,
    pub message: String,
}

impl std::fmt::Display for Xe24PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe24Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe24Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError>>>,
    stage_names: Vec<Xe24Stage>,
}

impl Xe24Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe24Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe24Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe24Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe24Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe24Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe24CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe24CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe24Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe24CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe24CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe24Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe24CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_24_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe24CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_24_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe24CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_24_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> {
    Ok(data)
}

pub fn xe_24_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_24_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_24_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_24_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe24PipelineError> {
    Err(Xe24PipelineError {
        stage: Xe24Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #108
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf108Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf108TrieNode {
    children: std::collections::HashMap<char, Xf108TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf108Trie {
    root: Xf108TrieNode,
    count: usize,
}

impl Xf108Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf108TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf108TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf108TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf108BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf108BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 77).
pub struct Xh77SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh77SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 119 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 77).
pub struct Xh77BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh77BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
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

    #[test]
    fn ext_window_stats_new_defaults() {
        let stats = ExtWindowStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_window_stats_record_success() {
        let mut stats = ExtWindowStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_window_stats_record_failure() {
        let mut stats = ExtWindowStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_window_stats_reset() {
        let mut stats = ExtWindowStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_window_stats_merge() {
        let mut a = ExtWindowStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtWindowStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn ext_window_stats_display() {
        let mut stats = ExtWindowStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_window_stats_default() {
        let stats = ExtWindowStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn extwindow_validator_accepts_and_rejects() {
        let mut v = ExtWindowValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn extwindow_validator_warnings() {
        let mut v = ExtWindowValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn extwindow_validator_clear_and_merge() {
        let mut v = ExtWindowValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = ExtWindowValidationCollector::new();
        a.add_error("a_err");
        let mut b = ExtWindowValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn window_state_active() {
        let ws = WindowState::active();
        assert!(ws.is_active());
        assert!(ws.focused);
        assert!(ws.visible);
    }

    #[test]
    fn window_state_inactive() {
        let ws = WindowState::inactive();
        assert!(!ws.is_active());
        assert!(!ws.focused);
        assert!(!ws.visible);
    }

    #[test]
    fn window_state_focus_change() {
        let mut ws = WindowState::active();
        ws.apply_focus_change(false);
        assert!(!ws.focused);
        assert!(ws.visible);
        assert!(!ws.is_active());
    }

    #[test]
    fn window_state_visibility_clears_focus() {
        let mut ws = WindowState::active();
        ws.apply_visibility_change(false);
        assert!(!ws.visible);
        assert!(!ws.focused);
    }

    #[test]
    fn window_state_toggle_maximized() {
        let mut ws = WindowState::active();
        ws.toggle_maximized();
        assert!(ws.maximized);
        assert!(!ws.fullscreen);
        ws.toggle_maximized();
        assert!(!ws.maximized);
    }

    #[test]
    fn window_state_fullscreen_clears_maximized() {
        let mut ws = WindowState::active();
        ws.toggle_maximized();
        assert!(ws.maximized);
        ws.toggle_fullscreen();
        assert!(ws.fullscreen);
        assert!(!ws.maximized);
    }

    #[test]
    fn window_state_serialization() {
        let ws = WindowState::active();
        let json = serde_json::to_string(&ws).unwrap();
        let ws2: WindowState = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, ws2);
    }

    #[test]
    fn quick_pick_item_matches_filter() {
        let item = QuickPickItem {
            label: "Open File".into(),
            description: Some("Opens a document".into()),
            detail: Some("Ctrl+O shortcut".into()),
            picked: false,
        };
        assert!(item.matches_filter("open"));
        assert!(item.matches_filter("DOCUMENT"));
        assert!(item.matches_filter("shortcut"));
        assert!(!item.matches_filter("save"));

        let minimal = QuickPickItem {
            label: "Run".into(),
            description: None,
            detail: None,
            picked: false,
        };
        assert!(minimal.matches_filter("run"));
        assert!(!minimal.matches_filter("debug"));
    }

    #[test]
    fn input_box_options_extensions() {
        let opts = InputBoxOptionsBuilder::new()
            .prompt("Name")
            .value("hello")
            .password(true)
            .build();
        assert!(!opts.has_validation());
        assert!(opts.has_value());
        assert!(opts.is_password());

        let empty = InputBoxOptionsBuilder::new().build();
        assert!(!empty.has_value());
        assert!(!empty.is_password());
    }

    #[test]
    fn quick_pick_options_extensions() {
        let opts = QuickPickOptions { placeholder: Some("Type here".into()), can_pick_many: true };
        assert!(opts.is_multi_select());
        assert!(opts.has_placeholder());

        let opts2 = QuickPickOptions { placeholder: None, can_pick_many: false };
        assert!(!opts2.is_multi_select());
        assert!(!opts2.has_placeholder());
    }

    #[test]
    fn window_state_summary_and_accessors() {
        let ws = WindowState::active();
        assert!(ws.is_focused());
        assert!(!ws.is_maximized());
        assert_eq!(ws.summary(), "focused, visible");

        let inactive = WindowState::inactive();
        assert_eq!(inactive.summary(), "hidden");

        let mut max = WindowState::active();
        max.toggle_maximized();
        assert!(max.is_maximized());
        assert!(max.summary().contains("maximized"));
    }

    #[test]
    fn dialog_filter_accepts_extension() {
        let f = DialogFilter {
            name: "Images".into(),
            extensions: vec!["png".into(), "jpg".into(), "gif".into()],
        };
        assert!(f.accepts_extension("png"));
        assert!(f.accepts_extension("PNG"));
        assert!(!f.accepts_extension("bmp"));
        assert_eq!(f.all_extensions(), vec!["png", "jpg", "gif"]);
    }

    #[test]
    fn quick_pick_item_set_operations() {
        let mut set = QuickPickItemSet::new();
        assert!(set.is_empty());

        set.push(QuickPickItem {
            label: "Zebra".into(),
            description: Some("Animal".into()),
            detail: None,
            picked: true,
        });
        set.push(QuickPickItem {
            label: "Apple".into(),
            description: None,
            detail: None,
            picked: false,
        });
        set.push(QuickPickItem {
            label: "Mango".into(),
            description: Some("Fruit".into()),
            detail: None,
            picked: true,
        });

        assert_eq!(set.len(), 3);
        assert_eq!(set.picked().len(), 2);
        assert!(set.find_by_label("Apple").is_some());
        assert!(set.find_by_label("Banana").is_none());

        let filtered = set.filter("animal");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "Zebra");

        set.sort_by_label();
        assert_eq!(set.labels(), vec!["Apple", "Mango", "Zebra"]);

        let display = format!("{set}");
        assert!(display.contains("3 items"));

        let collected: Vec<_> = set.into_iter().collect();
        assert_eq!(collected.len(), 3);
    }

    #[test]
    fn bridge_message_count_and_pending() {
        let mut bridge = WindowBridge::new();
        assert_eq!(bridge.message_count(), 0);
        assert!(!bridge.has_pending_items());

        bridge.handle(WindowMessage::CreateStatusBarItem {
            id: "sb1".into(),
            alignment: StatusBarAlignment::Left,
            priority: None,
        });
        bridge.handle(WindowMessage::CreateOutputChannel { name: "Log".into() });
        assert_eq!(bridge.message_count(), 2);
        assert!(bridge.has_pending_items());
    }

    #[test]
    fn display_impls_new() {
        let opts = InputBoxOptions {
            prompt: Some("Enter name".into()),
            placeholder: None,
            password: true,
            value: None,
            validation_message: None,
        };
        assert_eq!(opts.to_string(), "InputBox(Enter name, password)");

        let qp = QuickPickOptions { placeholder: None, can_pick_many: false };
        assert_eq!(qp.to_string(), "QuickPick(single)");

        let qp_multi = QuickPickOptions { placeholder: None, can_pick_many: true };
        assert_eq!(qp_multi.to_string(), "QuickPick(multi)");
    }

    // ── Split layout tests ──

    #[test]
    fn layout_split_pane_and_count() {
        let mut root = LayoutNode::Leaf {
            pane: Pane::new("editor1", 1.0),
        };
        assert_eq!(root.pane_count(), 1);

        let ok = root.split_pane("editor1", SplitDirection::Vertical, Pane::new("editor2", 0.5));
        assert!(ok);
        assert_eq!(root.pane_count(), 2);
        assert!(root.find_pane("editor1").is_some());
        assert!(root.find_pane("editor2").is_some());
        assert_eq!(root.pane_ids(), vec!["editor1", "editor2"]);
    }

    #[test]
    fn layout_remove_pane_collapses_split() {
        let mut root = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutNode::Leaf { pane: Pane::new("a", 0.5) },
                LayoutNode::Leaf { pane: Pane::new("b", 0.5) },
            ],
        };
        assert!(root.remove_pane("b"));
        assert_eq!(root.pane_count(), 1);
        // After collapse, root should be a leaf.
        assert!(matches!(root, LayoutNode::Leaf { .. }));
    }

    #[test]
    fn tab_group_add_remove_move() {
        let mut tg = TabGroup::new("group1");
        assert!(tg.is_empty());
        tg.add_tab("file1.rs");
        tg.add_tab("file2.rs");
        tg.add_tab("file3.rs");
        assert_eq!(tg.len(), 3);
        assert_eq!(tg.active_tab(), Some("file3.rs"));

        // Move last tab to front.
        assert!(tg.move_tab(2, 0));
        assert_eq!(tg.tabs, vec!["file3.rs", "file1.rs", "file2.rs"]);
        assert_eq!(tg.active_tab(), Some("file3.rs"));

        // Remove active tab.
        tg.remove_tab("file3.rs");
        assert_eq!(tg.len(), 2);
        assert_eq!(tg.active_tab(), Some("file1.rs"));

        // next/prev cycling.
        tg.next_tab();
        assert_eq!(tg.active_tab(), Some("file2.rs"));
        tg.prev_tab();
        assert_eq!(tg.active_tab(), Some("file1.rs"));
        tg.prev_tab(); // wrap
        assert_eq!(tg.active_tab(), Some("file2.rs"));
    }

    #[test]
    fn focus_history_tracks_order_and_deduplicates() {
        let mut fh = FocusHistory::new(4);
        assert!(fh.is_empty());
        fh.record_focus("pane-a");
        fh.record_focus("pane-b");
        fh.record_focus("pane-c");
        assert_eq!(fh.current(), Some("pane-c"));
        assert_eq!(fh.previous(), Some("pane-b"));
        assert_eq!(fh.len(), 3);

        // Re-focusing an existing pane moves it to front.
        fh.record_focus("pane-a");
        assert_eq!(fh.current(), Some("pane-a"));
        assert_eq!(fh.previous(), Some("pane-c"));
        assert_eq!(fh.len(), 3); // no duplicates

        // Removing an entry.
        fh.remove("pane-b");
        assert_eq!(fh.len(), 2);
        let ids: Vec<&str> = fh.iter().collect();
        assert_eq!(ids, vec!["pane-a", "pane-c"]);

        // Capacity enforcement.
        fh.record_focus("d1");
        fh.record_focus("d2");
        fh.record_focus("d3"); // exceeds capacity of 4
        assert!(fh.len() <= 4);
    }

    #[test]
    fn workspace_snapshot_serde_roundtrip() {
        let layout = LayoutNode::Split {
            direction: SplitDirection::Vertical,
            children: vec![
                LayoutNode::Leaf { pane: Pane::new("left", 0.4) },
                LayoutNode::Leaf { pane: Pane::new("right", 0.6) },
            ],
        };
        let mut tg = TabGroup::new("main");
        tg.add_tab("file1.rs");
        tg.add_tab("file2.rs");
        let snapshot = WorkspaceSnapshot::new(layout, vec![tg], WindowState::active());

        let json = snapshot.to_json().unwrap();
        let restored = WorkspaceSnapshot::from_json(&json).unwrap();
        assert_eq!(snapshot, restored);
        assert_eq!(restored.pane_ids(), vec!["left", "right"]);
    }

    #[test]
    fn pane_constraints_clamp_and_satisfies() {
        let c = PaneConstraints::new(100.0, 800.0, 50.0, 600.0);
        assert_eq!(c.clamp_width(50.0), 100.0);
        assert_eq!(c.clamp_width(1000.0), 800.0);
        assert_eq!(c.clamp_width(400.0), 400.0);
        assert_eq!(c.clamp_height(10.0), 50.0);
        assert!(c.satisfies(400.0, 300.0));
        assert!(!c.satisfies(50.0, 300.0));
        assert!(!c.satisfies(400.0, 700.0));
    }

    #[test]
    fn layout_normalize_weights() {
        let mut root = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            children: vec![
                LayoutNode::Leaf { pane: Pane::new("a", 3.0) },
                LayoutNode::Leaf { pane: Pane::new("b", 1.0) },
            ],
        };
        root.normalize_weights();
        let a = root.find_pane("a").unwrap();
        let b = root.find_pane("b").unwrap();
        assert!((a.weight - 0.75).abs() < 1e-9);
        assert!((b.weight - 0.25).abs() < 1e-9);
    }


    #[test]
    fn input_box_validation_pass() {
        let opts = InputBoxOptionsBuilder::new().build();
        let ib = WindowInputBoxWithValidation::new(opts)
            .with_validator(|s| if s.is_empty() { Err("empty".into()) } else { Ok(()) });
        assert!(ib.validate("hello").is_ok());
        assert!(ib.validate("").is_err());
    }

    #[test]
    fn input_box_no_validator() {
        let opts = InputBoxOptionsBuilder::new().build();
        let ib = WindowInputBoxWithValidation::new(opts);
        assert!(ib.validate("anything").is_ok());
    }

    #[test]
    fn statusbar_manager_basic() {
        let mut mgr = WindowStatusBarManager::new();
        mgr.add_item("git", "main", StatusBarAlignment::Left);
        mgr.add_item("line", "Ln 1", StatusBarAlignment::Right);
        assert_eq!(mgr.len(), 2);
        assert_eq!(mgr.get_text("git"), Some("main"));
    }

    #[test]
    fn statusbar_manager_update() {
        let mut mgr = WindowStatusBarManager::new();
        mgr.add_item("git", "main", StatusBarAlignment::Left);
        assert!(mgr.update_text("git", "develop"));
        assert_eq!(mgr.get_text("git"), Some("develop"));
    }

    #[test]
    fn statusbar_manager_remove() {
        let mut mgr = WindowStatusBarManager::new();
        mgr.add_item("git", "main", StatusBarAlignment::Left);
        assert!(mgr.remove_item("git"));
        assert!(mgr.is_empty());
    }

    #[test]
    fn statusbar_manager_left_items() {
        let mut mgr = WindowStatusBarManager::new();
        mgr.add_item("a", "A", StatusBarAlignment::Left);
        mgr.add_item("b", "B", StatusBarAlignment::Right);
        assert_eq!(mgr.left_items().len(), 1);
    }

    #[test]
    fn output_channel_factory() {
        let mut f = WindowOutputChannelFactory::new();
        f.create("Output");
        assert!(f.has_channel("Output"));
        assert_eq!(f.channel_count(), 1);
        assert!(f.remove("Output"));
        assert!(!f.has_channel("Output"));
    }

    #[test]
    fn active_theme_dark() {
        let t = WindowActiveTheme::dark("One Dark Pro");
        assert!(t.is_dark());
        assert!(!t.is_light());
    }

    #[test]
    fn active_theme_light() {
        let t = WindowActiveTheme::light("Solarized");
        assert!(t.is_light());
    }

    #[test]
    fn active_theme_display() {
        let t = WindowActiveTheme::dark("Monokai");
        assert!(format!("{t}").contains("Monokai"));
    }

    #[test]
    fn input_box_password() {
        let opts = InputBoxOptionsBuilder::new().password(true).build();
        let ib = WindowInputBoxWithValidation::new(opts);
        assert!(ib.is_password());
    }

    #[test]
    fn output_factory_no_dup() {
        let mut f = WindowOutputChannelFactory::new();
        f.create("ch1");
        f.create("ch1");
        assert_eq!(f.channel_count(), 2);
    }


    #[test]
    fn modal_dialog_state_display() {
        assert_eq!(format!("{}", ModalDialogState::Hidden), "hidden");
        assert_eq!(format!("{}", ModalDialogState::Open), "open");
        assert_eq!(format!("{}", ModalDialogState::Confirmed), "confirmed");
        assert_eq!(format!("{}", ModalDialogState::Dismissed), "dismissed");
        assert_eq!(format!("{}", ModalDialogState::TimedOut), "timed_out");
    }

    #[test]
    fn modal_handler_enqueue_and_next() {
        let mut handler = WindowModalHandler::new(3);
        assert!(handler.enqueue_dialog("d1"));
        assert!(handler.enqueue_dialog("d2"));
        assert_eq!(handler.pending_count(), 2);
        assert_eq!(handler.next_dialog(), Some("d1".to_string()));
        assert_eq!(handler.pending_count(), 1);
    }

    #[test]
    fn modal_handler_queue_full() {
        let mut handler = WindowModalHandler::new(2);
        assert!(handler.enqueue_dialog("d1"));
        assert!(handler.enqueue_dialog("d2"));
        assert!(!handler.enqueue_dialog("d3"));
    }

    #[test]
    fn modal_handler_record_response() {
        let mut handler = WindowModalHandler::new(5);
        handler.enqueue_dialog("d1");
        handler.next_dialog();
        handler.record_response(ModalDialogResponse::confirmed("d1", "OK"));
        assert_eq!(handler.response_count(), 1);
        assert_eq!(handler.confirmed_responses().len(), 1);
        assert_eq!(handler.state(), ModalDialogState::Hidden);
    }

    #[test]
    fn modal_handler_find_response() {
        let mut handler = WindowModalHandler::new(5);
        handler.record_response(ModalDialogResponse::dismissed("d1"));
        let r = handler.find_response("d1").unwrap();
        assert!(!r.is_confirmed());
        assert!(handler.find_response("d2").is_none());
    }

    #[test]
    fn modal_response_confirmed_builder() {
        let r = ModalDialogResponse::confirmed("dlg1", "Yes");
        assert!(r.is_confirmed());
        assert_eq!(r.selected_button.as_deref(), Some("Yes"));
    }

    #[test]
    fn tab_entry_builder() {
        let tab = TabEntry::new("t1", "Tab 1").with_pinned(true).with_order(5);
        assert!(tab.is_pinned);
        assert_eq!(tab.sort_order, 5);
        assert_eq!(tab.label, "Tab 1");
    }

    #[test]
    fn tab_group_manager_add_and_count() {
        let mut mgr = WindowTabGroupManager::new();
        let g0 = mgr.add_group("Group 1");
        assert_eq!(mgr.group_count(), 1);
        assert_eq!(mgr.active_group_index(), Some(g0));
        mgr.add_tab(g0, TabEntry::new("t1", "File 1"));
        assert_eq!(mgr.tab_count(g0), 1);
        assert_eq!(mgr.active_tab_id(g0), Some("t1"));
    }

    #[test]
    fn tab_group_manager_remove_tab() {
        let mut mgr = WindowTabGroupManager::new();
        let g = mgr.add_group("G");
        mgr.add_tab(g, TabEntry::new("t1", "F1"));
        mgr.add_tab(g, TabEntry::new("t2", "F2"));
        mgr.set_active_tab(g, "t1");
        let removed = mgr.remove_tab(g, "t1").unwrap();
        assert_eq!(removed.id, "t1");
        assert_eq!(mgr.active_tab_id(g), Some("t2"));
    }

    #[test]
    fn tab_group_manager_sorted_tabs() {
        let mut mgr = WindowTabGroupManager::new();
        let g = mgr.add_group("G");
        mgr.add_tab(g, TabEntry::new("a", "A").with_order(2));
        mgr.add_tab(g, TabEntry::new("b", "B").with_pinned(true).with_order(3));
        mgr.add_tab(g, TabEntry::new("c", "C").with_order(1));
        let sorted = mgr.sorted_tabs(g);
        assert_eq!(sorted[0].id, "b"); // pinned first
        assert_eq!(sorted[1].id, "c"); // order 1
        assert_eq!(sorted[2].id, "a"); // order 2
    }

    #[test]
    fn tab_group_manager_dirty_count() {
        let mut mgr = WindowTabGroupManager::new();
        let g0 = mgr.add_group("G0");
        let g1 = mgr.add_group("G1");
        let mut t1 = TabEntry::new("t1", "F1");
        t1.is_dirty = true;
        let mut t2 = TabEntry::new("t2", "F2");
        t2.is_dirty = true;
        mgr.add_tab(g0, t1);
        mgr.add_tab(g1, t2);
        mgr.add_tab(g1, TabEntry::new("t3", "F3"));
        assert_eq!(mgr.dirty_tab_count(), 2);
    }



    #[test]
    fn extwin_builder_valid() {
        let cfg = ExtWinBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn extwin_builder_empty_name() {
        let r = ExtWinBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn extwin_builder_bad_priority() {
        assert!(ExtWinBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn extwin_builder_zero_max() {
        assert!(ExtWinBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn extwin_cfg_merge() {
        let mut a = ExtWinBuilder::new("a").property("x", "1").build().unwrap();
        let b = ExtWinBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn extwin_cfg_display() {
        let cfg = ExtWinBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }

    #[test]
    fn extwin_fmt_list() {
        let f = ExtWinFmt::new(ExtWinFmtOpts::default().with_indent(0));
        let r = f.format_list(&["a", "b", "c"]);
        assert!(r.contains("a") && r.contains("b") && r.contains("c"));
    }

    #[test]
    fn extwin_fmt_kv() {
        let f = ExtWinFmt::default_fmt();
        let r = f.format_kv("key", "value");
        assert!(r.contains("key") && r.contains("=") && r.contains("value"));
    }

    #[test]
    fn extwin_fmt_section() {
        let f = ExtWinFmt::new(ExtWinFmtOpts::default());
        let r = f.format_section("Hdr", &["line1".into(), "line2".into()]);
        assert!(r.starts_with("[Hdr]"));
        assert!(r.contains("line1"));
    }

    #[test]
    fn extwin_fmt_truncate() {
        let f = ExtWinFmt::new(ExtWinFmtOpts::default().with_max_width(10));
        let r = f.truncate("this is a very long string");
        assert!(r.ends_with("..."));
        assert!(r.len() <= 10);
    }

    #[test]
    fn extwin_fmt_opts_defaults() {
        let o = ExtWinFmtOpts::default();
        assert_eq!(o.indent, 2);
        assert_eq!(o.max_width, 120);
        assert!(!o.use_color);
    }


    #[test]
    fn ext_window_config_new() {
        let cfg = ExtWindowConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_window_config_set_get() {
        let mut cfg = ExtWindowConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_window_config_remove() {
        let mut cfg = ExtWindowConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_window_config_keys_sorted() {
        let mut cfg = ExtWindowConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_window_config_bump_version() {
        let mut cfg = ExtWindowConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_window_config_clear() {
        let mut cfg = ExtWindowConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_window_config_merge() {
        let mut cfg1 = ExtWindowConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtWindowConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_window_config_disable() {
        let mut cfg = ExtWindowConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_window_rate_tracker_empty() {
        let rt = ExtWindowRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_window_rate_tracker_record() {
        let mut rt = ExtWindowRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_window_rate_tracker_prune() {
        let mut rt = ExtWindowRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_window_validator_valid() {
        let v = ExtWindowValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_window_validator_errors() {
        let mut v = ExtWindowValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_window_validator_clear() {
        let mut v = ExtWindowValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_window_validator_merge() {
        let mut v1 = ExtWindowValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = ExtWindowValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_window_rate_tracker_clear() {
        let mut rt = ExtWindowRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn qw_metrics_empty() {
        let m = QwMetrics::new("ext_win");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qw_metrics_record_and_mean() {
        let mut m = QwMetrics::new("ext_win");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qw_metrics_min_max() {
        let mut m = QwMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qw_metrics_variance_and_std() {
        let mut m = QwMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn qw_metrics_percentile() {
        let mut m = QwMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn qw_metrics_merge() {
        let mut a = QwMetrics::new("a");
        a.record(1.0);
        let mut b = QwMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn qw_metrics_reset() {
        let mut m = QwMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn qw_rate_window_empty() {
        let rw = QwRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn qw_rate_window_tick_and_rate() {
        let mut rw = QwRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn qw_lru_cache_basic() {
        let mut c = QwLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn qw_lru_cache_contains_and_keys() {
        let mut c = QwLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn qw_lru_cache_remove() {
        let mut c = QwLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn qw_metrics_sum() {
        let mut m = QwMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn qw_metrics_label() {
        let m = QwMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn qw_lru_cache_clear() {
        let mut c = QwLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_12_push_and_len() {
        let mut rb = super::XbRingBuffer12::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_12_overwrite() {
        let mut rb = super::XbRingBuffer12::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_12_get_out_of_bounds() {
        let rb = super::XbRingBuffer12::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_12_drain_all() {
        let mut rb = super::XbRingBuffer12::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_12_peek_front_back() {
        let mut rb = super::XbRingBuffer12::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_12_clear() {
        let mut rb = super::XbRingBuffer12::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_12_capacity() {
        let rb = super::XbRingBuffer12::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_12_basic() {
        let h = super::xb_fnv1a_12(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_12(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_12_different_inputs() {
        let h1 = super::xb_fnv1a_12(b"abc");
        let h2 = super::xb_fnv1a_12(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_12_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_12(&data);
        let dec = super::xb_rle_decode_12(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_12_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_12(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_12(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_12_values() {
        assert!((super::xb_clamp_12(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_12(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_12(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_12_values() {
        assert!((super::xb_lerp_12(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_12(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_12(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_12_wrap_around_twice() {
        let mut rb = super::XbRingBuffer12::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 78 ----

    #[test]
    fn xc_78_pool_new_empty() {
        let pool: super::Xc78Pool<i32> = super::Xc78Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_78_pool_release_acquire() {
        let mut pool = super::Xc78Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_78_pool_acquire_empty() {
        let mut pool: super::Xc78Pool<i32> = super::Xc78Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_78_pool_full() {
        let mut pool = super::Xc78Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_78_pool_drain() {
        let mut pool = super::Xc78Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_78_pool_stats() {
        let mut pool = super::Xc78Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_78_pool_clear() {
        let mut pool = super::Xc78Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_78_pool_shrink() {
        let mut pool = super::Xc78Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_78_pool_default() {
        let pool: super::Xc78Pool<String> = super::Xc78Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_78_pool_extend() {
        let mut pool = super::Xc78Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_78_pool_retain() {
        let mut pool = super::Xc78Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_78_scheduler_round_robin() {
        let mut sched = super::Xc78Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_78_scheduler_empty() {
        let mut sched = super::Xc78Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_78_scheduler_reset() {
        let mut sched = super::Xc78Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_78_scheduler_add_remove() {
        let mut sched = super::Xc78Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_78_scheduler_targets() {
        let sched = super::Xc78Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_78_hash_empty() {
        assert_eq!(super::xc_78_hash(b""), 5381);
    }

    #[test]
    fn xc_78_hash_data() {
        let h = super::xc_78_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_78_hash(b"hello"), h);
    }

    #[test]
    fn xc_78_reverse_str() {
        assert_eq!(super::xc_78_reverse("abc"), "cba");
        assert_eq!(super::xc_78_reverse(""), "");
    }


    #[test]
    fn xe_24_pipeline_empty() {
        let p = super::Xe24Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_24_pipeline_parse_stage() {
        let p = super::Xe24Pipeline::new()
            .add_parse(super::xe_24_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_24_pipeline_transform_double() {
        let p = super::Xe24Pipeline::new()
            .add_transform(super::xe_24_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_24_pipeline_validate_reverse() {
        let p = super::Xe24Pipeline::new()
            .add_validate(super::xe_24_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_24_pipeline_emit_filter() {
        let p = super::Xe24Pipeline::new()
            .add_emit(super::xe_24_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_24_pipeline_multi_stage() {
        let p = super::Xe24Pipeline::new()
            .add_parse(super::xe_24_pipeline_identity)
            .add_transform(super::xe_24_pipeline_double)
            .add_validate(super::xe_24_pipeline_reverse)
            .add_emit(super::xe_24_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_24_pipeline_error_propagation() {
        let p = super::Xe24Pipeline::new()
            .add_parse(super::xe_24_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe24Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_24_pipeline_compose() {
        let p1 = super::Xe24Pipeline::new()
            .add_parse(super::xe_24_pipeline_identity);
        let p2 = super::Xe24Pipeline::new()
            .add_transform(super::xe_24_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_24_pipeline_error_display() {
        let e = super::Xe24PipelineError {
            stage: super::Xe24Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_24_cache_put_get() {
        let mut c = super::Xe24Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_24_cache_miss() {
        let mut c: super::Xe24Cache<&str, i32> = super::Xe24Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_24_cache_ttl_expiry() {
        let mut c = super::Xe24Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_24_cache_evict() {
        let mut c = super::Xe24Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_24_cache_capacity() {
        let mut c = super::Xe24Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_24_cache_stats() {
        let mut c = super::Xe24Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_24_cache_clear() {
        let mut c = super::Xe24Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #108 --

    #[test]
    fn xf108_trie_insert_search() {
        let mut t = Xf108Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf108_trie_starts_with() {
        let mut t = Xf108Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf108_trie_remove() {
        let mut t = Xf108Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf108_trie_word_count() {
        let mut t = Xf108Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf108_trie_longest_prefix() {
        let mut t = Xf108Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf108_trie_all_words() {
        let mut t = Xf108Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf108_trie_autocomplete() {
        let mut t = Xf108Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf108_trie_empty_search() {
        let t = Xf108Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf108_bloom_add_contains() {
        let mut bf = Xf108BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf108_bloom_probably_absent() {
        let bf = Xf108BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf108_bloom_false_positive_rate() {
        let mut bf = Xf108BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf108_bloom_clear() {
        let mut bf = Xf108BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf108_bloom_union() {
        let mut a = Xf108BloomFilter::xf_new(512, 2);
        let mut b = Xf108BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf108_bloom_intersection_estimate() {
        let mut a = Xf108BloomFilter::xf_new(512, 2);
        let mut b = Xf108BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf108_bloom_union_size_mismatch() {
        let a = Xf108BloomFilter::xf_new(256, 2);
        let b = Xf108BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh77_skip_insert_contains() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh77_skip_remove() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh77_skip_len() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh77_skip_range_query() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh77_skip_floor_ceiling() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh77_skip_rank() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh77_skip_empty() {
        let sl = super::Xh77SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh77_skip_duplicates() {
        let mut sl = super::Xh77SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh77_bitset_set_test() {
        let mut bs = super::Xh77BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh77_bitset_clear_count() {
        let mut bs = super::Xh77BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh77_bitset_and_or_xor() {
        let mut a = super::Xh77BitSet::xh_new(128);
        let mut b = super::Xh77BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh77_bitset_iter_ones() {
        let mut bs = super::Xh77BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh77_bitset_first_last() {
        let mut bs = super::Xh77BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh77_bitset_empty() {
        let bs = super::Xh77BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }

}