//! Extension manifest and schema.

/// Core type for extensions_plat.
pub struct ExtensionsPlat {
    _initialized: bool,
}

impl ExtensionsPlat {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for ExtensionsPlat {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = ExtensionsPlat::new();
        assert!(v._initialized);
    }
}
