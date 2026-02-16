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
}
