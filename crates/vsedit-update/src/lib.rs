//! Update mechanism.

/// Core type for update.
pub struct Update {
    _initialized: bool,
}

impl Update {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Update {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Update::new();
        assert!(v._initialized);
    }
}
