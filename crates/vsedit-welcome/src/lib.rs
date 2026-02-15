//! Welcome page.

/// Core type for welcome.
pub struct Welcome {
    _initialized: bool,
}

impl Welcome {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Welcome {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Welcome::new();
        assert!(v._initialized);
    }
}
