//! Inline color preview/picker.

/// Core type for colorpicker.
pub struct Colorpicker {
    _initialized: bool,
}

impl Colorpicker {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Colorpicker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Colorpicker::new();
        assert!(v._initialized);
    }
}
