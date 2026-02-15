//! Quick access (Ctrl+P).

/// Core type for quickaccess.
pub struct Quickaccess {
    _initialized: bool,
}

impl Quickaccess {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Quickaccess {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Quickaccess::new();
        assert!(v._initialized);
    }
}
