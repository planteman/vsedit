//! URI to human-readable label formatting.

/// Core type for label.
pub struct Label {
    _initialized: bool,
}

impl Label {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Label {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Label::new();
        assert!(v._initialized);
    }
}
