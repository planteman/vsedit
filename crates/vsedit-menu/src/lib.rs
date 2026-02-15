//! Menu bar and context menu system.

/// Core type for menu.
pub struct Menu {
    _initialized: bool,
}

impl Menu {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Menu::new();
        assert!(v._initialized);
    }
}
