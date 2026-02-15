//! Type hierarchy view.

/// Core type for typehier.
pub struct Typehier {
    _initialized: bool,
}

impl Typehier {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Typehier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Typehier::new();
        assert!(v._initialized);
    }
}
