//! Function signature help.

/// Core type for paramhints.
pub struct Paramhints {
    _initialized: bool,
}

impl Paramhints {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Paramhints {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Paramhints::new();
        assert!(v._initialized);
    }
}
