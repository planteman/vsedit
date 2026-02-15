//! URI comparison and normalization.

/// Core type for uriidentity.
pub struct Uriidentity {
    _initialized: bool,
}

impl Uriidentity {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Uriidentity {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Uriidentity::new();
        assert!(v._initialized);
    }
}
