//! Modal dialog system.

/// The kind of dialog to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Info,
    Warning,
    Error,
    Confirm,
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
}
