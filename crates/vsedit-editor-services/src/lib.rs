//! Editor service container.

/// The editor services container provides access to all editor subsystems.
pub struct EditorServices {
    initialized: bool,
}

impl EditorServices {
    pub fn new() -> Self { Self { initialized: false } }
    pub fn initialize(&mut self) { self.initialized = true; }
    pub fn is_ready(&self) -> bool { self.initialized }
}

impl Default for EditorServices {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut svc = EditorServices::new();
        assert!(!svc.is_ready());
        svc.initialize();
        assert!(svc.is_ready());
    }
}
