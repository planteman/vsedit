//! Terminal panel integration.

/// Core type for terminal_view.
pub struct TerminalView {
    _initialized: bool,
}

impl TerminalView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for TerminalView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = TerminalView::new();
        assert!(v._initialized);
    }
}
