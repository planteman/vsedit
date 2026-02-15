//! Enterprise policy enforcement.

/// Core type for policy.
pub struct Policy {
    _initialized: bool,
}

impl Policy {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Policy::new();
        assert!(v._initialized);
    }
}
