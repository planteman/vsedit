//! Remote features.

/// Core type for remote_features.
pub struct RemoteFeatures {
    _initialized: bool,
}

impl RemoteFeatures {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for RemoteFeatures {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = RemoteFeatures::new();
        assert!(v._initialized);
    }
}
