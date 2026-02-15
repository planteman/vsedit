//! Output panel channels.

/// Service for output workbench functionality.
pub struct OutputService {
    initialized: bool,
}

impl OutputService {
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

impl Default for OutputService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lifecycle() {
        let mut svc = OutputService::new();
        assert!(!svc.is_initialized());
        svc.initialize();
        assert!(svc.is_initialized());
    }
}
