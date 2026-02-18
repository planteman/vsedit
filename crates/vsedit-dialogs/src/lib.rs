//! Dialog model service.
//!
//! Equivalent to VS Code's `vs/platform/dialogs/common/dialogs.ts`.
//! Provides data models for message dialogs, file pickers, and confirmation prompts.

use std::collections::HashMap;
use std::fmt;

/// Severity level for message dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A button in a dialog.
#[derive(Debug, Clone)]
pub struct DialogButton {
    pub label: String,
    pub is_secondary: bool,
}

/// Options for a message dialog.
#[derive(Debug, Clone)]
pub struct MessageDialogOptions {
    pub severity: Severity,
    pub message: String,
    pub detail: Option<String>,
    pub buttons: Vec<DialogButton>,
    pub cancel_button: Option<DialogButton>,
    pub checkbox_label: Option<String>,
    pub checkbox_checked: bool,
}

/// Result from a message dialog.
#[derive(Debug, Clone)]
pub struct MessageDialogResult {
    pub button_index: usize,
    pub checkbox_checked: bool,
}

/// Options for a file dialog.
#[derive(Debug, Clone)]
pub struct FileDialogOptions {
    pub title: Option<String>,
    pub default_path: Option<String>,
    pub can_select_files: bool,
    pub can_select_folders: bool,
    pub can_select_many: bool,
    pub filters: Vec<FileFilter>,
}

/// A file filter (e.g., "Rust Files" -> ["rs"]).
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileDialogOptions {
    pub fn open_file() -> Self {
        Self {
            title: Some("Open File".into()),
            default_path: None,
            can_select_files: true,
            can_select_folders: false,
            can_select_many: false,
            filters: Vec::new(),
        }
    }

    pub fn open_folder() -> Self {
        Self {
            title: Some("Open Folder".into()),
            default_path: None,
            can_select_files: false,
            can_select_folders: true,
            can_select_many: false,
            filters: Vec::new(),
        }
    }

    pub fn save_file() -> Self {
        Self {
            title: Some("Save File".into()),
            default_path: None,
            can_select_files: true,
            can_select_folders: false,
            can_select_many: false,
            filters: Vec::new(),
        }
    }
}

/// Options for a confirm dialog (simple yes/no).
#[derive(Debug, Clone)]
pub struct ConfirmDialogOptions {
    pub message: String,
    pub detail: Option<String>,
    pub primary_button: String,
    pub secondary_button: Option<String>,
    pub severity: Severity,
}

impl ConfirmDialogOptions {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
            primary_button: "OK".into(),
            secondary_button: Some("Cancel".into()),
            severity: Severity::Info,
        }
    }
}

/// Options for an input dialog.
#[derive(Debug, Clone)]
pub struct InputDialogOptions {
    pub prompt: String,
    pub value: Option<String>,
    pub placeholder: Option<String>,
    pub password: bool,
    pub validate_input: bool,
}

/// Dialog service trait.
pub trait IDialogService: Send + Sync {
    fn show_message(&self, options: MessageDialogOptions) -> MessageDialogResult;
    fn show_confirm(&self, options: ConfirmDialogOptions) -> bool;
    fn show_input(&self, options: InputDialogOptions) -> Option<String>;
}

impl FileDialogOptions {
    /// Add a file filter to the dialog options.
    pub fn with_filter(mut self, name: impl Into<String>, extensions: Vec<String>) -> Self {
        self.filters.push(FileFilter {
            name: name.into(),
            extensions,
        });
        self
    }

    /// Set the default path for the dialog.
    pub fn with_default_path(mut self, path: impl Into<String>) -> Self {
        self.default_path = Some(path.into());
        self
    }
}

impl ConfirmDialogOptions {
    /// Set the detail message for the confirm dialog.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set the severity level for the confirm dialog.
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}

/// Result from an input dialog.
#[derive(Debug, Clone)]
pub struct InputDialogResult {
    pub value: String,
    pub cancelled: bool,
}

impl InputDialogResult {
    /// Create a successful result with the given value.
    pub fn ok(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            cancelled: false,
        }
    }

    /// Create a cancelled result.
    pub fn cancelled() -> Self {
        Self {
            value: String::new(),
            cancelled: true,
        }
    }
}

impl InputDialogOptions {
    /// Create new input dialog options with the given prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            value: None,
            placeholder: None,
            password: false,
            validate_input: false,
        }
    }

    /// Set the initial value for the input dialog.
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set the placeholder text for the input dialog.
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Enable password mode for the input dialog.
    pub fn with_password(mut self) -> Self {
        self.password = true;
        self
    }
}

/// Options for a progress dialog.
#[derive(Debug, Clone)]
pub struct ProgressDialogOptions {
    pub title: String,
    pub message: Option<String>,
    pub cancellable: bool,
    pub total: Option<u64>,
}

impl ProgressDialogOptions {
    /// Create new progress dialog options with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: None,
            cancellable: false,
            total: None,
        }
    }

    /// Set the message for the progress dialog.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Make the progress dialog cancellable.
    pub fn with_cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }

    /// Set the total number of steps for the progress dialog.
    pub fn with_total(mut self, total: u64) -> Self {
        self.total = Some(total);
        self
    }
}

/// Tracks recently used file dialog paths.
#[derive(Debug, Clone, Default)]
pub struct DialogHistory {
    paths: Vec<String>,
}

impl DialogHistory {
    /// Create a new empty dialog history.
    pub fn new() -> Self {
        Self { paths: Vec::new() }
    }

    /// Record a path in the history.
    pub fn record_path(&mut self, path: impl Into<String>) {
        let p = path.into();
        self.paths.retain(|existing| existing != &p);
        self.paths.insert(0, p);
    }

    /// Get the most recent paths, up to `limit`.
    pub fn get_recent_paths(&self, limit: usize) -> &[String] {
        let end = limit.min(self.paths.len());
        &self.paths[..end]
    }

    /// Clear all recorded paths.
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Returns true if paths is empty.
    pub fn is_paths_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Get the first path, if any.
    pub fn first_path(&self) -> Option<&String> {
        self.paths.first()
    }

    /// Get the last path, if any.
    pub fn last_path(&self) -> Option<&String> {
        self.paths.last()
    }

    /// Retain only paths matching the predicate.
    pub fn retain_paths(&mut self, f: impl Fn(&String) -> bool) {
        self.paths.retain(|item| f(item));
    }
}

// ---------------------------------------------------------------------------
// File picker state (terminal UI overlay)
// ---------------------------------------------------------------------------

use std::path::PathBuf;

/// State for a terminal file-picker overlay.
pub struct FilePickerState {
    /// Current directory being browsed.
    pub current_dir: PathBuf,
    /// Listing of entries in the current directory.
    pub entries: Vec<FilePickerEntry>,
    /// Index of the highlighted entry.
    pub selected_index: usize,
    /// Search filter text typed by the user.
    pub filter: String,
    /// The dialog options controlling behaviour.
    pub options: FileDialogOptions,
    /// Paths chosen so far (for multi-select).
    pub chosen: Vec<PathBuf>,
}

/// A single entry in the file picker listing.
#[derive(Debug, Clone)]
pub struct FilePickerEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

impl FilePickerState {
    /// Create a new file picker starting at `start_dir`.
    pub fn new(start_dir: PathBuf, options: FileDialogOptions) -> Self {
        Self {
            current_dir: start_dir,
            entries: Vec::new(),
            selected_index: 0,
            filter: String::new(),
            options,
            chosen: Vec::new(),
        }
    }

    /// Returns entries filtered by the current search text and dialog filters.
    pub fn filtered_entries(&self) -> Vec<&FilePickerEntry> {
        let filter_lower = self.filter.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                if e.is_dir {
                    return true;
                }
                if !self.options.can_select_files {
                    return false;
                }
                if !filter_lower.is_empty() && !e.name.to_lowercase().contains(&filter_lower) {
                    return false;
                }
                if self.options.filters.is_empty() {
                    return true;
                }
                // Check extension against dialog filters
                let ext = e.path.extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                self.options.filters.iter().any(|f| {
                    f.extensions.iter().any(|fe| fe.eq_ignore_ascii_case(ext))
                })
            })
            .collect()
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        let max = self.filtered_entries().len().saturating_sub(1);
        if self.selected_index < max {
            self.selected_index += 1;
        }
    }

    /// Toggle the selected entry into/out of the chosen list (multi-select).
    pub fn toggle_selected(&mut self) {
        let filtered = self.filtered_entries();
        if let Some(entry) = filtered.get(self.selected_index) {
            let path = entry.path.clone();
            if let Some(pos) = self.chosen.iter().position(|p| *p == path) {
                self.chosen.remove(pos);
            } else {
                self.chosen.push(path);
            }
        }
    }

    /// Returns the selected path for single-select, or chosen paths for multi-select.
    pub fn result(&self) -> Vec<PathBuf> {
        if self.options.can_select_many && !self.chosen.is_empty() {
            return self.chosen.clone();
        }
        let filtered = self.filtered_entries();
        filtered
            .get(self.selected_index)
            .map(|e| vec![e.path.clone()])
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Message dialog state (centered overlay)
// ---------------------------------------------------------------------------

/// State for a centered message dialog overlay.
pub struct MessageDialogState {
    pub options: MessageDialogOptions,
    /// Index of the currently focused button.
    pub selected_button: usize,
}

impl MessageDialogState {
    pub fn new(options: MessageDialogOptions) -> Self {
        Self {
            options,
            selected_button: 0,
        }
    }

    /// Move focus to the next button.
    pub fn next_button(&mut self) {
        if !self.options.buttons.is_empty() {
            self.selected_button = (self.selected_button + 1) % self.options.buttons.len();
        }
    }

    /// Move focus to the previous button.
    pub fn prev_button(&mut self) {
        if !self.options.buttons.is_empty() {
            self.selected_button = if self.selected_button == 0 {
                self.options.buttons.len() - 1
            } else {
                self.selected_button - 1
            };
        }
    }

    /// Returns the result for the currently selected button.
    pub fn confirm(&self) -> MessageDialogResult {
        MessageDialogResult {
            button_index: self.selected_button,
            checkbox_checked: self.options.checkbox_checked,
        }
    }

    /// Compute the overlay rectangle dimensions (width, height) for rendering.
    pub fn overlay_size(&self) -> (u16, u16) {
        let msg_lines = self.options.message.lines().count() as u16;
        let detail_lines = self.options.detail.as_ref()
            .map(|d| d.lines().count() as u16)
            .unwrap_or(0);
        let height = 4 + msg_lines + detail_lines + 2; // border + title + msg + detail + buttons
        let max_text_width = self.options.message.lines()
            .map(|l| l.len())
            .max()
            .unwrap_or(20) as u16;
        let button_width: u16 = self.options.buttons.iter()
            .map(|b| b.label.len() as u16 + 4)
            .sum::<u16>() + self.options.buttons.len() as u16;
        let width = max_text_width.max(button_width).max(30) + 4;
        (width, height)
    }
}

// ---------------------------------------------------------------------------
// Input dialog state (centered overlay with text input)
// ---------------------------------------------------------------------------

/// State for a centered input dialog overlay.
pub struct InputDialogState {
    pub options: InputDialogOptions,
    /// Current text in the input field.
    pub input_text: String,
    /// Cursor position within the input field.
    pub cursor_pos: usize,
    /// Optional validation error message.
    pub error: Option<String>,
}

impl InputDialogState {
    pub fn new(options: InputDialogOptions) -> Self {
        let initial = options.value.clone().unwrap_or_default();
        let cursor = initial.len();
        Self {
            options,
            input_text: initial,
            cursor_pos: cursor,
            error: None,
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.input_text.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.input_text[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input_text.remove(prev);
            self.cursor_pos = prev;
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos = self.input_text[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor_pos < self.input_text.len() {
            self.cursor_pos += self.input_text[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
        }
    }

    /// Returns the submitted value if valid.
    pub fn submit(&self) -> Option<String> {
        if self.error.is_some() {
            None
        } else {
            Some(self.input_text.clone())
        }
    }

    /// Returns the display text (masked if password mode).
    pub fn display_text(&self) -> String {
        if self.options.password {
            "•".repeat(self.input_text.chars().count())
        } else {
            self.input_text.clone()
        }
    }
}

/// Convenience: show_open_file_dialog returns the file picker state.
pub fn show_open_file_dialog(options: FileDialogOptions) -> FilePickerState {
    let start = options.default_path.as_ref()
        .map(|p| PathBuf::from(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));
    FilePickerState::new(start, options)
}

/// Convenience: show_save_file_dialog returns the file picker state.
pub fn show_save_file_dialog(options: FileDialogOptions) -> FilePickerState {
    show_open_file_dialog(options)
}

/// Convenience: show_open_folder_dialog returns the file picker state.
pub fn show_open_folder_dialog(options: FileDialogOptions) -> FilePickerState {
    let mut opts = options;
    opts.can_select_files = false;
    opts.can_select_folders = true;
    show_open_file_dialog(opts)
}

/// Convenience: show_message_dialog returns the dialog state.
pub fn show_message_dialog(options: MessageDialogOptions) -> MessageDialogState {
    MessageDialogState::new(options)
}

/// Convenience: show_input_dialog returns the input dialog state.
pub fn show_input_dialog(options: InputDialogOptions) -> InputDialogState {
    InputDialogState::new(options)
}

/// Validate an input value against length constraints.
/// Returns `None` if valid, or `Some(error_message)` if invalid.
pub fn validate_input(value: &str, min_len: usize, max_len: usize) -> Option<String> {
    let len = value.len();
    if len < min_len {
        Some(format!(
            "Input too short: minimum {} characters required, got {}",
            min_len, len
        ))
    } else if len > max_len {
        Some(format!(
            "Input too long: maximum {} characters allowed, got {}",
            max_len, len
        ))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Confirm dialog with selectable options
// ---------------------------------------------------------------------------

/// A value associated with a confirm option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmValue {
    Yes,
    No,
    Cancel,
    Custom(String),
}

/// A single option in a confirm dialog.
#[derive(Debug, Clone)]
pub struct ConfirmOption {
    pub label: String,
    pub value: ConfirmValue,
}

/// State for a yes/no/cancel confirmation dialog.
pub struct ConfirmDialog {
    pub message: String,
    pub detail: Option<String>,
    pub severity: Severity,
    pub options: Vec<ConfirmOption>,
    pub selected: usize,
}

impl ConfirmDialog {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
            severity: Severity::Info,
            options: Vec::new(),
            selected: 0,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    pub fn yes_no(mut self) -> Self {
        self.options = vec![
            ConfirmOption { label: "Yes".into(), value: ConfirmValue::Yes },
            ConfirmOption { label: "No".into(), value: ConfirmValue::No },
        ];
        self
    }

    pub fn yes_no_cancel(mut self) -> Self {
        self.options = vec![
            ConfirmOption { label: "Yes".into(), value: ConfirmValue::Yes },
            ConfirmOption { label: "No".into(), value: ConfirmValue::No },
            ConfirmOption { label: "Cancel".into(), value: ConfirmValue::Cancel },
        ];
        self
    }

    pub fn add_option(mut self, label: impl Into<String>, value: ConfirmValue) -> Self {
        self.options.push(ConfirmOption {
            label: label.into(),
            value,
        });
        self
    }

    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected = (self.selected + 1) % self.options.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected = if self.selected == 0 {
                self.options.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn confirm(&self) -> &ConfirmOption {
        &self.options[self.selected]
    }

    pub fn option_count(&self) -> usize {
        self.options.len()
    }
}

// ---------------------------------------------------------------------------
// Input validator
// ---------------------------------------------------------------------------

/// Validates input dialog text against configurable constraints.
pub struct InputValidator {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub required: bool,
    pub forbidden_chars: Vec<char>,
}

impl InputValidator {
    pub fn new() -> Self {
        Self {
            min_length: None,
            max_length: None,
            pattern: None,
            required: false,
            forbidden_chars: Vec::new(),
        }
    }

    pub fn with_min_length(mut self, n: usize) -> Self {
        self.min_length = Some(n);
        self
    }

    pub fn with_max_length(mut self, n: usize) -> Self {
        self.max_length = Some(n);
        self
    }

    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn with_forbidden_chars(mut self, chars: Vec<char>) -> Self {
        self.forbidden_chars = chars;
        self
    }

    pub fn validate(&self, input: &str) -> Result<(), String> {
        if self.required && input.is_empty() {
            return Err("Input is required".into());
        }
        if let Some(min) = self.min_length {
            if input.len() < min {
                return Err(format!(
                    "Input too short: minimum {} characters required, got {}",
                    min,
                    input.len()
                ));
            }
        }
        if let Some(max) = self.max_length {
            if input.len() > max {
                return Err(format!(
                    "Input too long: maximum {} characters allowed, got {}",
                    max,
                    input.len()
                ));
            }
        }
        for c in &self.forbidden_chars {
            if input.contains(*c) {
                return Err(format!("Input contains forbidden character: '{}'", c));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Platform enum and button layout
// ---------------------------------------------------------------------------

/// Target platform, used for platform-specific dialog behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

/// Returns buttons in platform-appropriate order.
///
/// On macOS the primary/confirm button goes last (rightmost).
/// On Windows and Linux the primary button goes first (leftmost).
/// The first element of `buttons` is treated as the primary button.
pub fn dialog_button_layout(buttons: &[String], platform: Platform) -> Vec<String> {
    if buttons.is_empty() {
        return Vec::new();
    }
    match platform {
        Platform::MacOS => {
            let mut result: Vec<String> = buttons[1..].to_vec();
            result.push(buttons[0].clone());
            result
        }
        Platform::Windows | Platform::Linux => buttons.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// DialogQueue — queue dialogs to show sequentially
// ---------------------------------------------------------------------------

/// Represents a queued dialog to be shown.
#[derive(Debug, Clone)]
pub enum QueuedDialog {
    Message(MessageDialogOptions),
    Confirm(ConfirmDialogOptions),
    Input(InputDialogOptions),
}

/// Manages a queue of dialogs to show one at a time.
#[derive(Debug, Clone, Default)]
pub struct DialogQueue {
    queue: Vec<QueuedDialog>,
}

impl DialogQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, dialog: QueuedDialog) {
        self.queue.push(dialog);
    }

    /// Dequeue the next dialog to display.
    pub fn pop(&mut self) -> Option<QueuedDialog> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// Peek at the next dialog without removing it.
    pub fn peek(&self) -> Option<&QueuedDialog> {
        self.queue.first()
    }

    /// Count of each dialog type in the queue.
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut msg = 0;
        let mut confirm = 0;
        let mut input = 0;
        for d in &self.queue {
            match d {
                QueuedDialog::Message(_) => msg += 1,
                QueuedDialog::Confirm(_) => confirm += 1,
                QueuedDialog::Input(_) => input += 1,
            }
        }
        (msg, confirm, input)
    }
}

// ---------------------------------------------------------------------------
// DialogPreset — predefined dialog configurations
// ---------------------------------------------------------------------------

/// Common preset dialog configurations.
pub struct DialogPreset;

impl DialogPreset {
    /// "Are you sure?" confirmation dialog.
    pub fn confirm_delete(item_name: &str) -> ConfirmDialogOptions {
        ConfirmDialogOptions {
            message: format!("Delete \"{}\"?", item_name),
            detail: Some("This action cannot be undone.".into()),
            primary_button: "Delete".into(),
            secondary_button: Some("Cancel".into()),
            severity: Severity::Warning,
        }
    }

    /// Unsaved changes confirmation.
    pub fn unsaved_changes(file_name: &str) -> ConfirmDialogOptions {
        ConfirmDialogOptions {
            message: format!("Do you want to save changes to \"{}\"?", file_name),
            detail: Some("Your changes will be lost if you don't save them.".into()),
            primary_button: "Save".into(),
            secondary_button: Some("Don't Save".into()),
            severity: Severity::Warning,
        }
    }

    /// Error message dialog.
    pub fn error(message: impl Into<String>) -> MessageDialogOptions {
        MessageDialogOptions {
            severity: Severity::Error,
            message: message.into(),
            detail: None,
            buttons: vec![DialogButton { label: "OK".into(), is_secondary: false }],
            cancel_button: None,
            checkbox_label: None,
            checkbox_checked: false,
        }
    }

    /// Information message dialog.
    pub fn info(message: impl Into<String>) -> MessageDialogOptions {
        MessageDialogOptions {
            severity: Severity::Info,
            message: message.into(),
            detail: None,
            buttons: vec![DialogButton { label: "OK".into(), is_secondary: false }],
            cancel_button: None,
            checkbox_label: None,
            checkbox_checked: false,
        }
    }

    /// Simple text input dialog.
    pub fn text_input(prompt: impl Into<String>, placeholder: impl Into<String>) -> InputDialogOptions {
        InputDialogOptions {
            prompt: prompt.into(),
            value: None,
            placeholder: Some(placeholder.into()),
            password: false,
            validate_input: false,
        }
    }

    /// Rename item input dialog.
    pub fn rename(current_name: &str) -> InputDialogOptions {
        InputDialogOptions {
            prompt: "Enter new name".into(),
            value: Some(current_name.into()),
            placeholder: None,
            password: false,
            validate_input: true,
        }
    }
}

// ---------------------------------------------------------------------------
// DialogAccessibility — accessibility annotations
// ---------------------------------------------------------------------------

/// ARIA-like role for a dialog element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaRole {
    Dialog,
    AlertDialog,
    Form,
}

/// Accessibility annotations for dialog elements.
#[derive(Debug, Clone)]
pub struct DialogAccessibility {
    pub role: AriaRole,
    pub label: String,
    pub description: Option<String>,
    pub live_region: bool,
}

impl DialogAccessibility {
    pub fn new(role: AriaRole, label: impl Into<String>) -> Self {
        Self {
            role,
            label: label.into(),
            description: None,
            live_region: false,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_live_region(mut self) -> Self {
        self.live_region = true;
        self
    }

    /// Generate accessibility attributes for the dialog.
    pub fn for_message(severity: Severity, message: &str) -> Self {
        let role = match severity {
            Severity::Error | Severity::Warning => AriaRole::AlertDialog,
            Severity::Info => AriaRole::Dialog,
        };
        Self::new(role, message)
            .with_live_region()
    }

    pub fn for_input(prompt: &str) -> Self {
        Self::new(AriaRole::Form, prompt)
    }

    pub fn for_confirm(message: &str) -> Self {
        Self::new(AriaRole::AlertDialog, message)
            .with_live_region()
    }

    /// Summary text for screen readers.
    pub fn announce_text(&self) -> String {
        match &self.description {
            Some(desc) => format!("{}: {}", self.label, desc),
            None => self.label.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// InputValidator — pattern-based validation
// ---------------------------------------------------------------------------

impl InputValidator {
    /// Set a pattern (simple substring match) for validation.
    pub fn with_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = Some(pattern.into());
        self
    }

    /// Validate against the pattern if set.
    pub fn validate_pattern(&self, input: &str) -> Result<(), String> {
        if let Some(ref pat) = self.pattern {
            if !input.contains(pat.as_str()) {
                return Err(format!("Input must match pattern: {}", pat));
            }
        }
        Ok(())
    }

    /// Full validation including pattern.
    pub fn validate_all(&self, input: &str) -> Result<(), String> {
        self.validate(input)?;
        self.validate_pattern(input)?;
        Ok(())
    }

    /// Check if the input would be valid (boolean convenience).
    pub fn is_valid(&self, input: &str) -> bool {
        self.validate_all(input).is_ok()
    }
}

// ---------------------------------------------------------------------------
// Additional impl blocks — utility helpers, predicates, conversions
// ---------------------------------------------------------------------------

impl Severity {
    /// Returns `true` for `Error` or `Warning`.
    pub fn is_actionable(&self) -> bool {
        matches!(self, Severity::Error | Severity::Warning)
    }

    /// Returns a short human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Warning => "Warning",
            Severity::Error => "Error",
        }
    }

    /// Numeric level: Error=2, Warning=1, Info=0.
    pub fn level(&self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Error => 2,
        }
    }

    /// Returns `true` if `self` is at least as severe as `other`.
    pub fn at_least(&self, other: Severity) -> bool {
        self.level() >= other.level()
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl DialogButton {
    /// Create a primary button with the given label.
    pub fn primary(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_secondary: false,
        }
    }

    /// Create a secondary button with the given label.
    pub fn secondary(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_secondary: true,
        }
    }
}

impl FileFilter {
    /// Returns `true` if the given extension (without the dot) matches this filter.
    pub fn matches_extension(&self, ext: &str) -> bool {
        self.extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    }

    /// Returns `true` if this filter has no extensions (matches everything).
    pub fn is_wildcard(&self) -> bool {
        self.extensions.is_empty()
            || self.extensions.iter().any(|e| e == "*")
    }
}

impl FileDialogOptions {
    /// Returns `true` if neither files nor folders can be selected.
    pub fn is_noop(&self) -> bool {
        !self.can_select_files && !self.can_select_folders
    }

    /// Enable multi-select.
    pub fn with_multi_select(mut self) -> Self {
        self.can_select_many = true;
        self
    }

    /// Set the dialog title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Returns all unique extensions across every filter.
    pub fn all_extensions(&self) -> Vec<&str> {
        let mut exts: Vec<&str> = self
            .filters
            .iter()
            .flat_map(|f| f.extensions.iter().map(|e| e.as_str()))
            .collect();
        exts.sort_unstable();
        exts.dedup();
        exts
    }
}

impl MessageDialogOptions {
    /// Returns `true` if the dialog has a checkbox.
    pub fn has_checkbox(&self) -> bool {
        self.checkbox_label.is_some()
    }

    /// Total number of interactive buttons (including cancel if present).
    pub fn total_button_count(&self) -> usize {
        self.buttons.len() + if self.cancel_button.is_some() { 1 } else { 0 }
    }

    /// Builder: add a detail message.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl ConfirmValue {
    /// Returns `true` for `Yes` or any `Custom` value.
    pub fn is_affirmative(&self) -> bool {
        matches!(self, ConfirmValue::Yes | ConfirmValue::Custom(_))
    }

    /// Returns `true` for `Cancel`.
    pub fn is_cancel(&self) -> bool {
        *self == ConfirmValue::Cancel
    }
}

impl std::fmt::Display for ConfirmValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfirmValue::Yes => f.write_str("Yes"),
            ConfirmValue::No => f.write_str("No"),
            ConfirmValue::Cancel => f.write_str("Cancel"),
            ConfirmValue::Custom(s) => f.write_str(s),
        }
    }
}

impl FilePickerEntry {
    /// Human-readable size string (e.g. "1.2 KB").
    pub fn human_size(&self) -> String {
        if self.is_dir {
            return String::new();
        }
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        if self.size >= GB {
            format!("{:.1} GB", self.size as f64 / GB as f64)
        } else if self.size >= MB {
            format!("{:.1} MB", self.size as f64 / MB as f64)
        } else if self.size >= KB {
            format!("{:.1} KB", self.size as f64 / KB as f64)
        } else {
            format!("{} B", self.size)
        }
    }

    /// Returns the file extension, if any.
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|s| s.to_str())
    }
}

impl FilePickerState {
    /// Returns `true` if the filter text is non-empty.
    pub fn has_filter(&self) -> bool {
        !self.filter.is_empty()
    }

    /// Clear the filter text and reset the selection index.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.selected_index = 0;
    }

    /// Number of currently chosen paths.
    pub fn chosen_count(&self) -> usize {
        self.chosen.len()
    }

    /// Returns `true` if the given path has been chosen.
    pub fn is_chosen(&self, path: &std::path::Path) -> bool {
        self.chosen.iter().any(|p| p == path)
    }
}

impl DialogHistory {
    /// Returns the total number of recorded paths.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns `true` if `path` is already recorded.
    pub fn contains(&self, path: &str) -> bool {
        self.paths.iter().any(|p| p == path)
    }
}

impl ProgressDialogOptions {
    /// Returns `true` if the total number of steps is known.
    pub fn is_determinate(&self) -> bool {
        self.total.is_some()
    }

    /// Compute progress as a fraction in `[0.0, 1.0]`, or `None` if indeterminate.
    pub fn fraction(&self, current: u64) -> Option<f64> {
        self.total.map(|t| {
            if t == 0 {
                1.0
            } else {
                (current as f64 / t as f64).min(1.0)
            }
        })
    }
}

impl InputDialogState {
    /// Delete the character at the cursor position (forward delete).
    pub fn delete_forward(&mut self) {
        if self.cursor_pos < self.input_text.len() {
            let char_len = self.input_text[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.input_text
                .drain(self.cursor_pos..self.cursor_pos + char_len);
        }
    }

    /// Move the cursor to the beginning.
    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move the cursor to the end.
    pub fn move_end(&mut self) {
        self.cursor_pos = self.input_text.len();
    }

    /// Clear all text and reset the cursor.
    pub fn clear(&mut self) {
        self.input_text.clear();
        self.cursor_pos = 0;
        self.error = None;
    }

    /// Returns `true` if the input field is empty.
    pub fn is_empty(&self) -> bool {
        self.input_text.is_empty()
    }

    /// Character count (not byte count).
    pub fn char_count(&self) -> usize {
        self.input_text.chars().count()
    }
}

impl ConfirmDialog {
    /// Returns `true` if the currently selected option is affirmative.
    pub fn is_affirmative(&self) -> bool {
        self.options
            .get(self.selected)
            .map_or(false, |o| o.value.is_affirmative())
    }

    /// Find the index of the option with the given value, if any.
    pub fn index_of(&self, value: &ConfirmValue) -> Option<usize> {
        self.options.iter().position(|o| &o.value == value)
    }

    /// Select the option with the given value, returning `true` if found.
    pub fn select_value(&mut self, value: &ConfirmValue) -> bool {
        if let Some(idx) = self.index_of(value) {
            self.selected = idx;
            true
        } else {
            false
        }
    }
}

impl Default for InputValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl DialogQueue {
    /// Drain all queued dialogs into a `Vec`.
    pub fn drain_all(&mut self) -> Vec<QueuedDialog> {
        std::mem::take(&mut self.queue)
    }

    /// Returns `true` if any queued dialog has the given severity.
    pub fn has_severity(&self, severity: Severity) -> bool {
        self.queue.iter().any(|d| match d {
            QueuedDialog::Message(m) => m.severity == severity,
            QueuedDialog::Confirm(c) => c.severity == severity,
            QueuedDialog::Input(_) => false,
        })
    }
}


// ---------------------------------------------------------------------------
// DialogHistoryService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DialogHistoryService {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl DialogHistoryService {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for DialogHistoryService {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for DialogHistoryService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DialogHistoryService({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// DialogAccessNarrator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DialogAccessNarrator {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl DialogAccessNarrator {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for DialogAccessNarrator {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for DialogAccessNarrator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "DialogAccessNarrator({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// DialogHistoryServiceSnapshot — point-in-time snapshot of DialogHistoryService state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DialogHistoryServiceSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl DialogHistoryServiceSnapshot {
    pub fn capture(source: &DialogHistoryService, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for DialogHistoryServiceSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// DialogAccessNarratorStats — aggregate statistics for DialogAccessNarrator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct DialogAccessNarratorStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl DialogAccessNarratorStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for DialogAccessNarratorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// DialogHistoryServiceConfig — configuration for DialogHistoryService
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DialogHistoryServiceConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl DialogHistoryServiceConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for DialogHistoryServiceConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for DialogHistoryServiceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// DialogPriorityQueue — queue with priority ordering
// ---------------------------------------------------------------------------

/// Priority level for queued dialogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DialogPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// A dialog entry with priority for ordered dequeuing.
#[derive(Debug, Clone)]
pub struct PrioritizedDialog {
    pub priority: DialogPriority,
    pub title: String,
    pub message: String,
    pub severity: Severity,
    pub enqueued_at: u64,
}

impl PrioritizedDialog {
    pub fn new(priority: DialogPriority, title: impl Into<String>, message: impl Into<String>, severity: Severity) -> Self {
        Self {
            priority,
            title: title.into(),
            message: message.into(),
            severity,
            enqueued_at: 0,
        }
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self { self.enqueued_at = ts; self }
}

/// A priority-based dialog queue that dequeues highest priority first.
#[derive(Debug, Clone)]
pub struct DialogPriorityQueue {
    entries: Vec<PrioritizedDialog>,
}

impl DialogPriorityQueue {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn enqueue(&mut self, dialog: PrioritizedDialog) {
        self.entries.push(dialog);
    }

    /// Dequeue the highest-priority dialog (FIFO within same priority).
    pub fn dequeue(&mut self) -> Option<PrioritizedDialog> {
        if self.entries.is_empty() { return None; }
        let mut best_idx = 0;
        for (i, entry) in self.entries.iter().enumerate() {
            if entry.priority > self.entries[best_idx].priority {
                best_idx = i;
            }
        }
        Some(self.entries.remove(best_idx))
    }

    pub fn peek(&self) -> Option<&PrioritizedDialog> {
        self.entries.iter().max_by_key(|e| e.priority)
    }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn len(&self) -> usize { self.entries.len() }

    /// Count of dialogs at each priority level.
    pub fn priority_counts(&self) -> (usize, usize, usize, usize) {
        let mut low = 0; let mut normal = 0; let mut high = 0; let mut critical = 0;
        for e in &self.entries {
            match e.priority {
                DialogPriority::Low => low += 1,
                DialogPriority::Normal => normal += 1,
                DialogPriority::High => high += 1,
                DialogPriority::Critical => critical += 1,
            }
        }
        (low, normal, high, critical)
    }
}

impl Default for DialogPriorityQueue {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// DialogValidationChain — composable validation rules
// ---------------------------------------------------------------------------

/// A single validation rule.
#[derive(Debug, Clone)]
pub struct DialogValidationRule {
    pub name: String,
    pub message: String,
    validator: DialogValidationFn,
}

#[derive(Clone)]
enum DialogValidationFn {
    NonEmpty,
    MinLength(usize),
    MaxLength(usize),
    NoForbiddenChars(Vec<char>),
}

impl fmt::Debug for DialogValidationFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonEmpty => write!(f, "NonEmpty"),
            Self::MinLength(n) => write!(f, "MinLength({n})"),
            Self::MaxLength(n) => write!(f, "MaxLength({n})"),
            Self::NoForbiddenChars(cs) => write!(f, "NoForbiddenChars({cs:?})"),
        }
    }
}

impl DialogValidationRule {
    fn check(&self, input: &str) -> bool {
        match &self.validator {
            DialogValidationFn::NonEmpty => !input.is_empty(),
            DialogValidationFn::MinLength(n) => input.len() >= *n,
            DialogValidationFn::MaxLength(n) => input.len() <= *n,
            DialogValidationFn::NoForbiddenChars(cs) => !cs.iter().any(|c| input.contains(*c)),
        }
    }
}

/// A chain of validation rules applied in order.
#[derive(Debug, Clone)]
pub struct DialogValidationChain {
    rules: Vec<DialogValidationRule>,
}

impl DialogValidationChain {
    pub fn new() -> Self { Self { rules: Vec::new() } }

    pub fn non_empty(mut self) -> Self {
        self.rules.push(DialogValidationRule {
            name: "non_empty".into(),
            message: "Input must not be empty".into(),
            validator: DialogValidationFn::NonEmpty,
        });
        self
    }

    pub fn min_length(mut self, n: usize) -> Self {
        self.rules.push(DialogValidationRule {
            name: "min_length".into(),
            message: format!("Input must be at least {n} characters"),
            validator: DialogValidationFn::MinLength(n),
        });
        self
    }

    pub fn max_length(mut self, n: usize) -> Self {
        self.rules.push(DialogValidationRule {
            name: "max_length".into(),
            message: format!("Input must be at most {n} characters"),
            validator: DialogValidationFn::MaxLength(n),
        });
        self
    }

    pub fn no_chars(mut self, chars: Vec<char>) -> Self {
        self.rules.push(DialogValidationRule {
            name: "no_chars".into(),
            message: format!("Input must not contain: {:?}", chars),
            validator: DialogValidationFn::NoForbiddenChars(chars),
        });
        self
    }

    /// Validate input against all rules. Returns first error message, or Ok.
    pub fn validate(&self, input: &str) -> Result<(), String> {
        for rule in &self.rules {
            if !rule.check(input) {
                return Err(rule.message.clone());
            }
        }
        Ok(())
    }

    /// Returns all failing rule names.
    pub fn failing_rules(&self, input: &str) -> Vec<&str> {
        self.rules.iter()
            .filter(|r| !r.check(input))
            .map(|r| r.name.as_str())
            .collect()
    }

    pub fn rule_count(&self) -> usize { self.rules.len() }
}

impl Default for DialogValidationChain {
    fn default() -> Self { Self::new() }
}


/// Configuration manager for dialogs functionality.
pub struct DialogsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl DialogsConfig {
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

    pub fn merge(&mut self, other: &DialogsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for dialogs operations.
pub struct DialogsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl DialogsRateTracker {
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

/// Validation result collector for dialogs.
pub struct DialogsValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl DialogsValidator {
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

    pub fn merge(&mut self, other: &DialogsValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Modal dialog management — extended utilities (xh)
// ---------------------------------------------------------------------------

/// Metric accumulator for dialogs operations.
#[derive(Debug, Clone)]
pub struct XhMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XhMetrics {
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

/// Sliding-window rate counter for dialogs.
#[derive(Debug, Clone)]
pub struct XhRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XhRateWindow {
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

/// A small LRU-style cache for dialogs lookups.
#[derive(Debug, Clone)]
pub struct XhLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XhLruCache {
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
// xb_ utilities – batch 25
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer25 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer25 {
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
pub fn xb_fnv1a_25(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_25<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_25<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_25(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_25(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 29
// ---------------------------------------------------------------------------

/// Generic object pool `Xc29Pool<T>`.
pub struct Xc29Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc29Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc29PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc29Pool<T> {
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
    pub fn stats(&self) -> Xc29PoolStats {
        Xc29PoolStats {
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

impl<T> Default for Xc29Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc29Scheduler`.
pub struct Xc29Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc29Scheduler {
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

impl Default for Xc29Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_29 hash for the given byte slice.
pub fn xc_29_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_29 convention.
pub fn xc_29_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe37 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe37Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe37PipelineError {
    pub stage: Xe37Stage,
    pub message: String,
}

impl std::fmt::Display for Xe37PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe37Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe37Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError>>>,
    stage_names: Vec<Xe37Stage>,
}

impl Xe37Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe37Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe37Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe37Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe37Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> {
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

    pub fn compose(mut self, other: Xe37Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe37CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe37CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe37Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe37CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe37CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe37Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe37CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_37_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe37CacheEntry {
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

    fn xe_37_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe37CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_37_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> {
    Ok(data)
}

pub fn xe_37_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_37_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_37_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_37_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe37PipelineError> {
    Err(Xe37PipelineError {
        stage: Xe37Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_3: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg3Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg3Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg3Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_3: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg3Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg3Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg3Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg3Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 28).
pub struct Xh28SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh28SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 70 as u64,
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

/// A compact bit set supporting boolean operations (variant 28).
pub struct Xh28BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh28BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 28).
pub struct Xi28Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi28Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi28Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi28Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 28).
pub struct Xi28IntervalTree {
    xi_intervals: Vec<Xi28Interval>,
}

impl Xi28IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi28Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi28Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi28Interval) -> Vec<&Xi28Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi28Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi28Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi28Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi28Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi28Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi28Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 27) ---

/// Disjoint set / union-find for crate 27.
pub struct Xj27UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj27UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ27_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 27.
pub struct Xj27BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj27BTreeNode<K, V>>>,
    len: usize,
}

struct Xj27BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj27BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj27BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ27_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ27_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj27BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj27BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj27BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj27BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_28 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk28SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk28SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk28DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk28DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_27).
#[derive(Debug, Clone)]
pub struct Xl27Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl27Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_27).
#[derive(Debug, Clone)]
pub struct Xl27SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl27SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm27MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm27MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm27Tokenizer {
    text: String,
}

impl Xm27Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 28.
pub struct Xn28Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn28Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 28 -----

#[derive(Debug, Clone)]
struct Xn28AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn28AvlNode<K, V>>>,
    right: Option<Box<Xn28AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 28.
#[derive(Debug, Clone)]
pub struct Xn28AVL<K, V> {
    root: Option<Box<Xn28AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn28AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn28AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn28AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn28AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn28AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn28AvlNode<K, V>>) -> Box<Xn28AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn28AvlNode<K, V>>) -> Box<Xn28AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn28AvlNode<K, V>>) -> Box<Xn28AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn28AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn28AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn28AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn28AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn28AvlNode<K, V>>) -> &Xn28AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn28AvlNode<K, V>>) -> (Box<Xn28AvlNode<K, V>>, Option<Box<Xn28AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn28AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn28AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn28AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn28AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn28AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn28AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn28AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo28RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo28Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo28RBNode<K, V> {
    key: K,
    value: V,
    color: Xo28Color,
    left: Option<Box<Xo28RBNode<K, V>>>,
    right: Option<Box<Xo28RBNode<K, V>>>,
}

/// A red-black tree map for crate 28.
#[derive(Debug, Clone)]
pub struct Xo28RedBlack<K, V> {
    root: Option<Box<Xo28RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo28RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo28Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo28RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo28RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo28RBNode {
                    key, value, color: Xo28Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo28RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo28Color::Red)
    }

    fn xo_balance(mut h: Box<Xo28RBNode<K, V>>) -> Box<Xo28RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo28Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo28RBNode<K, V>>) -> Box<Xo28RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo28Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo28RBNode<K, V>>) -> Box<Xo28RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo28Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo28RBNode<K, V>>) {
        h.color = Xo28Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo28Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo28Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo28Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo28RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo28RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo28RBNode<K, V>) -> (K, V, Option<Box<Xo28RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo28RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo28Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo28RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo28ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 28.
#[derive(Debug, Clone)]
pub struct Xo28ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo28ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo28#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo28#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_dialog_open_file_defaults() {
        let opts = FileDialogOptions::open_file();
        assert!(opts.can_select_files);
        assert!(!opts.can_select_folders);
        assert!(!opts.can_select_many);
    }

    #[test]
    fn file_dialog_open_folder_defaults() {
        let opts = FileDialogOptions::open_folder();
        assert!(!opts.can_select_files);
        assert!(opts.can_select_folders);
    }

    #[test]
    fn confirm_dialog_defaults() {
        let opts = ConfirmDialogOptions::new("Delete file?");
        assert_eq!(opts.message, "Delete file?");
        assert_eq!(opts.primary_button, "OK");
        assert_eq!(opts.severity, Severity::Info);
    }

    #[test]
    fn dialog_button_creation() {
        let btn = DialogButton {
            label: "Yes".into(),
            is_secondary: false,
        };
        assert_eq!(btn.label, "Yes");
        assert!(!btn.is_secondary);
    }

    #[test]
    fn file_filter_creation() {
        let filter = FileFilter {
            name: "Rust Files".into(),
            extensions: vec!["rs".into()],
        };
        assert_eq!(filter.name, "Rust Files");
        assert_eq!(filter.extensions, vec!["rs"]);
    }

    #[test]
    fn file_dialog_with_filter() {
        let opts = FileDialogOptions::open_file()
            .with_filter("Images", vec!["png".into(), "jpg".into()])
            .with_default_path("/home/user");
        assert_eq!(opts.filters.len(), 1);
        assert_eq!(opts.filters[0].name, "Images");
        assert_eq!(opts.default_path, Some("/home/user".into()));
    }

    #[test]
    fn confirm_dialog_builders() {
        let opts = ConfirmDialogOptions::new("Save?")
            .with_detail("Unsaved changes will be lost")
            .with_severity(Severity::Warning);
        assert_eq!(opts.detail, Some("Unsaved changes will be lost".into()));
        assert_eq!(opts.severity, Severity::Warning);
    }

    #[test]
    fn input_dialog_result() {
        let ok = InputDialogResult::ok("hello");
        assert_eq!(ok.value, "hello");
        assert!(!ok.cancelled);
        let cancel = InputDialogResult::cancelled();
        assert!(cancel.cancelled);
        assert!(cancel.value.is_empty());
    }

    #[test]
    fn input_dialog_options_builders() {
        let opts = InputDialogOptions::new("Enter name:")
            .with_value("default")
            .with_placeholder("Type here...")
            .with_password();
        assert_eq!(opts.prompt, "Enter name:");
        assert_eq!(opts.value, Some("default".into()));
        assert_eq!(opts.placeholder, Some("Type here...".into()));
        assert!(opts.password);
    }

    #[test]
    fn progress_dialog_options() {
        let opts = ProgressDialogOptions::new("Loading...")
            .with_message("Processing files")
            .with_cancellable()
            .with_total(100);
        assert_eq!(opts.title, "Loading...");
        assert_eq!(opts.message, Some("Processing files".into()));
        assert!(opts.cancellable);
        assert_eq!(opts.total, Some(100));
    }

    #[test]
    fn dialog_history_tracks_paths() {
        let mut history = DialogHistory::new();
        history.record_path("/path/a");
        history.record_path("/path/b");
        history.record_path("/path/a"); // duplicate moves to front
        let recent = history.get_recent_paths(5);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0], "/path/a");
        assert_eq!(recent[1], "/path/b");
    }

    #[test]
    fn dialog_history_clear() {
        let mut history = DialogHistory::new();
        history.record_path("/path/a");
        history.clear();
        assert_eq!(history.get_recent_paths(10).len(), 0);
    }

    #[test]
    fn validate_input_valid() {
        assert!(validate_input("hello", 1, 10).is_none());
    }

    #[test]
    fn validate_input_too_short() {
        let err = validate_input("", 1, 10);
        assert!(err.is_some());
        assert!(err.unwrap().contains("too short"));
    }

    #[test]
    fn validate_input_too_long() {
        let err = validate_input("a very long string", 1, 5);
        assert!(err.is_some());
        assert!(err.unwrap().contains("too long"));
    }

    #[test]
    fn eq_severity_same() {
        assert_eq!(Severity::Info, Severity::Info);
    }

    #[test]
    fn ne_severity_diff() {
        assert_ne!(Severity::Info, Severity::Warning);
    }

    // --- FilePickerState tests ---

    #[test]
    fn file_picker_new() {
        let opts = FileDialogOptions::open_file();
        let state = FilePickerState::new(PathBuf::from("/tmp"), opts);
        assert_eq!(state.current_dir, PathBuf::from("/tmp"));
        assert_eq!(state.selected_index, 0);
        assert!(state.filter.is_empty());
        assert!(state.chosen.is_empty());
    }

    #[test]
    fn file_picker_move_up_at_zero() {
        let opts = FileDialogOptions::open_file();
        let mut state = FilePickerState::new(PathBuf::from("/tmp"), opts);
        state.move_up();
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn file_picker_toggle_selected_empty() {
        let opts = FileDialogOptions::open_file();
        let mut state = FilePickerState::new(PathBuf::from("/tmp"), opts);
        state.toggle_selected(); // no entries, should not panic
        assert!(state.chosen.is_empty());
    }

    #[test]
    fn file_picker_filtered_entries_dirs_always_shown() {
        let opts = FileDialogOptions::open_file()
            .with_filter("Rust", vec!["rs".into()]);
        let mut state = FilePickerState::new(PathBuf::from("/tmp"), opts);
        state.entries.push(FilePickerEntry {
            name: "src".into(),
            path: PathBuf::from("/tmp/src"),
            is_dir: true,
            size: 0,
        });
        state.entries.push(FilePickerEntry {
            name: "main.rs".into(),
            path: PathBuf::from("/tmp/main.rs"),
            is_dir: false,
            size: 100,
        });
        state.entries.push(FilePickerEntry {
            name: "readme.md".into(),
            path: PathBuf::from("/tmp/readme.md"),
            is_dir: false,
            size: 200,
        });
        let filtered = state.filtered_entries();
        // Should include dir and .rs file, not .md
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "src");
        assert_eq!(filtered[1].name, "main.rs");
    }

    #[test]
    fn file_picker_result_single() {
        let opts = FileDialogOptions::open_file();
        let mut state = FilePickerState::new(PathBuf::from("/tmp"), opts);
        state.entries.push(FilePickerEntry {
            name: "a.txt".into(), path: PathBuf::from("/tmp/a.txt"), is_dir: false, size: 10,
        });
        state.entries.push(FilePickerEntry {
            name: "b.txt".into(), path: PathBuf::from("/tmp/b.txt"), is_dir: false, size: 20,
        });
        state.selected_index = 1;
        let result = state.result();
        assert_eq!(result, vec![PathBuf::from("/tmp/b.txt")]);
    }

    // --- MessageDialogState tests ---

    #[test]
    fn message_dialog_state_buttons() {
        let opts = MessageDialogOptions {
            severity: Severity::Info,
            message: "Test".into(),
            detail: None,
            buttons: vec![
                DialogButton { label: "OK".into(), is_secondary: false },
                DialogButton { label: "Cancel".into(), is_secondary: true },
            ],
            cancel_button: None,
            checkbox_label: None,
            checkbox_checked: false,
        };
        let mut state = MessageDialogState::new(opts);
        assert_eq!(state.selected_button, 0);
        state.next_button();
        assert_eq!(state.selected_button, 1);
        state.next_button();
        assert_eq!(state.selected_button, 0); // wraps
        state.prev_button();
        assert_eq!(state.selected_button, 1); // wraps back
    }

    #[test]
    fn message_dialog_confirm() {
        let opts = MessageDialogOptions {
            severity: Severity::Warning,
            message: "Delete?".into(),
            detail: Some("Cannot undo".into()),
            buttons: vec![
                DialogButton { label: "Yes".into(), is_secondary: false },
                DialogButton { label: "No".into(), is_secondary: true },
            ],
            cancel_button: None,
            checkbox_label: None,
            checkbox_checked: false,
        };
        let state = MessageDialogState::new(opts);
        let result = state.confirm();
        assert_eq!(result.button_index, 0);
    }

    #[test]
    fn message_dialog_overlay_size() {
        let opts = MessageDialogOptions {
            severity: Severity::Info,
            message: "A short message".into(),
            detail: None,
            buttons: vec![
                DialogButton { label: "OK".into(), is_secondary: false },
            ],
            cancel_button: None,
            checkbox_label: None,
            checkbox_checked: false,
        };
        let state = MessageDialogState::new(opts);
        let (w, h) = state.overlay_size();
        assert!(w >= 30);
        assert!(h >= 6);
    }

    // --- InputDialogState tests ---

    #[test]
    fn input_dialog_state_new() {
        let opts = InputDialogOptions::new("Enter name:")
            .with_value("default");
        let state = InputDialogState::new(opts);
        assert_eq!(state.input_text, "default");
        assert_eq!(state.cursor_pos, 7);
    }

    #[test]
    fn input_dialog_insert_and_backspace() {
        let opts = InputDialogOptions::new("Prompt");
        let mut state = InputDialogState::new(opts);
        state.insert_char('a');
        state.insert_char('b');
        assert_eq!(state.input_text, "ab");
        assert_eq!(state.cursor_pos, 2);
        state.backspace();
        assert_eq!(state.input_text, "a");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn input_dialog_cursor_movement() {
        let opts = InputDialogOptions::new("Prompt").with_value("abc");
        let mut state = InputDialogState::new(opts);
        assert_eq!(state.cursor_pos, 3);
        state.move_left();
        assert_eq!(state.cursor_pos, 2);
        state.move_right();
        assert_eq!(state.cursor_pos, 3);
        state.move_right(); // already at end
        assert_eq!(state.cursor_pos, 3);
    }

    #[test]
    fn input_dialog_password_display() {
        let opts = InputDialogOptions::new("Password").with_password();
        let mut state = InputDialogState::new(opts);
        state.insert_char('s');
        state.insert_char('e');
        state.insert_char('c');
        assert_eq!(state.display_text(), "•••");
    }

    #[test]
    fn input_dialog_submit() {
        let opts = InputDialogOptions::new("Prompt");
        let mut state = InputDialogState::new(opts);
        state.insert_char('x');
        assert_eq!(state.submit(), Some("x".to_string()));
        state.error = Some("bad".into());
        assert_eq!(state.submit(), None);
    }

    // --- Convenience function tests ---

    #[test]
    fn show_open_file_dialog_starts_at_default() {
        let opts = FileDialogOptions::open_file()
            .with_default_path("/tmp/test");
        let state = show_open_file_dialog(opts);
        assert_eq!(state.current_dir, PathBuf::from("/tmp/test"));
    }

    #[test]
    fn show_open_folder_dialog_forces_folders() {
        let opts = FileDialogOptions::open_file();
        let state = show_open_folder_dialog(opts);
        assert!(!state.options.can_select_files);
        assert!(state.options.can_select_folders);
    }

    #[test]
    fn show_message_dialog_creates_state() {
        let opts = MessageDialogOptions {
            severity: Severity::Error,
            message: "Error occurred".into(),
            detail: None,
            buttons: vec![DialogButton { label: "OK".into(), is_secondary: false }],
            cancel_button: None,
            checkbox_label: None,
            checkbox_checked: false,
        };
        let state = show_message_dialog(opts);
        assert_eq!(state.selected_button, 0);
    }

    #[test]
    fn show_input_dialog_creates_state() {
        let opts = InputDialogOptions::new("Enter value");
        let state = show_input_dialog(opts);
        assert!(state.input_text.is_empty());
    }

    // --- ConfirmDialog tests ---

    #[test]
    fn test_confirm_dialog_yes_no() {
        let dialog = ConfirmDialog::new("Save changes?").yes_no();
        assert_eq!(dialog.option_count(), 2);
        assert_eq!(dialog.options[0].value, ConfirmValue::Yes);
        assert_eq!(dialog.options[1].value, ConfirmValue::No);
        assert_eq!(dialog.confirm().value, ConfirmValue::Yes);
    }

    #[test]
    fn test_confirm_dialog_yes_no_cancel() {
        let dialog = ConfirmDialog::new("Save?").yes_no_cancel();
        assert_eq!(dialog.option_count(), 3);
        assert_eq!(dialog.options[0].value, ConfirmValue::Yes);
        assert_eq!(dialog.options[1].value, ConfirmValue::No);
        assert_eq!(dialog.options[2].value, ConfirmValue::Cancel);
    }

    #[test]
    fn test_confirm_dialog_navigation() {
        let mut dialog = ConfirmDialog::new("Navigate?").yes_no_cancel();
        assert_eq!(dialog.selected, 0);
        dialog.select_next();
        assert_eq!(dialog.selected, 1);
        dialog.select_next();
        assert_eq!(dialog.selected, 2);
        dialog.select_next();
        assert_eq!(dialog.selected, 0); // wraps
        dialog.select_prev();
        assert_eq!(dialog.selected, 2); // wraps back
    }

    #[test]
    fn test_confirm_dialog_custom_option() {
        let dialog = ConfirmDialog::new("Choose")
            .add_option("Save All", ConfirmValue::Custom("save_all".into()))
            .add_option("Discard", ConfirmValue::Custom("discard".into()))
            .with_detail("Pick an action")
            .with_severity(Severity::Warning);
        assert_eq!(dialog.option_count(), 2);
        assert_eq!(dialog.detail, Some("Pick an action".into()));
        assert_eq!(dialog.severity, Severity::Warning);
        assert_eq!(dialog.confirm().label, "Save All");
    }

    #[test]
    fn test_confirm_value_eq() {
        assert_eq!(ConfirmValue::Yes, ConfirmValue::Yes);
        assert_ne!(ConfirmValue::Yes, ConfirmValue::No);
        assert_ne!(ConfirmValue::Cancel, ConfirmValue::No);
        assert_eq!(
            ConfirmValue::Custom("a".into()),
            ConfirmValue::Custom("a".into())
        );
        assert_ne!(
            ConfirmValue::Custom("a".into()),
            ConfirmValue::Custom("b".into())
        );
    }

    // --- InputValidator tests ---

    #[test]
    fn test_input_validator_required() {
        let v = InputValidator::new().with_required(true);
        assert!(v.validate("").is_err());
        assert!(v.validate("ok").is_ok());
    }

    #[test]
    fn test_input_validator_min_length() {
        let v = InputValidator::new().with_min_length(3);
        assert!(v.validate("ab").is_err());
        assert!(v.validate("abc").is_ok());
    }

    #[test]
    fn test_input_validator_max_length() {
        let v = InputValidator::new().with_max_length(5);
        assert!(v.validate("hello").is_ok());
        assert!(v.validate("toolong").is_err());
    }

    #[test]
    fn test_input_validator_forbidden_chars() {
        let v = InputValidator::new().with_forbidden_chars(vec!['/', '\\']);
        assert!(v.validate("hello").is_ok());
        assert!(v.validate("a/b").is_err());
        assert!(v.validate("a\\b").is_err());
    }

    #[test]
    fn test_input_validator_all_pass() {
        let v = InputValidator::new()
            .with_required(true)
            .with_min_length(2)
            .with_max_length(10)
            .with_forbidden_chars(vec!['@']);
        assert!(v.validate("hello").is_ok());
        assert!(v.validate("").is_err());
        assert!(v.validate("x").is_err());
        assert!(v.validate("way too long string").is_err());
        assert!(v.validate("h@llo").is_err());
    }

    // --- Platform / button layout tests ---

    #[test]
    fn test_button_layout_macos() {
        let buttons = vec!["OK".into(), "Cancel".into()];
        let layout = dialog_button_layout(&buttons, Platform::MacOS);
        assert_eq!(layout, vec!["Cancel", "OK"]);
    }

    #[test]
    fn test_button_layout_linux() {
        let buttons = vec!["OK".into(), "Cancel".into()];
        let layout = dialog_button_layout(&buttons, Platform::Linux);
        assert_eq!(layout, vec!["OK", "Cancel"]);
    }

    #[test]
    fn test_button_layout_windows() {
        let buttons = vec!["Save".into(), "Don't Save".into(), "Cancel".into()];
        let layout = dialog_button_layout(&buttons, Platform::Windows);
        assert_eq!(layout, vec!["Save", "Don't Save", "Cancel"]);
    }

    // --- New tests ---

    #[test]
    fn dialog_queue_push_pop_peek() {
        let mut q = DialogQueue::new();
        q.push(QueuedDialog::Message(DialogPreset::info("hello")));
        q.push(QueuedDialog::Confirm(DialogPreset::confirm_delete("file.rs")));
        q.push(QueuedDialog::Input(DialogPreset::text_input("Name", "enter name")));

        assert_eq!(q.len(), 3);
        let (msg, confirm, input) = q.counts();
        assert_eq!((msg, confirm, input), (1, 1, 1));

        assert!(matches!(q.peek(), Some(QueuedDialog::Message(_))));
        let first = q.pop().unwrap();
        assert!(matches!(first, QueuedDialog::Message(_)));
        assert_eq!(q.len(), 2);
    }

    #[test]
    fn dialog_queue_clear() {
        let mut q = DialogQueue::new();
        q.push(QueuedDialog::Message(DialogPreset::error("fail")));
        q.push(QueuedDialog::Message(DialogPreset::info("ok")));
        q.clear();
        assert!(q.is_empty());
        assert!(q.pop().is_none());
    }

    #[test]
    fn dialog_preset_confirm_delete() {
        let opts = DialogPreset::confirm_delete("main.rs");
        assert!(opts.message.contains("main.rs"));
        assert_eq!(opts.severity, Severity::Warning);
        assert_eq!(opts.primary_button, "Delete");
    }

    #[test]
    fn dialog_preset_unsaved_changes() {
        let opts = DialogPreset::unsaved_changes("config.toml");
        assert!(opts.message.contains("config.toml"));
        assert_eq!(opts.primary_button, "Save");
    }

    #[test]
    fn dialog_preset_error_and_info() {
        let err = DialogPreset::error("Something broke");
        assert_eq!(err.severity, Severity::Error);
        assert_eq!(err.buttons.len(), 1);

        let info = DialogPreset::info("Done!");
        assert_eq!(info.severity, Severity::Info);
    }

    #[test]
    fn dialog_preset_rename() {
        let opts = DialogPreset::rename("old_name.txt");
        assert_eq!(opts.value, Some("old_name.txt".into()));
        assert!(opts.validate_input);
    }

    #[test]
    fn dialog_accessibility_for_message() {
        let a = DialogAccessibility::for_message(Severity::Error, "File not found");
        assert_eq!(a.role, AriaRole::AlertDialog);
        assert!(a.live_region);
        assert_eq!(a.announce_text(), "File not found");
    }

    #[test]
    fn dialog_accessibility_with_description() {
        let a = DialogAccessibility::new(AriaRole::Dialog, "Save")
            .with_description("Save current file");
        assert_eq!(a.announce_text(), "Save: Save current file");
    }

    #[test]
    fn dialog_accessibility_for_input_and_confirm() {
        let input_a = DialogAccessibility::for_input("Enter name");
        assert_eq!(input_a.role, AriaRole::Form);

        let confirm_a = DialogAccessibility::for_confirm("Are you sure?");
        assert_eq!(confirm_a.role, AriaRole::AlertDialog);
        assert!(confirm_a.live_region);
    }

    #[test]
    fn input_validator_with_pattern() {
        let v = InputValidator::new()
            .with_pattern("@")
            .with_required(true);
        assert!(v.validate_all("user@host").is_ok());
        assert!(v.validate_all("noatsign").is_err());
        assert!(v.validate_all("").is_err()); // required
    }

    #[test]
    fn input_validator_is_valid_convenience() {
        let v = InputValidator::new()
            .with_min_length(3)
            .with_max_length(10);
        assert!(v.is_valid("hello"));
        assert!(!v.is_valid("hi"));
        assert!(!v.is_valid("way too long string here"));
    }

    // --- New tests for added functionality ---

    #[test]
    fn severity_label_and_level() {
        assert_eq!(Severity::Info.label(), "Info");
        assert_eq!(Severity::Warning.label(), "Warning");
        assert_eq!(Severity::Error.label(), "Error");

        assert_eq!(Severity::Info.level(), 0);
        assert_eq!(Severity::Warning.level(), 1);
        assert_eq!(Severity::Error.level(), 2);
    }

    #[test]
    fn severity_is_actionable() {
        assert!(!Severity::Info.is_actionable());
        assert!(Severity::Warning.is_actionable());
        assert!(Severity::Error.is_actionable());
    }

    #[test]
    fn severity_at_least() {
        assert!(Severity::Error.at_least(Severity::Warning));
        assert!(Severity::Warning.at_least(Severity::Info));
        assert!(!Severity::Info.at_least(Severity::Warning));
        assert!(Severity::Error.at_least(Severity::Error));
    }

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", Severity::Error), "Error");
        assert_eq!(format!("{}", Severity::Info), "Info");
    }

    #[test]
    fn dialog_button_constructors() {
        let p = DialogButton::primary("OK");
        assert_eq!(p.label, "OK");
        assert!(!p.is_secondary);

        let s = DialogButton::secondary("Cancel");
        assert_eq!(s.label, "Cancel");
        assert!(s.is_secondary);
    }

    #[test]
    fn file_filter_matches_extension() {
        let f = FileFilter {
            name: "Images".into(),
            extensions: vec!["png".into(), "jpg".into()],
        };
        assert!(f.matches_extension("png"));
        assert!(f.matches_extension("PNG"));
        assert!(!f.matches_extension("gif"));
    }

    #[test]
    fn file_filter_is_wildcard() {
        let empty = FileFilter { name: "All".into(), extensions: vec![] };
        assert!(empty.is_wildcard());

        let star = FileFilter { name: "All".into(), extensions: vec!["*".into()] };
        assert!(star.is_wildcard());

        let specific = FileFilter { name: "Rust".into(), extensions: vec!["rs".into()] };
        assert!(!specific.is_wildcard());
    }

    #[test]
    fn file_dialog_options_helpers() {
        let opts = FileDialogOptions::open_file()
            .with_filter("Rust", vec!["rs".into()])
            .with_filter("TOML", vec!["toml".into()])
            .with_multi_select()
            .with_title("Pick files");
        assert!(opts.can_select_many);
        assert_eq!(opts.title, Some("Pick files".into()));
        assert_eq!(opts.all_extensions(), vec!["rs", "toml"]);
        assert!(!opts.is_noop());

        let noop = FileDialogOptions {
            title: None,
            default_path: None,
            can_select_files: false,
            can_select_folders: false,
            can_select_many: false,
            filters: vec![],
        };
        assert!(noop.is_noop());
    }

    #[test]
    fn message_dialog_options_helpers() {
        let opts = DialogPreset::info("hello").with_detail("detail text");
        assert_eq!(opts.detail, Some("detail text".into()));
        assert!(!opts.has_checkbox());
        assert_eq!(opts.total_button_count(), 1);
    }

    #[test]
    fn confirm_value_predicates_and_display() {
        assert!(ConfirmValue::Yes.is_affirmative());
        assert!(ConfirmValue::Custom("save".into()).is_affirmative());
        assert!(!ConfirmValue::No.is_affirmative());
        assert!(ConfirmValue::Cancel.is_cancel());
        assert!(!ConfirmValue::Yes.is_cancel());

        assert_eq!(format!("{}", ConfirmValue::Yes), "Yes");
        assert_eq!(format!("{}", ConfirmValue::Custom("x".into())), "x");
    }

    #[test]
    fn file_picker_entry_human_size() {
        let dir_entry = FilePickerEntry {
            name: "src".into(), path: PathBuf::from("/src"), is_dir: true, size: 0,
        };
        assert_eq!(dir_entry.human_size(), "");

        let small = FilePickerEntry {
            name: "a.txt".into(), path: PathBuf::from("/a.txt"), is_dir: false, size: 512,
        };
        assert_eq!(small.human_size(), "512 B");

        let kb = FilePickerEntry {
            name: "b.txt".into(), path: PathBuf::from("/b.txt"), is_dir: false, size: 2048,
        };
        assert!(kb.human_size().contains("KB"));

        let mb = FilePickerEntry {
            name: "c.bin".into(), path: PathBuf::from("/c.bin"), is_dir: false, size: 5 * 1024 * 1024,
        };
        assert!(mb.human_size().contains("MB"));
    }

    #[test]
    fn file_picker_entry_extension() {
        let e = FilePickerEntry {
            name: "main.rs".into(), path: PathBuf::from("/main.rs"), is_dir: false, size: 0,
        };
        assert_eq!(e.extension(), Some("rs"));

        let no_ext = FilePickerEntry {
            name: "Makefile".into(), path: PathBuf::from("/Makefile"), is_dir: false, size: 0,
        };
        assert_eq!(no_ext.extension(), None);
    }

    #[test]
    fn file_picker_state_filter_and_chosen() {
        let opts = FileDialogOptions::open_file();
        let mut state = FilePickerState::new(PathBuf::from("/tmp"), opts);
        assert!(!state.has_filter());

        state.filter = "main".into();
        assert!(state.has_filter());

        state.clear_filter();
        assert!(!state.has_filter());
        assert_eq!(state.selected_index, 0);

        state.chosen.push(PathBuf::from("/tmp/a.txt"));
        assert_eq!(state.chosen_count(), 1);
        assert!(state.is_chosen(std::path::Path::new("/tmp/a.txt")));
        assert!(!state.is_chosen(std::path::Path::new("/tmp/b.txt")));
    }

    #[test]
    fn dialog_history_len_and_contains() {
        let mut h = DialogHistory::new();
        assert_eq!(h.len(), 0);
        assert!(!h.contains("/a"));

        h.record_path("/a");
        h.record_path("/b");
        assert_eq!(h.len(), 2);
        assert!(h.contains("/a"));
        assert!(!h.contains("/c"));
    }

    #[test]
    fn progress_dialog_fraction() {
        let indeterminate = ProgressDialogOptions::new("Working...");
        assert!(!indeterminate.is_determinate());
        assert_eq!(indeterminate.fraction(50), None);

        let determinate = ProgressDialogOptions::new("Loading").with_total(200);
        assert!(determinate.is_determinate());
        let frac = determinate.fraction(100).unwrap();
        assert!((frac - 0.5).abs() < f64::EPSILON);

        // Clamp to 1.0 when current > total
        let over = determinate.fraction(999).unwrap();
        assert!((over - 1.0).abs() < f64::EPSILON);

        // Zero total → 1.0
        let zero_total = ProgressDialogOptions::new("X").with_total(0);
        assert!((zero_total.fraction(0).unwrap() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn input_dialog_state_delete_forward() {
        let opts = InputDialogOptions::new("P").with_value("abc");
        let mut s = InputDialogState::new(opts);
        s.move_home();
        assert_eq!(s.cursor_pos, 0);
        s.delete_forward(); // removes 'a'
        assert_eq!(s.input_text, "bc");
        assert_eq!(s.cursor_pos, 0);
    }

    #[test]
    fn input_dialog_state_clear_and_char_count() {
        let opts = InputDialogOptions::new("P");
        let mut s = InputDialogState::new(opts);
        s.insert_char('h');
        s.insert_char('i');
        assert_eq!(s.char_count(), 2);
        assert!(!s.is_empty());

        s.clear();
        assert!(s.is_empty());
        assert_eq!(s.cursor_pos, 0);
        assert!(s.error.is_none());
    }

    #[test]
    fn input_dialog_state_move_home_end() {
        let opts = InputDialogOptions::new("P").with_value("hello");
        let mut s = InputDialogState::new(opts);
        assert_eq!(s.cursor_pos, 5);
        s.move_home();
        assert_eq!(s.cursor_pos, 0);
        s.move_end();
        assert_eq!(s.cursor_pos, 5);
    }

    #[test]
    fn confirm_dialog_affirmative_and_select_value() {
        let mut d = ConfirmDialog::new("Save?").yes_no_cancel();
        assert!(d.is_affirmative()); // selected=0 → Yes
        d.select_next(); // → No
        assert!(!d.is_affirmative());

        assert_eq!(d.index_of(&ConfirmValue::Cancel), Some(2));
        assert!(d.select_value(&ConfirmValue::Cancel));
        assert_eq!(d.selected, 2);
        assert!(!d.select_value(&ConfirmValue::Custom("nope".into())));
    }

    #[test]
    fn dialog_queue_drain_and_has_severity() {
        let mut q = DialogQueue::new();
        q.push(QueuedDialog::Message(DialogPreset::error("err")));
        q.push(QueuedDialog::Confirm(ConfirmDialogOptions::new("ok")));

        assert!(q.has_severity(Severity::Error));
        assert!(q.has_severity(Severity::Info)); // confirm defaults to Info
        assert!(!q.has_severity(Severity::Warning));

        let drained = q.drain_all();
        assert_eq!(drained.len(), 2);
        assert!(q.is_empty());
    }

    #[test]
    fn input_validator_default() {
        let v = InputValidator::default();
        assert!(v.is_valid("anything"));
        assert!(v.is_valid(""));
    }

    #[test] fn dialogHistoryService_new() { let s = DialogHistoryService::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn dialogHistoryService_add() { let mut s = DialogHistoryService::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn dialogHistoryService_remove() { let mut s = DialogHistoryService::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn dialogHistoryService_config() { let mut s = DialogHistoryService::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn dialogHistoryService_nav() { let mut s = DialogHistoryService::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn dialogHistoryService_filter() { let mut s = DialogHistoryService::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn dialogHistoryService_display() { assert!(format!("{}", DialogHistoryService::new()).contains("DialogHistoryService")); }
    #[test] fn dialogAccessNarrator_new() { let s = DialogAccessNarrator::new(); assert!(s.is_empty()); }
    #[test] fn dialogAccessNarrator_add() { let mut s = DialogAccessNarrator::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn dialogAccessNarrator_active() { let mut s = DialogAccessNarrator::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn dialogAccessNarrator_error() { let mut s = DialogAccessNarrator::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn dialogAccessNarrator_rm_group() { let mut s = DialogAccessNarrator::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn dialogAccessNarrator_display() { assert!(format!("{}", DialogAccessNarrator::new()).contains("DialogAccessNarrator")); }


    #[test] fn dialogHistoryService_snap_capture() {
        let s = DialogHistoryService::new();
        let snap = DialogHistoryServiceSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn dialogHistoryService_snap_stale() {
        let s = DialogHistoryService::new();
        let snap = DialogHistoryServiceSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn dialogHistoryService_snap_diff() {
        let s = DialogHistoryService::new();
        let s1v = DialogHistoryServiceSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn dialogHistoryService_snap_display() {
        let s = DialogHistoryService::new();
        let snap = DialogHistoryServiceSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn dialogAccessNarrator_stats_record() {
        let mut st = DialogAccessNarratorStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn dialogAccessNarrator_stats_hit_ratio() {
        let mut st = DialogAccessNarratorStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn dialogAccessNarrator_stats_merge() {
        let mut a = DialogAccessNarratorStats::new();
        a.total_adds = 5;
        let mut b = DialogAccessNarratorStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn dialogAccessNarrator_stats_display() {
        let st = DialogAccessNarratorStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn dialogHistoryService_config_default() {
        let c = DialogHistoryServiceConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn dialogHistoryService_config_builder() {
        let c = DialogHistoryServiceConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn dialogHistoryService_config_labels() {
        let mut c = DialogHistoryServiceConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn dialogHistoryService_config_cleanup_threshold() {
        let c = DialogHistoryServiceConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn dialogHistoryService_config_display() {
        assert!(format!("{}", DialogHistoryServiceConfig::new()).contains("Config"));
    }
    #[test] fn dialogAccessNarrator_stats_peaks() {
        let mut st = DialogAccessNarratorStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- DialogPriorityQueue --------------------------------------------------

    #[test]
    fn priority_queue_dequeue_highest() {
        let mut q = DialogPriorityQueue::new();
        q.enqueue(PrioritizedDialog::new(DialogPriority::Low, "lo", "msg", Severity::Info));
        q.enqueue(PrioritizedDialog::new(DialogPriority::Critical, "crit", "msg", Severity::Error));
        q.enqueue(PrioritizedDialog::new(DialogPriority::Normal, "norm", "msg", Severity::Warning));
        let d = q.dequeue().unwrap();
        assert_eq!(d.title, "crit");
    }

    #[test]
    fn priority_queue_empty() {
        let mut q = DialogPriorityQueue::new();
        assert!(q.dequeue().is_none());
        assert!(q.is_empty());
    }

    #[test]
    fn priority_queue_peek() {
        let mut q = DialogPriorityQueue::new();
        q.enqueue(PrioritizedDialog::new(DialogPriority::High, "hi", "m", Severity::Info));
        assert_eq!(q.peek().unwrap().title, "hi");
        assert_eq!(q.len(), 1); // peek doesn't remove
    }

    #[test]
    fn priority_queue_counts() {
        let mut q = DialogPriorityQueue::new();
        q.enqueue(PrioritizedDialog::new(DialogPriority::Low, "a", "m", Severity::Info));
        q.enqueue(PrioritizedDialog::new(DialogPriority::Low, "b", "m", Severity::Info));
        q.enqueue(PrioritizedDialog::new(DialogPriority::Critical, "c", "m", Severity::Error));
        assert_eq!(q.priority_counts(), (2, 0, 0, 1));
    }

    // -- DialogValidationChain ------------------------------------------------

    #[test]
    fn validation_chain_non_empty() {
        let chain = DialogValidationChain::new().non_empty();
        assert!(chain.validate("hello").is_ok());
        assert!(chain.validate("").is_err());
    }

    #[test]
    fn validation_chain_min_max() {
        let chain = DialogValidationChain::new()
            .min_length(3)
            .max_length(10);
        assert!(chain.validate("ab").is_err());
        assert!(chain.validate("abc").is_ok());
        assert!(chain.validate("12345678901").is_err());
    }

    #[test]
    fn validation_chain_no_chars() {
        let chain = DialogValidationChain::new()
            .no_chars(vec!['/', '\\']);
        assert!(chain.validate("hello").is_ok());
        assert!(chain.validate("path/to").is_err());
    }

    #[test]
    fn validation_chain_failing_rules() {
        let chain = DialogValidationChain::new()
            .non_empty()
            .min_length(5);
        let fails = chain.failing_rules("ab");
        assert_eq!(fails, vec!["min_length"]);
    }

    #[test]
    fn validation_chain_all_pass() {
        let chain = DialogValidationChain::new()
            .non_empty()
            .min_length(1)
            .max_length(100);
        assert!(chain.validate("ok").is_ok());
        assert!(chain.failing_rules("ok").is_empty());
    }

    #[test]
    fn validation_chain_multiple_fail() {
        let chain = DialogValidationChain::new()
            .non_empty()
            .min_length(5);
        let fails = chain.failing_rules("");
        assert_eq!(fails.len(), 2);
    }

    #[test]
    fn priority_dialog_with_timestamp() {
        let d = PrioritizedDialog::new(DialogPriority::Normal, "t", "m", Severity::Info)
            .with_timestamp(42);
        assert_eq!(d.enqueued_at, 42);
    }

    #[test]
    fn validation_chain_rule_count() {
        let chain = DialogValidationChain::new()
            .non_empty()
            .min_length(3)
            .max_length(100)
            .no_chars(vec!['@']);
        assert_eq!(chain.rule_count(), 4);
    }


    #[test]
    fn dialogs_config_new() {
        let cfg = DialogsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn dialogs_config_set_get() {
        let mut cfg = DialogsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn dialogs_config_remove() {
        let mut cfg = DialogsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn dialogs_config_keys_sorted() {
        let mut cfg = DialogsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn dialogs_config_bump_version() {
        let mut cfg = DialogsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn dialogs_config_clear() {
        let mut cfg = DialogsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn dialogs_config_merge() {
        let mut cfg1 = DialogsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = DialogsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn dialogs_config_disable() {
        let mut cfg = DialogsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn dialogs_rate_tracker_empty() {
        let rt = DialogsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn dialogs_rate_tracker_record() {
        let mut rt = DialogsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn dialogs_rate_tracker_prune() {
        let mut rt = DialogsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn dialogs_validator_valid() {
        let v = DialogsValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn dialogs_validator_errors() {
        let mut v = DialogsValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn dialogs_validator_clear() {
        let mut v = DialogsValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn dialogs_validator_merge() {
        let mut v1 = DialogsValidator::new();
        v1.add_error("e1");
        let mut v2 = DialogsValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn dialogs_rate_tracker_clear() {
        let mut rt = DialogsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn xh_metrics_empty() {
        let m = XhMetrics::new("dialogs");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xh_metrics_record_and_mean() {
        let mut m = XhMetrics::new("dialogs");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xh_metrics_min_max() {
        let mut m = XhMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xh_metrics_variance_and_std() {
        let mut m = XhMetrics::new("v");
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
    fn xh_metrics_percentile() {
        let mut m = XhMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xh_metrics_merge() {
        let mut a = XhMetrics::new("a");
        a.record(1.0);
        let mut b = XhMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xh_metrics_reset() {
        let mut m = XhMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xh_rate_window_empty() {
        let rw = XhRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xh_rate_window_tick_and_rate() {
        let mut rw = XhRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xh_lru_cache_basic() {
        let mut c = XhLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xh_lru_cache_contains_and_keys() {
        let mut c = XhLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xh_lru_cache_remove() {
        let mut c = XhLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xh_metrics_sum() {
        let mut m = XhMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xh_metrics_label() {
        let m = XhMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xh_lru_cache_clear() {
        let mut c = XhLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_25_push_and_len() {
        let mut rb = super::XbRingBuffer25::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_25_overwrite() {
        let mut rb = super::XbRingBuffer25::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_25_get_out_of_bounds() {
        let rb = super::XbRingBuffer25::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_25_drain_all() {
        let mut rb = super::XbRingBuffer25::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_25_peek_front_back() {
        let mut rb = super::XbRingBuffer25::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_25_clear() {
        let mut rb = super::XbRingBuffer25::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_25_capacity() {
        let rb = super::XbRingBuffer25::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_25_basic() {
        let h = super::xb_fnv1a_25(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_25(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_25_different_inputs() {
        let h1 = super::xb_fnv1a_25(b"abc");
        let h2 = super::xb_fnv1a_25(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_25_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_25(&data);
        let dec = super::xb_rle_decode_25(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_25_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_25(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_25(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_25_values() {
        assert!((super::xb_clamp_25(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_25(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_25(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_25_values() {
        assert!((super::xb_lerp_25(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_25(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_25(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_25_wrap_around_twice() {
        let mut rb = super::XbRingBuffer25::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 29 ----

    #[test]
    fn xc_29_pool_new_empty() {
        let pool: super::Xc29Pool<i32> = super::Xc29Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_29_pool_release_acquire() {
        let mut pool = super::Xc29Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_29_pool_acquire_empty() {
        let mut pool: super::Xc29Pool<i32> = super::Xc29Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_29_pool_full() {
        let mut pool = super::Xc29Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_29_pool_drain() {
        let mut pool = super::Xc29Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_29_pool_stats() {
        let mut pool = super::Xc29Pool::new(8);
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
    fn xc_29_pool_clear() {
        let mut pool = super::Xc29Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_29_pool_shrink() {
        let mut pool = super::Xc29Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_29_pool_default() {
        let pool: super::Xc29Pool<String> = super::Xc29Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_29_pool_extend() {
        let mut pool = super::Xc29Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_29_pool_retain() {
        let mut pool = super::Xc29Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_29_scheduler_round_robin() {
        let mut sched = super::Xc29Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_29_scheduler_empty() {
        let mut sched = super::Xc29Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_29_scheduler_reset() {
        let mut sched = super::Xc29Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_29_scheduler_add_remove() {
        let mut sched = super::Xc29Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_29_scheduler_targets() {
        let sched = super::Xc29Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_29_hash_empty() {
        assert_eq!(super::xc_29_hash(b""), 5381);
    }

    #[test]
    fn xc_29_hash_data() {
        let h = super::xc_29_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_29_hash(b"hello"), h);
    }

    #[test]
    fn xc_29_reverse_str() {
        assert_eq!(super::xc_29_reverse("abc"), "cba");
        assert_eq!(super::xc_29_reverse(""), "");
    }


    #[test]
    fn xe_37_pipeline_empty() {
        let p = super::Xe37Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_37_pipeline_parse_stage() {
        let p = super::Xe37Pipeline::new()
            .add_parse(super::xe_37_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_37_pipeline_transform_double() {
        let p = super::Xe37Pipeline::new()
            .add_transform(super::xe_37_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_37_pipeline_validate_reverse() {
        let p = super::Xe37Pipeline::new()
            .add_validate(super::xe_37_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_37_pipeline_emit_filter() {
        let p = super::Xe37Pipeline::new()
            .add_emit(super::xe_37_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_37_pipeline_multi_stage() {
        let p = super::Xe37Pipeline::new()
            .add_parse(super::xe_37_pipeline_identity)
            .add_transform(super::xe_37_pipeline_double)
            .add_validate(super::xe_37_pipeline_reverse)
            .add_emit(super::xe_37_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_37_pipeline_error_propagation() {
        let p = super::Xe37Pipeline::new()
            .add_parse(super::xe_37_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe37Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_37_pipeline_compose() {
        let p1 = super::Xe37Pipeline::new()
            .add_parse(super::xe_37_pipeline_identity);
        let p2 = super::Xe37Pipeline::new()
            .add_transform(super::xe_37_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_37_pipeline_error_display() {
        let e = super::Xe37PipelineError {
            stage: super::Xe37Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_37_cache_put_get() {
        let mut c = super::Xe37Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_37_cache_miss() {
        let mut c: super::Xe37Cache<&str, i32> = super::Xe37Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_37_cache_ttl_expiry() {
        let mut c = super::Xe37Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_37_cache_evict() {
        let mut c = super::Xe37Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_37_cache_capacity() {
        let mut c = super::Xe37Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_37_cache_stats() {
        let mut c = super::Xe37Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_37_cache_clear() {
        let mut c = super::Xe37Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_3 graph tests ------------------------------------------------

    #[test]
    fn xg_3_graph_empty() {
        let g = super::Xg3Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_3_graph_add_node() {
        let mut g = super::Xg3Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_3_graph_add_edge() {
        let mut g = super::Xg3Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_3_graph_neighbors() {
        let mut g = super::Xg3Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_3_graph_has_path() {
        let mut g = super::Xg3Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_3_graph_self_path() {
        let g = super::Xg3Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_3_graph_topo_sort() {
        let mut g = super::Xg3Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_3_graph_cycle_detect_false() {
        let mut g = super::Xg3Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_3_graph_cycle_detect_true() {
        let mut g = super::Xg3Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_3 heap tests -------------------------------------------------

    #[test]
    fn xg_3_heap_empty() {
        let h: super::Xg3Heap<i32> = super::Xg3Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_3_heap_push_pop() {
        let mut h = super::Xg3Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_3_heap_peek() {
        let mut h = super::Xg3Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_3_heap_drain_sorted() {
        let mut h = super::Xg3Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_3_heap_merge() {
        let mut a = super::Xg3Heap::new();
        let mut b = super::Xg3Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_3_heap_default() {
        let h: super::Xg3Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_3_graph_default() {
        let g: super::Xg3Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh28_skip_insert_contains() {
        let mut sl = super::Xh28SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh28_skip_remove() {
        let mut sl = super::Xh28SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh28_skip_len() {
        let mut sl = super::Xh28SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh28_skip_range_query() {
        let mut sl = super::Xh28SkipList::xh_new(4);
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
    fn xh28_skip_floor_ceiling() {
        let mut sl = super::Xh28SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh28_skip_rank() {
        let mut sl = super::Xh28SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh28_skip_empty() {
        let sl = super::Xh28SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh28_skip_duplicates() {
        let mut sl = super::Xh28SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh28_bitset_set_test() {
        let mut bs = super::Xh28BitSet::xh_new(256);
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
    fn xh28_bitset_clear_count() {
        let mut bs = super::Xh28BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh28_bitset_and_or_xor() {
        let mut a = super::Xh28BitSet::xh_new(128);
        let mut b = super::Xh28BitSet::xh_new(128);
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
    fn xh28_bitset_iter_ones() {
        let mut bs = super::Xh28BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh28_bitset_first_last() {
        let mut bs = super::Xh28BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh28_bitset_empty() {
        let bs = super::Xh28BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi28_deque_push_pop_back() {
        let mut dq = super::Xi28Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi28_deque_push_pop_front() {
        let mut dq = super::Xi28Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi28_deque_mixed_ops() {
        let mut dq = super::Xi28Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi28_deque_get_and_split() {
        let mut dq = super::Xi28Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi28_deque_rotate_left() {
        let mut dq = super::Xi28Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi28_deque_rotate_right() {
        let mut dq = super::Xi28Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi28_deque_grow() {
        let mut dq = super::Xi28Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi28_deque_empty() {
        let dq = super::Xi28Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi28_interval_tree_insert_query() {
        let mut tree = super::Xi28IntervalTree::xi_new();
        tree.xi_insert(super::Xi28Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi28Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi28Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi28_interval_tree_overlap() {
        let mut tree = super::Xi28IntervalTree::xi_new();
        tree.xi_insert(super::Xi28Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi28Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi28Interval::xi_new(12, 20));
        let q = super::Xi28Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi28_interval_tree_remove() {
        let mut tree = super::Xi28IntervalTree::xi_new();
        tree.xi_insert(super::Xi28Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi28Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi28_interval_tree_gaps() {
        let mut tree = super::Xi28IntervalTree::xi_new();
        tree.xi_insert(super::Xi28Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi28Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi28Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi28Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi28Interval::xi_new(8, 10));
    }

    #[test]
    fn xi28_interval_tree_merge() {
        let mut tree = super::Xi28IntervalTree::xi_new();
        tree.xi_insert(super::Xi28Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi28Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi28Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi28Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi28Interval::xi_new(10, 15));
    }

    #[test]
    fn xi28_interval_tree_all() {
        let mut tree = super::Xi28IntervalTree::xi_new();
        tree.xi_insert(super::Xi28Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi28Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi28_interval_tree_empty() {
        let tree = super::Xi28IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi28_interval_tree_contains_point() {
        let iv = super::Xi28Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 27) ---

    #[test]
    fn xj_27_uf_make_and_find() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_27_uf_union_connected() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_27_uf_component_count() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_27_uf_component_size() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_27_uf_largest_component() {
        let mut uf = super::Xj27UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_27_uf_many_elements() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_27_uf_separate_components() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_27_uf_path_compression() {
        let mut uf = super::Xj27UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_27_bt_insert_get() {
        let mut bt = super::Xj27BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_27_bt_contains_len() {
        let mut bt = super::Xj27BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_27_bt_replace() {
        let mut bt = super::Xj27BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_27_bt_remove() {
        let mut bt = super::Xj27BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_27_bt_keys_values() {
        let mut bt = super::Xj27BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_27_bt_range() {
        let mut bt = super::Xj27BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_27_bt_min_max() {
        let mut bt = super::Xj27BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_27_bt_many_inserts() {
        let mut bt = super::Xj27BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_28 segment tree tests ---

    #[test]
    fn xk_28_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk28SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_28_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk28SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_28_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk28SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_28_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk28SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_28_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk28SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_28_st_single_element() {
        let data = vec![42];
        let st = super::Xk28SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_28_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk28SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_28_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk28SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_28 disjoint intervals tests ---

    #[test]
    fn xk_28_di_add_and_count() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_28_di_merge_overlap() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_28_di_contains() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_28_di_remove() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_28_di_covered_length() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_28_di_gaps() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_28_di_merge_adjacent() {
        let mut di = super::Xk28DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_28_di_empty() {
        let di = super::Xk28DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_27_rope_new_empty() {
        let rope = super::Xl27Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_27_rope_from_str() {
        let rope = super::Xl27Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_27_rope_insert_at() {
        let mut rope = super::Xl27Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_27_rope_delete_range() {
        let mut rope = super::Xl27Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_27_rope_char_at() {
        let rope = super::Xl27Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_27_rope_split_concat() {
        let rope = super::Xl27Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_27_rope_line_count() {
        let rope = super::Xl27Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_27_rope_line_at() {
        let rope = super::Xl27Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_27_sa_build_and_search() {
        let sa = super::Xl27SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_27_sa_count() {
        let sa = super::Xl27SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_27_sa_longest_repeated() {
        let sa = super::Xl27SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_27_sa_all_positions() {
        let sa = super::Xl27SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_27_sa_len() {
        let sa = super::Xl27SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_27_sa_empty() {
        let sa = super::Xl27SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_27_rope_slice() {
        let rope = super::Xl27Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_27_sa_search_start() {
        let sa = super::Xl27SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_27_sparse_set_get() {
        let mut m = super::Xm27MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_27_sparse_row_col() {
        let mut m = super::Xm27MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_27_sparse_transpose() {
        let mut m = super::Xm27MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_27_sparse_multiply_vec() {
        let mut m = super::Xm27MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_27_sparse_nnz_density() {
        let mut m = super::Xm27MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_27_sparse_clear() {
        let mut m = super::Xm27MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_27_sparse_overwrite_zero() {
        let mut m = super::Xm27MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_27_tokenizer_basic() {
        let t = super::Xm27Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_27_tokenizer_count() {
        let t = super::Xm27Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_27_tokenizer_unique() {
        let t = super::Xm27Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_27_tokenizer_frequency() {
        let t = super::Xm27Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_27_tokenizer_delimiter() {
        let t = super::Xm27Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_27_tokenizer_whitespace() {
        let t = super::Xm27Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_27_tokenizer_empty() {
        let t = super::Xm27Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 28 ----

    #[test]
    fn xn_28_fenwick_prefix_sum() {
        let mut ft = super::Xn28Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_28_fenwick_range_sum() {
        let mut ft = super::Xn28Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_28_fenwick_point_query() {
        let mut ft = super::Xn28Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_28_fenwick_len() {
        let ft = super::Xn28Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_28_fenwick_multiple_updates() {
        let mut ft = super::Xn28Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_28_fenwick_single_element() {
        let mut ft = super::Xn28Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_28_fenwick_find_kth() {
        let mut ft = super::Xn28Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_28_fenwick_negative_delta() {
        let mut ft = super::Xn28Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 28 ----

    #[test]
    fn xn_28_avl_insert_get() {
        let mut m = super::Xn28AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_28_avl_remove() {
        let mut m = super::Xn28AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_28_avl_in_order() {
        let mut m = super::Xn28AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_28_avl_min_max() {
        let mut m = super::Xn28AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_28_avl_floor_ceiling() {
        let mut m = super::Xn28AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_28_avl_height_balanced() {
        let mut m = super::Xn28AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_28_avl_overwrite() {
        let mut m = super::Xn28AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_28_avl_empty() {
        let m: super::Xn28AVL<i32, i32> = super::Xn28AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo28RedBlack tests ---

    #[test]
    fn xo_28_rb_insert_and_get() {
        let mut tree = super::Xo28RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_28_rb_len_and_empty() {
        let mut tree = super::Xo28RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_28_rb_min_max() {
        let mut tree = super::Xo28RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_28_rb_contains() {
        let mut tree = super::Xo28RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_28_rb_remove() {
        let mut tree = super::Xo28RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_28_rb_in_order() {
        let mut tree = super::Xo28RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_28_rb_black_height() {
        let mut tree = super::Xo28RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_28_rb_overwrite() {
        let mut tree = super::Xo28RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo28ConsistentHash tests ---

    #[test]
    fn xo_28_ch_add_and_count() {
        let mut ring = super::Xo28ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_28_ch_remove_node() {
        let mut ring = super::Xo28ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_28_ch_get_node() {
        let mut ring = super::Xo28ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_28_ch_empty_ring() {
        let ring = super::Xo28ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_28_ch_distribution() {
        let mut ring = super::Xo28ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_28_ch_rebalance() {
        let mut ring = super::Xo28ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_28_ch_virtual_nodes() {
        let mut ring = super::Xo28ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_28_ch_consistent_lookup() {
        let mut ring = super::Xo28ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}
