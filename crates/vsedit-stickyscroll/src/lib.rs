//! Sticky scroll headers.

/// Core type for stickyscroll.
pub struct Stickyscroll {
    _initialized: bool,
}

impl Stickyscroll {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Stickyscroll {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Stickyscroll::new();
        assert!(v._initialized);
    }
}
