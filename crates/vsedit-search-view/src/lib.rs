//! Search results view.

/// Core type for search_view.
pub struct SearchView {
    _initialized: bool,
}

impl SearchView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for SearchView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = SearchView::new();
        assert!(v._initialized);
    }
}
