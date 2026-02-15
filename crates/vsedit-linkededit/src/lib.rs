//! Linked editing ranges.

/// Core type for linkededit.
pub struct Linkededit {
    _initialized: bool,
}

impl Linkededit {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Linkededit {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Linkededit::new();
        assert!(v._initialized);
    }
}
