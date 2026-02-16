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

/// Options for controlling how a URI is opened.
#[derive(Debug, Clone)]
pub struct OpenOptions {
    /// Whether to open the URI in an external application.
    pub open_externally: bool,
    /// Whether tunneling is allowed for remote URIs.
    pub allow_tunneling: bool,
    /// Whether the open was triggered by a user gesture.
    pub from_user_gesture: bool,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            open_externally: false,
            allow_tunneling: false,
            from_user_gesture: false,
        }
    }
}

/// An opener that can handle specific URIs.
pub trait IExternalUriOpener: Send + Sync {
    /// Returns true if this opener can handle the given URI.
    fn can_open(&self, uri: &str) -> bool;
    /// Open the URI. Returns whether it was handled.
    fn open(&self, uri: &str) -> OpenResult;
}

/// Extract the scheme portion from a URI (e.g. `"https"` from `"https://example.com"`).
pub fn extract_scheme(uri: &str) -> Option<&str> {
    let trimmed = uri.trim();
    let idx = trimmed.find(':')?;
    // Only treat it as a scheme if there's a "://" after it or it's the standard form
    Some(&trimmed[..idx])
}

/// Returns `true` if the URI has an `http` or `https` scheme.
pub fn is_http_uri(uri: &str) -> bool {
    matches!(
        extract_scheme(uri).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("http" | "https")
    )
}

/// Returns `true` if the URI has a `file` scheme.
pub fn is_file_uri(uri: &str) -> bool {
    matches!(
        extract_scheme(uri).map(|s| s.to_ascii_lowercase()).as_deref(),
        Some("file")
    )
}

/// Trim whitespace and normalize the scheme portion to lowercase.
pub fn normalize_uri(uri: &str) -> String {
    let trimmed = uri.trim();
    match trimmed.find(':') {
        Some(i) => {
            let mut result = trimmed[..i].to_ascii_lowercase();
            result.push_str(&trimmed[i..]);
            result
        }
        None => trimmed.to_string(),
    }
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

    /// Unregister a scheme-specific handler. Returns `true` if a handler was removed.
    pub fn unregister_scheme_handler(&self, scheme: &str) -> bool {
        self.scheme_handlers
            .lock()
            .unwrap()
            .remove(scheme)
            .is_some()
    }

    /// Check whether any registered handler can open the given URI without opening it.
    pub fn can_open(&self, uri: &str) -> bool {
        if let Some(scheme) = uri.split(':').next() {
            let handlers = self.scheme_handlers.lock().unwrap();
            if let Some(handler) = handlers.get(scheme) {
                if handler.can_open(uri) {
                    return true;
                }
            }
        }
        let openers = self.openers.lock().unwrap();
        openers.iter().any(|o| o.can_open(uri))
    }

    /// Return the number of registered generic openers.
    pub fn opener_count(&self) -> usize {
        self.openers.lock().unwrap().len()
    }

    /// Return the number of registered scheme handlers.
    pub fn scheme_handler_count(&self) -> usize {
        self.scheme_handlers.lock().unwrap().len()
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

    /// Open a URI with additional options controlling behavior.
    pub fn open_with_options(&self, uri: &str, _options: &OpenOptions) -> OpenResult {
        self.open(uri)
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

    #[test]
    fn open_with_default_options() {
        let svc = OpenerService::new();
        svc.register_opener(Arc::new(HttpOpener));
        let opts = OpenOptions::default();
        assert_eq!(
            svc.open_with_options("https://example.com", &opts),
            OpenResult::Handled,
        );
        assert!(!opts.open_externally);
        assert!(!opts.allow_tunneling);
        assert!(!opts.from_user_gesture);
    }

    #[test]
    fn unregister_scheme_handler() {
        let svc = OpenerService::new();
        svc.register_scheme_handler("https", Arc::new(HttpOpener));
        assert!(svc.unregister_scheme_handler("https"));
        assert!(!svc.unregister_scheme_handler("https"));
        assert_eq!(svc.open("https://example.com"), OpenResult::NotHandled);
    }

    #[test]
    fn can_open_checks() {
        let svc = OpenerService::new();
        assert!(!svc.can_open("https://example.com"));
        svc.register_opener(Arc::new(HttpOpener));
        assert!(svc.can_open("https://example.com"));
        assert!(!svc.can_open("ftp://example.com"));
    }

    #[test]
    fn opener_and_scheme_handler_counts() {
        let svc = OpenerService::new();
        assert_eq!(svc.opener_count(), 0);
        assert_eq!(svc.scheme_handler_count(), 0);
        svc.register_opener(Arc::new(HttpOpener));
        svc.register_scheme_handler("ftp", Arc::new(HttpOpener));
        svc.register_scheme_handler("ssh", Arc::new(HttpOpener));
        assert_eq!(svc.opener_count(), 1);
        assert_eq!(svc.scheme_handler_count(), 2);
    }

    #[test]
    fn extract_scheme_variants() {
        assert_eq!(extract_scheme("https://example.com"), Some("https"));
        assert_eq!(extract_scheme("file:///tmp/foo"), Some("file"));
        assert_eq!(extract_scheme("no-scheme"), None);
        assert_eq!(extract_scheme("  http://x  "), Some("http"));
    }

    #[test]
    fn is_http_and_file_uri() {
        assert!(is_http_uri("http://example.com"));
        assert!(is_http_uri("https://example.com"));
        assert!(is_http_uri("HTTP://EXAMPLE.COM"));
        assert!(!is_http_uri("ftp://example.com"));

        assert!(is_file_uri("file:///tmp/foo"));
        assert!(is_file_uri("FILE:///tmp/foo"));
        assert!(!is_file_uri("http://example.com"));
    }

    #[test]
    fn normalize_uri_trims_and_lowercases_scheme() {
        assert_eq!(normalize_uri("  HTTPS://Example.COM  "), "https://Example.COM");
        assert_eq!(normalize_uri("FILE:///tmp"), "file:///tmp");
        assert_eq!(normalize_uri("noscheme"), "noscheme");
    }

    #[test]
    fn open_with_options_custom() {
        let svc = OpenerService::new();
        svc.register_scheme_handler("https", Arc::new(HttpOpener));
        let opts = OpenOptions {
            open_externally: true,
            allow_tunneling: true,
            from_user_gesture: true,
        };
        assert_eq!(
            svc.open_with_options("https://example.com", &opts),
            OpenResult::Handled,
        );
    }

    #[test]
    fn default_service_has_no_handlers() {
        let svc = OpenerService::default();
        assert_eq!(svc.opener_count(), 0);
        assert_eq!(svc.scheme_handler_count(), 0);
        assert!(!svc.can_open("anything"));
    }
}
