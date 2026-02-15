//! Inline code lens decorations.

/// Core type for codelens.
pub struct Codelens {
    _initialized: bool,
}

impl Codelens {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Codelens {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Codelens::new();
        assert!(v._initialized);
    }
}
