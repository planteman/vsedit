//! Hover tooltip contribution.

/// Core type for hover_contrib.
pub struct HoverContrib {
    _initialized: bool,
}

impl HoverContrib {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for HoverContrib {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = HoverContrib::new();
        assert!(v._initialized);
    }
}
