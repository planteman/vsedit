//! Viewport and line height calculations.

/// Core type for viewlayout.
pub struct Viewlayout {
    _initialized: bool,
}

impl Viewlayout {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Viewlayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Viewlayout::new();
        assert!(v._initialized);
    }
}
