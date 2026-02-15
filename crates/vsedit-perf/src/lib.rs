//! Performance monitoring.

/// Core type for perf.
pub struct Perf {
    _initialized: bool,
}

impl Perf {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for Perf {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = Perf::new();
        assert!(v._initialized);
    }
}
