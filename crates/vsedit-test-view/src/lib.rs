//! Test explorer view.

/// Core type for test_view.
pub struct TestView {
    _initialized: bool,
}

impl TestView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for TestView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = TestView::new();
        assert!(v._initialized);
    }
}
