//! Network utilities.

/// Core type for network.
pub struct Network {
    _initialized: bool,
}

impl Network {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Network {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Network::new();
        assert!(v._initialized);
    }
}
