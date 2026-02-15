//! Multi-file diff view.

/// Core type for multidiff.
pub struct Multidiff {
    _initialized: bool,
}

impl Multidiff {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Multidiff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Multidiff::new();
        assert!(v._initialized);
    }
}
