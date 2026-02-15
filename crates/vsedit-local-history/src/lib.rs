//! Local file history.

/// Core type for local_history.
pub struct LocalHistory {
    _initialized: bool,
}

impl LocalHistory {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for LocalHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = LocalHistory::new();
        assert!(v._initialized);
    }
}
