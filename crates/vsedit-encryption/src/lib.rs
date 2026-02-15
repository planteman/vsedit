//! Secret storage and keyring.

/// Core type for encryption.
pub struct Encryption {
    _initialized: bool,
}

impl Encryption {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Encryption {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Encryption::new();
        assert!(v._initialized);
    }
}
