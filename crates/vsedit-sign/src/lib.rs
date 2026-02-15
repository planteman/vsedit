//! Request signing.

/// Core type for sign.
pub struct Sign {
    _initialized: bool,
}

impl Sign {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Sign {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Sign::new();
        assert!(v._initialized);
    }
}
