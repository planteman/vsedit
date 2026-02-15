//! Emmet abbreviation expansion.

/// Core type for emmet.
pub struct Emmet {
    _initialized: bool,
}

impl Emmet {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Emmet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Emmet::new();
        assert!(v._initialized);
    }
}
