//! Workbench logging.

/// Service for log workbench functionality.
pub struct LogService {
    initialized: bool,
}

impl LogService {
    pub fn new() -> Self {
        Self { initialized: false }
    }

    pub fn initialize(&mut self) {
        self.initialized = true;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Default for LogService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lifecycle() {
        let mut svc = LogService::new();
        assert!(!svc.is_initialized());
        svc.initialize();
        assert!(svc.is_initialized());
    }
}
