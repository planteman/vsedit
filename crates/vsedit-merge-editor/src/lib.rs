//! 3-way merge editor.

/// Core type for merge_editor.
pub struct MergeEditor {
    _initialized: bool,
}

impl MergeEditor {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for MergeEditor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = MergeEditor::new();
        assert!(v._initialized);
    }
}
