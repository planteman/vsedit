//! Debug view and features.

/// Core type for debug_view.
pub struct DebugView {
    _initialized: bool,
}

impl DebugView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for DebugView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = DebugView::new();
        assert!(v._initialized);
    }
}
