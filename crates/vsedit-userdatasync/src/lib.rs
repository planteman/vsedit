//! Settings sync across devices.

/// Core type for userdatasync.
pub struct Userdatasync {
    _initialized: bool,
}

impl Userdatasync {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Userdatasync {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Userdatasync::new();
        assert!(v._initialized);
    }
}
