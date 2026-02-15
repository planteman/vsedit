//! Inline chat widget.

/// Core type for inlinechat_view.
pub struct InlinechatView {
    _initialized: bool,
}

impl InlinechatView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for InlinechatView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = InlinechatView::new();
        assert!(v._initialized);
    }
}
