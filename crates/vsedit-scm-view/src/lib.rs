//! Source control view.

/// Core type for scm_view.
pub struct ScmView {
    _initialized: bool,
}

impl ScmView {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for ScmView {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = ScmView::new();
        assert!(v._initialized);
    }
}
