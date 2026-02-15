//! Sixel/Kitty/iTerm2 image rendering.

/// Core type for image.
pub struct Image {
    _initialized: bool,
}

impl Image {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Image {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Image::new();
        assert!(v._initialized);
    }
}
