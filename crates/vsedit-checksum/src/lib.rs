//! File integrity verification.

/// Core type for checksum.
pub struct Checksum {
    _initialized: bool,
}

impl Checksum {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Checksum {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Checksum::new();
        assert!(v._initialized);
    }
}
