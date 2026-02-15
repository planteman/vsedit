//! Unicode confusable detection.

/// Core type for unicodehl.
pub struct Unicodehl {
    _initialized: bool,
}

impl Unicodehl {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Unicodehl {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Unicodehl::new();
        assert!(v._initialized);
    }
}
