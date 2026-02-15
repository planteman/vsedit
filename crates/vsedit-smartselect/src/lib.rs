//! Expand/shrink selection.

/// Core type for smartselect.
pub struct Smartselect {
    _initialized: bool,
}

impl Smartselect {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Smartselect {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Smartselect::new();
        assert!(v._initialized);
    }
}
