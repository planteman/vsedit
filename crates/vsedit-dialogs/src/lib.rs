//! Dialog model service.
//!
//! Equivalent to VS Code's `vs/platform/dialogs/common/dialogs.ts`.
//! Provides data models for message dialogs, file pickers, and confirmation prompts.

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
}
