//! Modal dialog system.

use std::fmt;

/// The kind of dialog to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Info,
    Warning,
    Error,
    Confirm,
}

impl fmt::Display for DialogKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogKind::Info => write!(f, "Info"),
            DialogKind::Warning => write!(f, "Warning"),
            DialogKind::Error => write!(f, "Error"),
            DialogKind::Confirm => write!(f, "Confirm"),
        }
    }
}

/// A button shown in a dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogButton {
    pub label: String,
    pub is_primary: bool,
    pub returns_value: String,
}

impl DialogButton {
    pub fn new(label: impl Into<String>, returns_value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_primary: false,
            returns_value: returns_value.into(),
        }
    }

    pub fn primary(label: impl Into<String>, returns_value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_primary: true,
            returns_value: returns_value.into(),
        }
    }
}

/// Options for showing a dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogOptions {
    pub title: String,
    pub message: String,
    pub kind: DialogKind,
    pub buttons: Vec<DialogButton>,
    pub detail: Option<String>,
}

impl DialogOptions {
    /// Convenience constructor for an OK / Cancel dialog.
    pub fn ok_cancel(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            kind: DialogKind::Info,
            buttons: vec![
                DialogButton::primary("OK", "ok"),
                DialogButton::new("Cancel", "cancel"),
            ],
            detail: None,
        }
    }

    /// Convenience constructor for a Yes / No / Cancel dialog.
    pub fn yes_no_cancel(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            kind: DialogKind::Confirm,
            buttons: vec![
                DialogButton::primary("Yes", "yes"),
                DialogButton::new("No", "no"),
                DialogButton::new("Cancel", "cancel"),
            ],
            detail: None,
        }
    }
}

/// The result of a dialog interaction.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogResult {
    pub button_value: Option<String>,
    pub cancelled: bool,
}

impl DialogResult {
    pub fn cancelled() -> Self {
        Self {
            button_value: None,
            cancelled: true,
        }
    }

    pub fn selected(value: impl Into<String>) -> Self {
        Self {
            button_value: Some(value.into()),
            cancelled: false,
        }
    }

    /// Returns `true` if the result has a button value matching `expected`.
    pub fn is_value(&self, expected: &str) -> bool {
        self.button_value.as_deref() == Some(expected)
    }
}

impl fmt::Display for DialogResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cancelled {
            write!(f, "Cancelled")
        } else if let Some(ref v) = self.button_value {
            write!(f, "Selected: {v}")
        } else {
            write!(f, "Cancelled")
        }
    }
}

/// Options for showing an input box.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBoxOptions {
    pub prompt: String,
    pub value: Option<String>,
    pub placeholder: Option<String>,
    pub password: bool,
    pub validate_input: bool,
}

/// The result of an input box interaction.
#[derive(Debug, Clone, PartialEq)]
pub struct InputBoxResult {
    pub value: Option<String>,
    pub cancelled: bool,
}

impl InputBoxResult {
    pub fn cancelled() -> Self {
        Self {
            value: None,
            cancelled: true,
        }
    }

    pub fn submitted(value: impl Into<String>) -> Self {
        Self {
            value: Some(value.into()),
            cancelled: false,
        }
    }
}

/// Trait for types that can show dialogs and input boxes.
pub trait DialogService {
    fn show_dialog(&self, options: DialogOptions) -> DialogResult;
    fn show_input_box(&self, options: InputBoxOptions) -> InputBoxResult;
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when working with dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogError {
    /// No buttons were provided in the dialog options.
    NoButtons,
    /// The user selected a value that is not among the dialog buttons.
    InvalidSelection(String),
    /// The dialog timed out waiting for user input.
    Timeout,
}

impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogError::NoButtons => write!(f, "dialog has no buttons"),
            DialogError::InvalidSelection(v) => write!(f, "invalid selection: {v}"),
            DialogError::Timeout => write!(f, "dialog timed out"),
        }
    }
}

impl std::error::Error for DialogError {}

// ---------------------------------------------------------------------------
// Builder patterns
// ---------------------------------------------------------------------------

/// Builder for constructing [`DialogOptions`] with chained methods.
#[derive(Debug, Clone)]
pub struct DialogOptionsBuilder {
    title: String,
    message: String,
    kind: DialogKind,
    buttons: Vec<DialogButton>,
    detail: Option<String>,
}

impl DialogOptionsBuilder {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            kind: DialogKind::Info,
            buttons: Vec::new(),
            detail: None,
        }
    }

    pub fn kind(mut self, kind: DialogKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn button(mut self, button: DialogButton) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn build(self) -> DialogOptions {
        DialogOptions {
            title: self.title,
            message: self.message,
            kind: self.kind,
            buttons: self.buttons,
            detail: self.detail,
        }
    }
}

/// Builder for constructing [`InputBoxOptions`] with chained methods.
#[derive(Debug, Clone)]
pub struct InputBoxOptionsBuilder {
    prompt: String,
    value: Option<String>,
    placeholder: Option<String>,
    password: bool,
    validate_input: bool,
}

impl InputBoxOptionsBuilder {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            value: None,
            placeholder: None,
            password: false,
            validate_input: false,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    pub fn validate_input(mut self, validate: bool) -> Self {
        self.validate_input = validate;
        self
    }

    pub fn build(self) -> InputBoxOptions {
        InputBoxOptions {
            prompt: self.prompt,
            value: self.value,
            placeholder: self.placeholder,
            password: self.password,
            validate_input: self.validate_input,
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience structs
// ---------------------------------------------------------------------------

/// Convenience struct for yes/no confirmation dialogs.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmDialogOptions {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
}

impl ConfirmDialogOptions {
    /// Create confirmation dialog options from a simple message.
    pub fn from_message(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            title: "Confirm".to_string(),
            message: msg,
            detail: None,
        }
    }

    /// Convert into full [`DialogOptions`] with Yes / No buttons.
    pub fn into_dialog_options(self) -> DialogOptions {
        DialogOptions {
            title: self.title,
            message: self.message,
            kind: DialogKind::Confirm,
            buttons: vec![
                DialogButton::primary("Yes", "yes"),
                DialogButton::new("No", "no"),
            ],
            detail: self.detail,
        }
    }
}

// ---------------------------------------------------------------------------
// In-memory / mock dialog service
// ---------------------------------------------------------------------------

/// A mock [`DialogService`] that returns pre-configured responses.
///
/// Useful for testing code that depends on user dialog interactions.
pub struct InMemoryDialogService {
    dialog_result: DialogResult,
    input_box_result: InputBoxResult,
}

impl InMemoryDialogService {
    pub fn new(dialog_result: DialogResult, input_box_result: InputBoxResult) -> Self {
        Self {
            dialog_result,
            input_box_result,
        }
    }

    /// Create a service that always returns a selected dialog value and a
    /// submitted input box value.
    pub fn always_ok() -> Self {
        Self {
            dialog_result: DialogResult::selected("ok"),
            input_box_result: InputBoxResult::submitted(""),
        }
    }

    /// Create a service that always returns cancelled results.
    pub fn always_cancel() -> Self {
        Self {
            dialog_result: DialogResult::cancelled(),
            input_box_result: InputBoxResult::cancelled(),
        }
    }
}

impl DialogService for InMemoryDialogService {
    fn show_dialog(&self, _options: DialogOptions) -> DialogResult {
        self.dialog_result.clone()
    }

    fn show_input_box(&self, _options: InputBoxOptions) -> InputBoxResult {
        self.input_box_result.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_result_cancelled() {
        let r = DialogResult::cancelled();
        assert!(r.cancelled);
        assert!(r.button_value.is_none());
    }

    #[test]
    fn dialog_result_selected() {
        let r = DialogResult::selected("ok");
        assert!(!r.cancelled);
        assert_eq!(r.button_value.as_deref(), Some("ok"));
    }

    #[test]
    fn dialog_button_primary() {
        let btn = DialogButton::primary("OK", "ok");
        assert!(btn.is_primary);
        assert_eq!(btn.label, "OK");
        assert_eq!(btn.returns_value, "ok");
    }

    #[test]
    fn input_box_result_submitted() {
        let r = InputBoxResult::submitted("hello");
        assert!(!r.cancelled);
        assert_eq!(r.value.as_deref(), Some("hello"));
    }

    // ----- new tests -----

    #[test]
    fn dialog_error_display_no_buttons() {
        let e = DialogError::NoButtons;
        assert_eq!(e.to_string(), "dialog has no buttons");
    }

    #[test]
    fn dialog_error_display_invalid_selection() {
        let e = DialogError::InvalidSelection("bad".into());
        assert_eq!(e.to_string(), "invalid selection: bad");
    }

    #[test]
    fn dialog_error_display_timeout() {
        assert_eq!(DialogError::Timeout.to_string(), "dialog timed out");
    }

    #[test]
    fn dialog_kind_display() {
        assert_eq!(DialogKind::Info.to_string(), "Info");
        assert_eq!(DialogKind::Warning.to_string(), "Warning");
        assert_eq!(DialogKind::Error.to_string(), "Error");
        assert_eq!(DialogKind::Confirm.to_string(), "Confirm");
    }

    #[test]
    fn dialog_result_display() {
        assert_eq!(DialogResult::selected("ok").to_string(), "Selected: ok");
        assert_eq!(DialogResult::cancelled().to_string(), "Cancelled");
    }

    #[test]
    fn dialog_result_is_value() {
        let r = DialogResult::selected("yes");
        assert!(r.is_value("yes"));
        assert!(!r.is_value("no"));
        assert!(!DialogResult::cancelled().is_value("yes"));
    }

    #[test]
    fn dialog_options_ok_cancel() {
        let opts = DialogOptions::ok_cancel("Title", "Msg");
        assert_eq!(opts.buttons.len(), 2);
        assert_eq!(opts.buttons[0].returns_value, "ok");
        assert!(opts.buttons[0].is_primary);
        assert_eq!(opts.buttons[1].returns_value, "cancel");
        assert!(!opts.buttons[1].is_primary);
    }

    #[test]
    fn dialog_options_yes_no_cancel() {
        let opts = DialogOptions::yes_no_cancel("Save?", "Do you want to save?");
        assert_eq!(opts.kind, DialogKind::Confirm);
        assert_eq!(opts.buttons.len(), 3);
        assert_eq!(opts.buttons[0].returns_value, "yes");
        assert_eq!(opts.buttons[1].returns_value, "no");
        assert_eq!(opts.buttons[2].returns_value, "cancel");
    }

    #[test]
    fn dialog_options_builder() {
        let opts = DialogOptionsBuilder::new("Title", "Message")
            .kind(DialogKind::Warning)
            .button(DialogButton::primary("Save", "save"))
            .button(DialogButton::new("Discard", "discard"))
            .detail("Some detail")
            .build();
        assert_eq!(opts.title, "Title");
        assert_eq!(opts.kind, DialogKind::Warning);
        assert_eq!(opts.buttons.len(), 2);
        assert_eq!(opts.detail.as_deref(), Some("Some detail"));
    }

    #[test]
    fn input_box_options_builder() {
        let opts = InputBoxOptionsBuilder::new("Enter name")
            .value("default")
            .placeholder("Type here…")
            .password(true)
            .validate_input(true)
            .build();
        assert_eq!(opts.prompt, "Enter name");
        assert_eq!(opts.value.as_deref(), Some("default"));
        assert_eq!(opts.placeholder.as_deref(), Some("Type here…"));
        assert!(opts.password);
        assert!(opts.validate_input);
    }

    #[test]
    fn in_memory_dialog_service_always_ok() {
        let svc = InMemoryDialogService::always_ok();
        let opts = DialogOptions::ok_cancel("T", "M");
        let result = svc.show_dialog(opts);
        assert!(result.is_value("ok"));
        assert!(!result.cancelled);
    }

    #[test]
    fn in_memory_dialog_service_always_cancel() {
        let svc = InMemoryDialogService::always_cancel();
        let opts = DialogOptions::ok_cancel("T", "M");
        let result = svc.show_dialog(opts);
        assert!(result.cancelled);

        let ib_opts = InputBoxOptionsBuilder::new("Prompt").build();
        let ib_result = svc.show_input_box(ib_opts);
        assert!(ib_result.cancelled);
    }

    #[test]
    fn confirm_dialog_options_from_message() {
        let confirm = ConfirmDialogOptions::from_message("Delete file?");
        assert_eq!(confirm.title, "Confirm");
        assert_eq!(confirm.message, "Delete file?");
        let opts = confirm.into_dialog_options();
        assert_eq!(opts.kind, DialogKind::Confirm);
        assert_eq!(opts.buttons.len(), 2);
        assert_eq!(opts.buttons[0].returns_value, "yes");
        assert_eq!(opts.buttons[1].returns_value, "no");
    }
}
