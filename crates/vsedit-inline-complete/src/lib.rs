//! Inline ghost text completions.

/// Core type for inline_complete.
pub struct InlineComplete {
    _initialized: bool,
}

impl InlineComplete {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for InlineComplete {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = InlineComplete::new();
        assert!(v._initialized);
    }
}
