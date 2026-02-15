//! File explorer.

/// Core type for explorer.
pub struct Explorer {
    _initialized: bool,
}

impl Explorer {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Explorer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Explorer::new();
        assert!(v._initialized);
    }
}
