//! Notebook editor.

/// Core type for notebook_view.
pub struct NotebookView {
    _initialized: bool,
}

impl NotebookView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for NotebookView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = NotebookView::new();
        assert!(v._initialized);
    }
}
