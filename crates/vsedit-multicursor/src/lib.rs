//! Multi-cursor operations.

/// Core type for multicursor.
pub struct Multicursor {
    _initialized: bool,
}

impl Multicursor {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Multicursor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Multicursor::new();
        assert!(v._initialized);
    }
}
