//! URI opener service.
//!
//! Equivalent to VS Code's `vs/platform/opener/common/opener.ts`.
//! Opens URIs in the appropriate handler (editor, browser, terminal, etc.).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Result of opening a URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenResult {
    Handled,
    NotHandled,
}

/// An opener that can handle specific URIs.
pub trait IExternalUriOpener: Send + Sync {
    /// Returns true if this opener can handle the given URI.
    fn can_open(&self, uri: &str) -> bool;
    /// Open the URI. Returns whether it was handled.
    fn open(&self, uri: &str) -> OpenResult;
}

/// Opener service that routes URIs to registered openers.
pub struct OpenerService {
    openers: Mutex<Vec<Arc<dyn IExternalUriOpener>>>,
    scheme_handlers: Mutex<HashMap<String, Arc<dyn IExternalUriOpener>>>,
}

impl OpenerService {
    pub fn new() -> Self {
        Self {
            openers: Mutex::new(Vec::new()),
            scheme_handlers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a generic URI opener.
    pub fn register_opener(&self, opener: Arc<dyn IExternalUriOpener>) {
        self.openers.lock().unwrap().push(opener);
    }

    /// Register an opener for a specific URI scheme.
    pub fn register_scheme_handler(
        &self,
        scheme: &str,
        handler: Arc<dyn IExternalUriOpener>,
    ) {
        self.scheme_handlers
            .lock()
            .unwrap()
            .insert(scheme.to_string(), handler);
    }

    /// Open a URI using the first matching handler.
    pub fn open(&self, uri: &str) -> OpenResult {
        // Check scheme-specific handlers first
        if let Some(scheme) = uri.split(':').next() {
            let handlers = self.scheme_handlers.lock().unwrap();
            if let Some(handler) = handlers.get(scheme) {
                if handler.can_open(uri) {
                    return handler.open(uri);
                }
            }
        }

        // Try generic openers
        let openers = self.openers.lock().unwrap();
        for opener in openers.iter() {
            if opener.can_open(uri) {
                return opener.open(uri);
            }
        }

        OpenResult::NotHandled
    }
}

impl Default for OpenerService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HttpOpener;
    impl IExternalUriOpener for HttpOpener {
        fn can_open(&self, uri: &str) -> bool {
            uri.starts_with("http://") || uri.starts_with("https://")
        }
        fn open(&self, _uri: &str) -> OpenResult {
            OpenResult::Handled
        }
    }

    #[test]
    fn scheme_handler() {
        let svc = OpenerService::new();
        svc.register_scheme_handler("https", Arc::new(HttpOpener));
        assert_eq!(svc.open("https://example.com"), OpenResult::Handled);
        assert_eq!(svc.open("ftp://example.com"), OpenResult::NotHandled);
    }

    #[test]
    fn generic_opener() {
        let svc = OpenerService::new();
        svc.register_opener(Arc::new(HttpOpener));
        assert_eq!(svc.open("http://example.com"), OpenResult::Handled);
    }

    #[test]
    fn no_handler() {
        let svc = OpenerService::new();
        assert_eq!(svc.open("custom://resource"), OpenResult::NotHandled);
    }
}
