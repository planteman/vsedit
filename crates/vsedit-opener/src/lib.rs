//! URI opener service.
//!
//! Equivalent to VS Code's `vs/platform/opener/common/opener.ts`.
//! Opens URIs in the appropriate handler (editor, browser, terminal, etc.).

use std::collections::HashMap;
use std::fmt;
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

/// Parsed components of a URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriComponents {
    pub scheme: String,
    pub authority: String,
    pub path: String,
    pub query: Option<String>,
    pub fragment: Option<String>,
}

impl fmt::Display for UriComponents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}{}", self.scheme, self.authority, self.path)?;
        if let Some(ref q) = self.query {
            write!(f, "?{}", q)?;
        }
        if let Some(ref frag) = self.fragment {
            write!(f, "#{}", frag)?;
        }
        Ok(())
    }
}

/// Parse a URI string into its components.
pub fn parse_uri(uri: &str) -> Option<UriComponents> {
    let trimmed = uri.trim();
    let scheme_end = trimmed.find("://")?;
    let scheme = trimmed[..scheme_end].to_ascii_lowercase();
    let rest = &trimmed[scheme_end + 3..];

    // Split off fragment
    let (rest, fragment) = match rest.rfind('#') {
        Some(i) => (&rest[..i], Some(rest[i + 1..].to_string())),
        None => (rest, None),
    };

    // Split off query
    let (rest, query) = match rest.find('?') {
        Some(i) => (&rest[..i], Some(rest[i + 1..].to_string())),
        None => (rest, None),
    };

    // Split authority and path
    let (authority, path) = match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.to_string(), String::new()),
    };

    Some(UriComponents {
        scheme,
        authority,
        path,
        query,
        fragment,
    })
}

/// Validate that a URI string is well-formed.
pub fn validate_uri(uri: &str) -> Result<(), OpenerError> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err(OpenerError::EmptyUri);
    }
    if !trimmed.contains("://") {
        return Err(OpenerError::MissingScheme(trimmed.to_string()));
    }
    let scheme = extract_scheme(trimmed).unwrap_or("");
    if scheme.is_empty() {
        return Err(OpenerError::MissingScheme(trimmed.to_string()));
    }
    // Scheme must be alphabetic (with hyphens/dots/plus allowed)
    if !scheme.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return Err(OpenerError::InvalidScheme(scheme.to_string()));
    }
    Ok(())
}

/// Errors from URI validation and opener operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenerError {
    EmptyUri,
    MissingScheme(String),
    InvalidScheme(String),
    UnsupportedScheme(String),
    HandlerNotFound(String),
}

impl fmt::Display for OpenerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenerError::EmptyUri => write!(f, "URI is empty"),
            OpenerError::MissingScheme(u) => write!(f, "missing scheme in URI: {u}"),
            OpenerError::InvalidScheme(s) => write!(f, "invalid scheme: {s}"),
            OpenerError::UnsupportedScheme(s) => write!(f, "unsupported scheme: {s}"),
            OpenerError::HandlerNotFound(u) => write!(f, "no handler found for: {u}"),
        }
    }
}

/// Tracks history of opened URIs.
#[derive(Debug, Clone)]
pub struct OpenHistory {
    entries: Vec<OpenHistoryEntry>,
    max_entries: usize,
}

/// A single entry in the open history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenHistoryEntry {
    pub uri: String,
    pub result: OpenResult,
    pub timestamp_ms: u64,
}

impl OpenHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn record(&mut self, uri: impl Into<String>, result: OpenResult, timestamp_ms: u64) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(OpenHistoryEntry {
            uri: uri.into(),
            result,
            timestamp_ms,
        });
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn last_opened(&self) -> Option<&OpenHistoryEntry> {
        self.entries.last()
    }

    pub fn find_by_uri(&self, uri: &str) -> Vec<&OpenHistoryEntry> {
        self.entries.iter().filter(|e| e.uri == uri).collect()
    }

    pub fn successful_opens(&self) -> Vec<&OpenHistoryEntry> {
        self.entries.iter().filter(|e| e.result == OpenResult::Handled).collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries(&self) -> &[OpenHistoryEntry] {
        &self.entries
    }

    /// Count unique URIs in history.
    pub fn unique_uri_count(&self) -> usize {
        let mut uris: Vec<&str> = self.entries.iter().map(|e| e.uri.as_str()).collect();
        uris.sort_unstable();
        uris.dedup();
        uris.len()
    }
}

/// Allow list for URI schemes to restrict which schemes can be opened.
#[derive(Debug, Clone)]
pub struct SchemeAllowList {
    allowed: Vec<String>,
}

impl SchemeAllowList {
    pub fn new() -> Self {
        Self { allowed: Vec::new() }
    }

    pub fn with_defaults() -> Self {
        Self {
            allowed: vec!["http".into(), "https".into(), "file".into(), "mailto".into()],
        }
    }

    pub fn allow(&mut self, scheme: &str) {
        let lower = scheme.to_ascii_lowercase();
        if !self.allowed.contains(&lower) {
            self.allowed.push(lower);
        }
    }

    pub fn deny(&mut self, scheme: &str) -> bool {
        let lower = scheme.to_ascii_lowercase();
        let before = self.allowed.len();
        self.allowed.retain(|s| s != &lower);
        self.allowed.len() < before
    }

    pub fn is_allowed(&self, uri: &str) -> bool {
        match extract_scheme(uri) {
            Some(scheme) => self.allowed.contains(&scheme.to_ascii_lowercase()),
            None => false,
        }
    }

    pub fn allowed_schemes(&self) -> &[String] {
        &self.allowed
    }
}

impl Default for SchemeAllowList {
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

    #[test]
    fn parse_uri_full() {
        let parsed = parse_uri("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(parsed.scheme, "https");
        assert_eq!(parsed.authority, "example.com");
        assert_eq!(parsed.path, "/path");
        assert_eq!(parsed.query, Some("q=1".into()));
        assert_eq!(parsed.fragment, Some("frag".into()));
    }

    #[test]
    fn parse_uri_no_query_no_fragment() {
        let parsed = parse_uri("http://localhost/api/v1").unwrap();
        assert_eq!(parsed.scheme, "http");
        assert_eq!(parsed.authority, "localhost");
        assert_eq!(parsed.path, "/api/v1");
        assert_eq!(parsed.query, None);
        assert_eq!(parsed.fragment, None);
    }

    #[test]
    fn parse_uri_no_path() {
        let parsed = parse_uri("http://example.com").unwrap();
        assert_eq!(parsed.authority, "example.com");
        assert!(parsed.path.is_empty());
    }

    #[test]
    fn parse_uri_invalid() {
        assert!(parse_uri("not-a-uri").is_none());
        assert!(parse_uri("").is_none());
    }

    #[test]
    fn uri_components_display() {
        let comp = UriComponents {
            scheme: "https".into(),
            authority: "example.com".into(),
            path: "/path".into(),
            query: Some("q=1".into()),
            fragment: Some("top".into()),
        };
        assert_eq!(comp.to_string(), "https://example.com/path?q=1#top");
    }

    #[test]
    fn validate_uri_ok() {
        assert!(validate_uri("http://example.com").is_ok());
        assert!(validate_uri("file:///tmp/foo").is_ok());
    }

    #[test]
    fn validate_uri_errors() {
        assert_eq!(validate_uri(""), Err(OpenerError::EmptyUri));
        assert_eq!(validate_uri("noscheme"), Err(OpenerError::MissingScheme("noscheme".into())));
    }

    #[test]
    fn opener_error_display() {
        assert_eq!(OpenerError::EmptyUri.to_string(), "URI is empty");
        assert!(OpenerError::MissingScheme("x".into()).to_string().contains("missing scheme"));
        assert!(OpenerError::InvalidScheme("x".into()).to_string().contains("invalid scheme"));
        assert!(OpenerError::HandlerNotFound("x".into()).to_string().contains("no handler"));
    }

    #[test]
    fn open_history_record_and_query() {
        let mut history = OpenHistory::new(100);
        assert!(history.is_empty());
        history.record("https://a.com", OpenResult::Handled, 1000);
        history.record("https://b.com", OpenResult::NotHandled, 2000);
        history.record("https://a.com", OpenResult::Handled, 3000);
        assert_eq!(history.len(), 3);
        assert_eq!(history.find_by_uri("https://a.com").len(), 2);
        assert_eq!(history.successful_opens().len(), 2);
        assert_eq!(history.unique_uri_count(), 2);
        assert_eq!(history.last_opened().unwrap().uri, "https://a.com");
    }

    #[test]
    fn open_history_max_entries() {
        let mut history = OpenHistory::new(2);
        history.record("a", OpenResult::Handled, 1);
        history.record("b", OpenResult::Handled, 2);
        history.record("c", OpenResult::Handled, 3);
        assert_eq!(history.len(), 2);
        assert_eq!(history.entries()[0].uri, "b");
    }

    #[test]
    fn open_history_clear() {
        let mut history = OpenHistory::new(10);
        history.record("x", OpenResult::Handled, 1);
        history.clear();
        assert!(history.is_empty());
    }

    #[test]
    fn scheme_allow_list_defaults() {
        let list = SchemeAllowList::with_defaults();
        assert!(list.is_allowed("http://example.com"));
        assert!(list.is_allowed("https://example.com"));
        assert!(list.is_allowed("file:///tmp"));
        assert!(list.is_allowed("mailto:user@example.com"));
        assert!(!list.is_allowed("ftp://example.com"));
    }

    #[test]
    fn scheme_allow_list_add_remove() {
        let mut list = SchemeAllowList::new();
        assert!(!list.is_allowed("http://example.com"));
        list.allow("http");
        assert!(list.is_allowed("http://example.com"));
        // No duplicates
        list.allow("http");
        assert_eq!(list.allowed_schemes().len(), 1);
        assert!(list.deny("http"));
        assert!(!list.is_allowed("http://example.com"));
        assert!(!list.deny("http"));
    }

    #[test]
    fn scheme_allow_list_case_insensitive() {
        let mut list = SchemeAllowList::new();
        list.allow("HTTP");
        assert!(list.is_allowed("http://example.com"));
        assert!(list.is_allowed("HTTP://EXAMPLE.COM"));
    }
}
