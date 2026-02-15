//! Accessibility features.

/// Core type for a11y_features.
pub struct A11yFeatures {
    _initialized: bool,
}

impl A11yFeatures {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for A11yFeatures {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = A11yFeatures::new();
        assert!(v._initialized);
    }
}
