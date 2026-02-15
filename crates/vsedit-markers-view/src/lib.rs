//! Problems panel.

/// Core type for markers_view.
pub struct MarkersView {
    _initialized: bool,
}

impl MarkersView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for MarkersView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = MarkersView::new();
        assert!(v._initialized);
    }
}
