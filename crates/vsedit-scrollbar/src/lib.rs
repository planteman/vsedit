//! Virtual scrollbar widget.

/// Core type for scrollbar.
pub struct Scrollbar {
    _initialized: bool,
}

impl Scrollbar {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Scrollbar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Scrollbar::new();
        assert!(v._initialized);
    }
}
