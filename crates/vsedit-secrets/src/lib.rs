//! Secure credential storage.

/// Core type for secrets.
pub struct Secrets {
    _initialized: bool,
}

impl Secrets {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Secrets {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Secrets::new();
        assert!(v._initialized);
    }
}
