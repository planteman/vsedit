//! Settings editor UI.

/// Core type for settings_ui.
pub struct SettingsUi {
    _initialized: bool,
}

impl SettingsUi {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for SettingsUi {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = SettingsUi::new();
        assert!(v._initialized);
    }
}
