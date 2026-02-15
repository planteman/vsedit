//! Snippets manager.

/// Core type for snippets_mgr.
pub struct SnippetsMgr {
    _initialized: bool,
}

impl SnippetsMgr {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for SnippetsMgr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = SnippetsMgr::new();
        assert!(v._initialized);
    }
}
