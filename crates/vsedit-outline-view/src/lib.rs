//! Document outline view.

/// Core type for outline_view.
pub struct OutlineView {
    _initialized: bool,
}

impl OutlineView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for OutlineView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = OutlineView::new();
        assert!(v._initialized);
    }
}
