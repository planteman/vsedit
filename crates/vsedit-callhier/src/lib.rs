//! Call hierarchy view.

/// Core type for callhier.
pub struct Callhier {
    _initialized: bool,
}

impl Callhier {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Callhier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Callhier::new();
        assert!(v._initialized);
    }
}
