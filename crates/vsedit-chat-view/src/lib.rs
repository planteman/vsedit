//! Chat panel.

/// Core type for chat_view.
pub struct ChatView {
    _initialized: bool,
}

impl ChatView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = ChatView::new();
        assert!(v._initialized);
    }
}
