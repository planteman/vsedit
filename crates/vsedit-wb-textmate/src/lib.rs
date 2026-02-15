//! TextMate grammar tokenization.

/// Service for textmate workbench functionality.
pub struct TextmateService {
    initialized: bool,
}

impl TextmateService {
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

impl Default for TextmateService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lifecycle() {
        let mut svc = TextmateService::new();
        assert!(!svc.is_initialized());
        svc.initialize();
        assert!(svc.is_initialized());
    }
}
