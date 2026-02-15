//! Inlay type annotations.

/// Core type for inlayhints.
pub struct Inlayhints {
    _initialized: bool,
}

impl Inlayhints {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Inlayhints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Inlayhints::new();
        assert!(v._initialized);
    }
}
