//! Status bar widget.

/// Core type for statusbar.
pub struct Statusbar {
    _initialized: bool,
}

impl Statusbar {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Statusbar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Statusbar::new();
        assert!(v._initialized);
    }
}
