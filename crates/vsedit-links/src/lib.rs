//! URL detection in editor.

/// Core type for links.
pub struct Links {
    _initialized: bool,
}

impl Links {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Links {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Links::new();
        assert!(v._initialized);
    }
}
