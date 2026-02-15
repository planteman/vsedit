//! Modal dialog system.

/// Core type for dialog.
pub struct Dialog {
    _initialized: bool,
}

impl Dialog {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Dialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Dialog::new();
        assert!(v._initialized);
    }
}
