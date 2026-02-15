//! Hot exit and file backup.

/// Core type for backup.
pub struct Backup {
    _initialized: bool,
}

impl Backup {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Backup {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Backup::new();
        assert!(v._initialized);
    }
}
