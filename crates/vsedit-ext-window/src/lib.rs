//! Ext API: Window.
//!
//! RPC bridge between the extension host and the main thread for window.

use std::collections::VecDeque;
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
    fn ext_window_validator_accepts_valid_name() {
        let v = ExtWindowValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_window_validator_rejects_empty() {
        let v = ExtWindowValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_window_validator_rejects_too_long() {
        let v = ExtWindowValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_window_validator_forbidden_prefix() {
        let v = ExtWindowValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_window_validator_allowed_chars() {
        let v = ExtWindowValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_window_validator_range() {
        let v = ExtWindowValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_window_sanitize_removes_control() {
        let result = ExtWindowValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_window_truncate_short_string() {
        assert_eq!(ExtWindowValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_window_truncate_long_string() {
        let result = ExtWindowValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_window_is_ascii_printable() {
        assert!(ExtWindowValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtWindowValidator::is_ascii_printable("Hello\x00World"));
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
}
