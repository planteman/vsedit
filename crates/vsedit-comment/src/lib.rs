//! Toggle line/block comment.

/// Core type for comment.
pub struct Comment {
    _initialized: bool,
}

impl Comment {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Comment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Comment::new();
        assert!(v._initialized);
    }
}
