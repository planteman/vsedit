//! OAuth provider integration.

/// Service for auth workbench functionality.
pub struct AuthService {
    initialized: bool,
}

impl AuthService {
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

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_lifecycle() {
        let mut svc = AuthService::new();
        assert!(!svc.is_initialized());
        svc.initialize();
        assert!(svc.is_initialized());
    }
}
