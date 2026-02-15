//! Interactive REPL window.

/// Core type for interactive.
pub struct Interactive {
    _initialized: bool,
}

impl Interactive {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Interactive {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Interactive::new();
        assert!(v._initialized);
    }
}
