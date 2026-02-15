//! Keyboard shortcuts editor.

/// Core type for keybindings_ui.
pub struct KeybindingsUi {
    _initialized: bool,
}

impl KeybindingsUi {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for KeybindingsUi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = KeybindingsUi::new();
        assert!(v._initialized);
    }
}
