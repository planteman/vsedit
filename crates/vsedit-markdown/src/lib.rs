//! Terminal markdown renderer.

/// Core type for markdown.
pub struct Markdown {
    _initialized: bool,
}

impl Markdown {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Markdown::new();
        assert!(v._initialized);
    }
}
