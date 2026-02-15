//! Accessibility service.

/// Core type for a11y.
pub struct A11y {
    _initialized: bool,
}

impl A11y {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for A11y {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = A11y::new();
        assert!(v._initialized);
    }
}
