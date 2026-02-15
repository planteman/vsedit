//! Comments panel.

/// Core type for comments_view.
pub struct CommentsView {
    _initialized: bool,
}

impl CommentsView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for CommentsView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = CommentsView::new();
        assert!(v._initialized);
    }
}
