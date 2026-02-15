//! File download service.

/// Core type for download.
pub struct Download {
    _initialized: bool,
}

impl Download {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Download {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Download::new();
        assert!(v._initialized);
    }
}
