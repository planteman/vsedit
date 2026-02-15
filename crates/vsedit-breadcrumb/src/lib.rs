//! Path breadcrumb navigation.

/// Core type for breadcrumb.
pub struct Breadcrumb {
    _initialized: bool,
}

impl Breadcrumb {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Breadcrumb::new();
        assert!(v._initialized);
    }
}
