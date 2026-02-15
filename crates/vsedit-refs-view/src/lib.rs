//! References view.

/// Core type for refs_view.
pub struct RefsView {
    _initialized: bool,
}

impl RefsView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for RefsView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = RefsView::new();
        assert!(v._initialized);
    }
}
