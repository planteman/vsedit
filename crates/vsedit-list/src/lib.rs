//! Virtual scrolling list widget.

/// Core type for list.
pub struct List {
    _initialized: bool,
}

impl List {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = List::new();
        assert!(v._initialized);
    }
}
