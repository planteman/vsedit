//! Quick fix and refactoring.

/// Core type for codeaction.
pub struct Codeaction {
    _initialized: bool,
}

impl Codeaction {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Codeaction {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Codeaction::new();
        assert!(v._initialized);
    }
}
