//! Language pack management.

/// Core type for langpacks.
pub struct Langpacks {
    _initialized: bool,
}

impl Langpacks {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Langpacks {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Langpacks::new();
        assert!(v._initialized);
    }
}
