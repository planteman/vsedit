//! Port forwarding and tunnels.

/// Core type for tunnel.
pub struct Tunnel {
    _initialized: bool,
}

impl Tunnel {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Tunnel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Tunnel::new();
        assert!(v._initialized);
    }
}
