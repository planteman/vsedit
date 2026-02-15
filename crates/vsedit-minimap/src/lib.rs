//! Code minimap using braille characters.

/// Core type for minimap.
pub struct Minimap {
    _initialized: bool,
}

impl Minimap {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Minimap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Minimap::new();
        assert!(v._initialized);
    }
}
