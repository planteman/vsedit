//! Output panel view.

/// Core type for output_view.
pub struct OutputView {
    _initialized: bool,
}

impl OutputView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for OutputView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = OutputView::new();
        assert!(v._initialized);
    }
}
