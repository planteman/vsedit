//! Workspace history and file parsing.

/// Core type for workspaces.
pub struct Workspaces {
    _initialized: bool,
}

impl Workspaces {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Workspaces {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Workspaces::new();
        assert!(v._initialized);
    }
}
