//! JSON schema registry.

/// Core type for jsonschemas.
pub struct Jsonschemas {
    _initialized: bool,
}

impl Jsonschemas {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Jsonschemas {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Jsonschemas::new();
        assert!(v._initialized);
    }
}
