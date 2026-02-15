//! Mouse-based drag and drop.

/// Core type for dnd.
pub struct Dnd {
    _initialized: bool,
}

impl Dnd {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Dnd {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Dnd::new();
        assert!(v._initialized);
    }
}
