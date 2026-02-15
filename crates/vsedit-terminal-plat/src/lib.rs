//! Terminal PTY abstraction.

/// Core type for terminal_plat.
pub struct TerminalPlat {
    _initialized: bool,
}

impl TerminalPlat {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for TerminalPlat {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = TerminalPlat::new();
        assert!(v._initialized);
    }
}
