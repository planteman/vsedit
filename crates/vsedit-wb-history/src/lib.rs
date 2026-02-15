//! Navigation history.

/// Service for history workbench functionality.
pub struct HistoryService {
    initialized: bool,
}

impl HistoryService {
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

impl Default for HistoryService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lifecycle() {
        let mut svc = HistoryService::new();
        assert!(!svc.is_initialized());
        svc.initialize();
        assert!(svc.is_initialized());
    }
}
