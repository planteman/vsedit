//! Remote connection management.

/// Core type for remote.
pub struct Remote {
    _initialized: bool,
}

impl Remote {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Remote {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Remote::new();
        assert!(v._initialized);
    }
}
