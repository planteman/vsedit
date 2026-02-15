//! The complete editor widget combining all editor subsystems.

/// Editor widget state.
pub struct EditorWidget {
    pub is_focused: bool,
    pub is_readonly: bool,
    pub show_line_numbers: bool,
    pub show_minimap: bool,
}

impl EditorWidget {
    pub fn new() -> Self {
        Self {
            is_focused: false,
            is_readonly: false,
            show_line_numbers: true,
            show_minimap: false,
        }
    }
}

impl Default for EditorWidget {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let w = EditorWidget::new();
        assert!(!w.is_focused);
        assert!(w.show_line_numbers);
    }
}
