//! Editor tab bar widget.

/// Core type for tabbar.
pub struct Tabbar {
    _initialized: bool,
}

impl Tabbar {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Tabbar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Tabbar::new();
        assert!(v._initialized);
    }
}
