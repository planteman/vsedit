//! Quick input model service.

/// Core type for quickinput_svc.
pub struct QuickinputSvc {
    _initialized: bool,
}

impl QuickinputSvc {
    pub fn new() -> Self {
        Self { _initialized: true }
    }
}

impl Default for QuickinputSvc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation() {
        let v = QuickinputSvc::new();
        assert!(v._initialized);
    }
}
