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

}
