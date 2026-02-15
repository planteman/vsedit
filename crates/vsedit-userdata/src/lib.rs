//! User data profile management.

/// Core type for userdata.
pub struct Userdata {
    _initialized: bool,
}

impl Userdata {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Userdata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Userdata::new();
        assert!(v._initialized);
    }
}
