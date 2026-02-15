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
}
