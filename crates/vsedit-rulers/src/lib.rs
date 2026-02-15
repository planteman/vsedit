//! Column ruler lines.

/// Core type for rulers.
pub struct Rulers {
    _initialized: bool,
}

impl Rulers {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Rulers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Rulers::new();
        assert!(v._initialized);
    }
}
