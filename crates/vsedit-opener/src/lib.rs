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

/// Accumulated statistics for opener operations.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenerStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl OpenerStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &OpenerStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for OpenerStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OpenerStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "OpenerStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for opener.
#[derive(Debug, Clone)]
pub struct OpenerValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl OpenerValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for OpenerValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in the opener registry mapping a pattern to an opener name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenerRegistryEntry {
    pub name: String,
    pub scheme_pattern: String,
    pub priority: i32,
}

/// Registry for custom URI openers that match based on scheme patterns.
pub struct OpenerRegistry {
    entries: Vec<OpenerRegistryEntry>,
}

impl OpenerRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Register an opener for a URI scheme pattern.
    pub fn register(&mut self, name: impl Into<String>, scheme_pattern: impl Into<String>, priority: i32) {
        self.entries.push(OpenerRegistryEntry {
            name: name.into(),
            scheme_pattern: scheme_pattern.into(),
            priority,
        });
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Unregister an opener by name.
    pub fn unregister(&mut self, name: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.name != name);
        self.entries.len() < before
    }

    /// Find the best matching opener for a URI.
    pub fn find_opener(&self, uri: &str) -> Option<&OpenerRegistryEntry> {
        let scheme = extract_scheme(uri)?;
        self.entries.iter().find(|e| {
            e.scheme_pattern == "*" || e.scheme_pattern.eq_ignore_ascii_case(scheme)
        })
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[OpenerRegistryEntry] {
        &self.entries
    }
}

impl Default for OpenerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of attempting to open a URI with the platform default handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultOpenResult {
    /// The URI was dispatched to the default handler.
    Dispatched(String),
    /// No default handler was found for this scheme.
    NoHandler(String),
}

/// Simulate opening a URI with the platform default handler.
/// In a real implementation this would invoke `xdg-open`, `open`, or `start`.
pub fn open_with_default(uri: &str) -> DefaultOpenResult {
    match extract_scheme(uri) {
        Some(scheme) => match scheme.to_ascii_lowercase().as_str() {
            "http" | "https" | "mailto" | "file" => {
                DefaultOpenResult::Dispatched(format!("default-handler:{}", uri))
            }
            _ => DefaultOpenResult::NoHandler(format!("no handler for scheme '{}'", scheme)),
        },
        None => DefaultOpenResult::NoHandler("no scheme found".to_string()),
    }
}

/// A URI pattern for matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriPattern {
    pub scheme: Option<String>,
    pub authority_contains: Option<String>,
    pub path_prefix: Option<String>,
}

impl UriPattern {
    pub fn scheme_only(scheme: impl Into<String>) -> Self {
        Self {
            scheme: Some(scheme.into()),
            authority_contains: None,
            path_prefix: None,
        }
    }

    pub fn with_authority(mut self, authority: impl Into<String>) -> Self {
        self.authority_contains = Some(authority.into());
        self
    }

    pub fn with_path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(prefix.into());
        self
    }

    /// Check if this pattern matches a parsed URI.
    pub fn matches(&self, components: &UriComponents) -> bool {
        if let Some(ref s) = self.scheme {
            if !s.eq_ignore_ascii_case(&components.scheme) {
                return false;
            }
        }
        if let Some(ref auth) = self.authority_contains {
            if !components.authority.contains(auth.as_str()) {
                return false;
            }
        }
        if let Some(ref prefix) = self.path_prefix {
            if !components.path.starts_with(prefix.as_str()) {
                return false;
            }
        }
        true
    }
}

/// Match a URI string against a list of patterns, returning the index of the first match.
pub fn opener_match(uri: &str, patterns: &[UriPattern]) -> Option<usize> {
    let components = parse_uri(uri)?;
    patterns.iter().position(|p| p.matches(&components))
}


// ---------------------------------------------------------------------------
// URI component parser
// ---------------------------------------------------------------------------

impl UriComponents {
    /// Parse a URI string into components.
    pub fn parse(uri: &str) -> Option<Self> {
        let (scheme, rest) = uri.split_once("://")?;
        if scheme.is_empty() {
            return None;
        }
        let (authority_and_path, fragment) = match rest.rsplit_once('#') {
            Some((before, frag)) => (before, Some(frag.to_string())),
            None => (rest, None),
        };
        let (authority_and_path, query) = match authority_and_path.split_once('?') {
            Some((before, q)) => (before, Some(q.to_string())),
            None => (authority_and_path, None),
        };
        let (authority, path) = match authority_and_path.find('/') {
            Some(idx) => (&authority_and_path[..idx], &authority_and_path[idx..]),
            None => (authority_and_path, ""),
        };
        Some(Self {
            scheme: scheme.to_string(),
            authority: authority.to_string(),
            path: path.to_string(),
            query,
            fragment,
        })
    }

    /// Reconstruct the URI from components.
    pub fn to_uri(&self) -> String {
        let mut uri = format!("{}://{}{}", self.scheme, self.authority, self.path);
        if let Some(ref q) = self.query {
            uri.push('?');
            uri.push_str(q);
        }
        if let Some(ref f) = self.fragment {
            uri.push('#');
            uri.push_str(f);
        }
        uri
    }

    /// Returns true if this is a file URI.
    pub fn is_file(&self) -> bool {
        self.scheme == "file"
    }

    /// Returns true if this is an HTTP or HTTPS URI.
    pub fn is_http(&self) -> bool {
        self.scheme == "http" || self.scheme == "https"
    }

    /// Returns the file extension from the path, if any.
    pub fn extension(&self) -> Option<String> {
        self.path.rsplit_once('.').map(|(_, ext)| ext.to_string())
    }
}

impl Default for UriComponents {
    fn default() -> Self {
        Self {
            scheme: "file".to_string(),
            authority: String::new(),
            path: String::new(),
            query: None,
            fragment: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Opener chain
// ---------------------------------------------------------------------------

/// Result of a chain of openers trying to handle a URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainResult {
    /// One of the openers handled the URI at the given index.
    Handled(usize),
    /// No opener handled the URI.
    Unhandled,
}

impl fmt::Display for ChainResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainResult::Handled(idx) => write!(f, "handled by opener #{idx}"),
            ChainResult::Unhandled => write!(f, "unhandled"),
        }
    }
}

/// Classifies a URI's scheme into a category.
pub fn classify_uri_scheme(uri: &str) -> &'static str {
    match uri.split_once("://").map(|(s, _)| s) {
        Some("http") | Some("https") => "web",
        Some("file") => "local",
        Some("ssh") | Some("sftp") => "remote",
        Some("mailto") => "email",
        Some("vscode") | Some("vscode-insiders") => "editor",
        _ => "unknown",
    }
}

/// Counts how many URIs in a list match a given scheme.
pub fn count_by_scheme(uris: &[&str], scheme: &str) -> usize {
    let prefix = format!("{scheme}://");
    uris.iter().filter(|u| u.starts_with(&prefix)).count()
}

/// Groups URIs by their scheme.
pub fn group_by_scheme<'a>(uris: &[&'a str]) -> HashMap<String, Vec<&'a str>> {
    let mut groups: HashMap<String, Vec<&'a str>> = HashMap::new();
    for &uri in uris {
        let scheme = uri.split_once("://")
            .map(|(s, _)| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        groups.entry(scheme).or_default().push(uri);
    }
    groups
}

// ---------------------------------------------------------------------------
// URI history with dedup and capacity
// ---------------------------------------------------------------------------

/// Entry in the URI history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriHistoryEntry {
    pub uri: String,
    pub timestamp_ms: u64,
    pub open_count: u64,
}

/// Tracks recently opened URIs with deduplication and max capacity.
///
/// When a URI is recorded that already exists in history its timestamp is
/// updated and its open count is incremented instead of creating a duplicate.
/// When capacity is exceeded the least-recently-used entry is evicted.
#[derive(Debug, Clone)]
pub struct UriHistory {
    entries: Vec<UriHistoryEntry>,
    capacity: usize,
}

impl UriHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "UriHistory capacity must be > 0");
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Record a URI open. Deduplicates by URI string: if the URI is already
    /// present its timestamp is updated and its count incremented; otherwise a
    /// new entry is appended and the oldest entry is evicted if at capacity.
    pub fn record(&mut self, uri: impl Into<String>, timestamp_ms: u64) {
        let uri = uri.into();
        if let Some(existing) = self.entries.iter_mut().find(|e| e.uri == uri) {
            existing.timestamp_ms = timestamp_ms;
            existing.open_count += 1;
            return;
        }
        if self.entries.len() >= self.capacity {
            // Evict the entry with the oldest timestamp.
            if let Some(oldest_idx) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, e)| e.timestamp_ms)
                .map(|(i, _)| i)
            {
                self.entries.remove(oldest_idx);
            }
        }
        self.entries.push(UriHistoryEntry {
            uri,
            timestamp_ms,
            open_count: 1,
        });
    }

    /// Return all entries ordered by most-recently-used first.
    pub fn recent(&self) -> Vec<&UriHistoryEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.timestamp_ms.cmp(&a.timestamp_ms));
        sorted
    }

    /// Look up an entry by URI.
    pub fn get(&self, uri: &str) -> Option<&UriHistoryEntry> {
        self.entries.iter().find(|e| e.uri == uri)
    }

    /// Remove a specific URI from history. Returns `true` if found.
    pub fn remove(&mut self, uri: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.uri != uri);
        self.entries.len() < before
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return the most-recently-used entry.
    pub fn most_recent(&self) -> Option<&UriHistoryEntry> {
        self.entries.iter().max_by_key(|e| e.timestamp_ms)
    }

    /// Total number of opens across all entries.
    pub fn total_opens(&self) -> u64 {
        self.entries.iter().map(|e| e.open_count).sum()
    }
}

impl fmt::Display for UriHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UriHistory({} entries, {} total opens)",
            self.len(),
            self.total_opens()
        )
    }
}

// ---------------------------------------------------------------------------
// Opener matcher
// ---------------------------------------------------------------------------

/// A rule that matches URIs to a named opener based on scheme, host substring,
/// and/or a path prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenerMatchRule {
    pub opener_name: String,
    pub scheme: Option<String>,
    pub host_contains: Option<String>,
    pub path_prefix: Option<String>,
}

/// Matches URIs against a set of rules to find the appropriate opener.
#[derive(Debug, Clone)]
pub struct OpenerMatcher {
    rules: Vec<OpenerMatchRule>,
}

impl OpenerMatcher {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a match rule.
    pub fn add_rule(
        &mut self,
        opener_name: impl Into<String>,
        scheme: Option<&str>,
        host_contains: Option<&str>,
        path_prefix: Option<&str>,
    ) {
        self.rules.push(OpenerMatchRule {
            opener_name: opener_name.into(),
            scheme: scheme.map(|s| s.to_ascii_lowercase()),
            host_contains: host_contains.map(|s| s.to_string()),
            path_prefix: path_prefix.map(|s| s.to_string()),
        });
    }

    /// Find the first matching opener name for a URI string.
    pub fn match_uri(&self, uri: &str) -> Option<&str> {
        let components = UriComponents::parse(uri)?;
        for rule in &self.rules {
            if let Some(ref s) = rule.scheme {
                if !s.eq_ignore_ascii_case(&components.scheme) {
                    continue;
                }
            }
            if let Some(ref host) = rule.host_contains {
                if !components.authority.contains(host.as_str()) {
                    continue;
                }
            }
            if let Some(ref prefix) = rule.path_prefix {
                if !components.path.starts_with(prefix.as_str()) {
                    continue;
                }
            }
            return Some(&rule.opener_name);
        }
        None
    }

    /// Return all matching opener names (not just the first).
    pub fn match_all(&self, uri: &str) -> Vec<&str> {
        let components = match UriComponents::parse(uri) {
            Some(c) => c,
            None => return Vec::new(),
        };
        self.rules
            .iter()
            .filter(|rule| {
                if let Some(ref s) = rule.scheme {
                    if !s.eq_ignore_ascii_case(&components.scheme) {
                        return false;
                    }
                }
                if let Some(ref host) = rule.host_contains {
                    if !components.authority.contains(host.as_str()) {
                        return false;
                    }
                }
                if let Some(ref prefix) = rule.path_prefix {
                    if !components.path.starts_with(prefix.as_str()) {
                        return false;
                    }
                }
                true
            })
            .map(|r| r.opener_name.as_str())
            .collect()
    }

    /// Number of rules registered.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for OpenerMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OpenerMatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpenerMatcher({} rules)", self.rules.len())
    }
}

// ---------------------------------------------------------------------------
// Open attempt diagnostics
// ---------------------------------------------------------------------------

/// Records the outcome of a single open attempt for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAttempt {
    pub uri: String,
    pub opener_name: String,
    pub result: OpenResult,
    pub timestamp_ms: u64,
    pub error_message: Option<String>,
}

impl OpenAttempt {
    pub fn success(
        uri: impl Into<String>,
        opener_name: impl Into<String>,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            uri: uri.into(),
            opener_name: opener_name.into(),
            result: OpenResult::Handled,
            timestamp_ms,
            error_message: None,
        }
    }

    pub fn failure(
        uri: impl Into<String>,
        opener_name: impl Into<String>,
        timestamp_ms: u64,
        error: impl Into<String>,
    ) -> Self {
        Self {
            uri: uri.into(),
            opener_name: opener_name.into(),
            result: OpenResult::NotHandled,
            timestamp_ms,
            error_message: Some(error.into()),
        }
    }

    pub fn is_success(&self) -> bool {
        self.result == OpenResult::Handled
    }
}

impl fmt::Display for OpenAttempt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.is_success() { "ok" } else { "fail" };
        write!(
            f,
            "[{}] {} via {} @ {}ms",
            status, self.uri, self.opener_name, self.timestamp_ms
        )?;
        if let Some(ref err) = self.error_message {
            write!(f, " ({})", err)?;
        }
        Ok(())
    }
}

impl From<OpenAttempt> for OpenResult {
    fn from(attempt: OpenAttempt) -> Self {
        attempt.result
    }
}

/// Collects open attempts for diagnostics and replay.
#[derive(Debug, Clone, Default)]
pub struct OpenAttemptLog {
    attempts: Vec<OpenAttempt>,
}

impl OpenAttemptLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, attempt: OpenAttempt) {
        self.attempts.push(attempt);
    }

    pub fn attempts(&self) -> &[OpenAttempt] {
        &self.attempts
    }

    pub fn successes(&self) -> Vec<&OpenAttempt> {
        self.attempts.iter().filter(|a| a.is_success()).collect()
    }

    pub fn failures(&self) -> Vec<&OpenAttempt> {
        self.attempts.iter().filter(|a| !a.is_success()).collect()
    }

    pub fn by_opener(&self, name: &str) -> Vec<&OpenAttempt> {
        self.attempts
            .iter()
            .filter(|a| a.opener_name == name)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.attempts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attempts.is_empty()
    }

    pub fn clear(&mut self) {
        self.attempts.clear();
    }
}

// ---------------------------------------------------------------------------
// URI sanitizer
// ---------------------------------------------------------------------------

/// Schemes considered dangerous and blocked by the sanitizer.
const DANGEROUS_SCHEMES: &[&str] = &["javascript", "data", "vbscript", "blob"];

/// Validates and cleans URI strings: blocks dangerous schemes, strips control
/// characters, and normalises percent-encoding.
#[derive(Debug, Clone)]
pub struct UriSanitizer {
    blocked_schemes: Vec<String>,
    strip_fragments: bool,
    strip_credentials: bool,
}

impl UriSanitizer {
    /// Create a sanitizer with default dangerous-scheme blocklist.
    pub fn new() -> Self {
        Self {
            blocked_schemes: DANGEROUS_SCHEMES.iter().map(|s| s.to_string()).collect(),
            strip_fragments: false,
            strip_credentials: true,
        }
    }

    /// Add a custom blocked scheme.
    pub fn block_scheme(&mut self, scheme: &str) {
        let lower = scheme.to_ascii_lowercase();
        if !self.blocked_schemes.contains(&lower) {
            self.blocked_schemes.push(lower);
        }
    }

    /// Configure whether fragments (`#...`) are stripped.
    pub fn strip_fragments(mut self, strip: bool) -> Self {
        self.strip_fragments = strip;
        self
    }

    /// Configure whether embedded credentials (`user:pass@`) are stripped.
    pub fn strip_credentials(mut self, strip: bool) -> Self {
        self.strip_credentials = strip;
        self
    }

    /// Sanitize a URI string. Returns `Err` if the URI uses a blocked scheme
    /// or is otherwise invalid.
    pub fn sanitize(&self, uri: &str) -> Result<String, OpenerError> {
        // Remove control characters.
        let cleaned: String = uri.chars().filter(|c| !c.is_control()).collect();
        let trimmed = cleaned.trim();

        if trimmed.is_empty() {
            return Err(OpenerError::EmptyUri);
        }

        // Normalise the scheme to lowercase.
        let normalised = normalize_uri(trimmed);

        // Check for blocked schemes.
        if let Some(scheme) = extract_scheme(&normalised) {
            let lower = scheme.to_ascii_lowercase();
            if self.blocked_schemes.contains(&lower) {
                return Err(OpenerError::UnsupportedScheme(lower));
            }
        }

        let mut result = normalised;

        // Strip credentials from authority (user:pass@host → host).
        if self.strip_credentials {
            if let Some(scheme_end) = result.find("://") {
                let after_scheme = scheme_end + 3;
                let rest = &result[after_scheme..];
                // Find the end of the authority section.
                let auth_end = rest.find('/').unwrap_or(rest.len());
                let authority = &rest[..auth_end];
                if let Some(at_pos) = authority.rfind('@') {
                    let prefix = &result[..after_scheme];
                    let host_part = &authority[at_pos + 1..];
                    let suffix = &rest[auth_end..];
                    result = format!("{}{}{}", prefix, host_part, suffix);
                }
            }
        }

        // Optionally strip fragments.
        if self.strip_fragments {
            if let Some(hash_pos) = result.rfind('#') {
                result.truncate(hash_pos);
            }
        }

        Ok(result)
    }

    /// Returns `true` if the URI uses a blocked scheme.
    pub fn is_blocked(&self, uri: &str) -> bool {
        extract_scheme(uri)
            .map(|s| self.blocked_schemes.contains(&s.to_ascii_lowercase()))
            .unwrap_or(false)
    }
}

impl Default for UriSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for UriSanitizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UriSanitizer(blocked: [{}])",
            self.blocked_schemes.join(", ")
        )
    }
}


// ---------------------------------------------------------------------------
// OpenerPriorityManager
// ---------------------------------------------------------------------------

pub struct OpenerPriorityManager {
    priorities: Vec<(String, i32)>,
}

impl OpenerPriorityManager {
    pub fn new() -> Self { Self { priorities: Vec::new() } }

    pub fn set_priority(&mut self, handler_id: impl Into<String>, priority: i32) {
        let id = handler_id.into();
        if let Some(entry) = self.priorities.iter_mut().find(|(h, _)| h == &id) {
            entry.1 = priority;
        } else {
            self.priorities.push((id, priority));
        }
    }

    pub fn get_priority(&self, handler_id: &str) -> Option<i32> {
        self.priorities.iter().find(|(h, _)| h == handler_id).map(|(_, p)| *p)
    }

    pub fn sorted_handlers(&self) -> Vec<String> {
        let mut sorted = self.priorities.clone();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().map(|(h, _)| h).collect()
    }

    pub fn len(&self) -> usize { self.priorities.len() }
}

impl Default for OpenerPriorityManager { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// OpenerSchemeRouter
// ---------------------------------------------------------------------------

pub struct OpenerSchemeRouter {
    routes: std::collections::HashMap<String, String>,
}

impl OpenerSchemeRouter {
    pub fn new() -> Self { Self { routes: std::collections::HashMap::new() } }

    pub fn register_scheme(&mut self, scheme: impl Into<String>, handler: impl Into<String>) {
        self.routes.insert(scheme.into(), handler.into());
    }

    pub fn route(&self, uri: &str) -> Option<&str> {
        extract_scheme(uri).and_then(|s| self.routes.get(s)).map(|s| s.as_str())
    }

    pub fn has_scheme(&self, scheme: &str) -> bool { self.routes.contains_key(scheme) }
    pub fn scheme_count(&self) -> usize { self.routes.len() }
    pub fn remove_scheme(&mut self, scheme: &str) -> bool { self.routes.remove(scheme).is_some() }
}

impl Default for OpenerSchemeRouter { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// OpenerConfirmDialog
// ---------------------------------------------------------------------------

pub struct OpenerConfirmDialog {
    pub uri: String,
    pub message: String,
    pub is_external: bool,
}

impl OpenerConfirmDialog {
    pub fn for_external_link(uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let message = format!("Open external link: {}?", &uri);
        Self { uri, message, is_external: true }
    }

    pub fn for_file(uri: impl Into<String>) -> Self {
        let uri = uri.into();
        let message = format!("Open file: {}?", &uri);
        Self { uri, message, is_external: false }
    }

    pub fn should_confirm(uri: &str) -> bool {
        is_http_uri(uri)
    }
}

impl std::fmt::Display for OpenerConfirmDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

// ---------------------------------------------------------------------------
// OpenerMetricsTracker
// ---------------------------------------------------------------------------

pub struct OpenerMetricsTracker {
    opens: u64,
    failures: u64,
    by_scheme: std::collections::HashMap<String, u64>,
}

impl OpenerMetricsTracker {
    pub fn new() -> Self { Self { opens: 0, failures: 0, by_scheme: std::collections::HashMap::new() } }

    pub fn record_open(&mut self, uri: &str) {
        self.opens += 1;
        if let Some(scheme) = extract_scheme(uri) {
            *self.by_scheme.entry(scheme.to_string()).or_insert(0) += 1;
        }
    }

    pub fn record_failure(&mut self) { self.failures += 1; }

    pub fn total_opens(&self) -> u64 { self.opens }
    pub fn total_failures(&self) -> u64 { self.failures }
    pub fn opens_for_scheme(&self, scheme: &str) -> u64 { self.by_scheme.get(scheme).copied().unwrap_or(0) }
    pub fn success_rate(&self) -> f64 {
        let total = self.opens + self.failures;
        if total == 0 { 1.0 } else { self.opens as f64 / total as f64 }
    }
    pub fn reset(&mut self) { self.opens = 0; self.failures = 0; self.by_scheme.clear(); }
}

impl Default for OpenerMetricsTracker { fn default() -> Self { Self::new() } }

impl std::fmt::Display for OpenerMetricsTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OpenerMetrics(opens={}, failures={})", self.opens, self.failures)
    }
}


// ---------------------------------------------------------------------------
// OpenerProtocolHandlerRegistry
// ---------------------------------------------------------------------------

/// A handler for a specific URI protocol/scheme.
pub trait ProtocolHandler: Send + Sync + fmt::Debug {
    /// The scheme this handler is registered for (e.g., "vscode", "mailto").
    fn scheme(&self) -> &str;
    /// Attempt to handle the URI. Returns `true` if handled.
    fn handle(&self, uri: &str) -> bool;
    /// A human-readable description of this handler.
    fn description(&self) -> &str;
}

/// Registry for URI protocol handlers.
#[derive(Debug)]
pub struct OpenerProtocolHandlerRegistry {
    handlers: Vec<Arc<dyn ProtocolHandler>>,
    /// Dispatch log: (scheme, uri, handled)
    dispatch_log: Vec<(String, String, bool)>,
    max_log: usize,
}

impl OpenerProtocolHandlerRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            dispatch_log: Vec::new(),
            max_log: 200,
        }
    }

    /// Register a protocol handler.
    pub fn register(&mut self, handler: Arc<dyn ProtocolHandler>) {
        self.handlers.push(handler);
    }

    /// Unregister all handlers for a given scheme. Returns count removed.
    pub fn unregister_scheme(&mut self, scheme: &str) -> usize {
        let before = self.handlers.len();
        self.handlers.retain(|h| h.scheme() != scheme);
        before - self.handlers.len()
    }

    /// Dispatch a URI to the appropriate handler.
    pub fn dispatch(&mut self, uri: &str) -> OpenResult {
        let scheme = extract_scheme(uri).unwrap_or("").to_string();
        let mut result_handled = false;
        for handler in &self.handlers {
            if handler.scheme() == scheme {
                let handled = handler.handle(uri);
                if handled {
                    result_handled = true;
                    break;
                }
            }
        }
        if result_handled {
            self.log_dispatch(&scheme, uri, true);
            OpenResult::Handled
        } else {
            self.log_dispatch(&scheme, uri, false);
            OpenResult::NotHandled
        }
    }

    fn log_dispatch(&mut self, scheme: &str, uri: &str, handled: bool) {
        if self.dispatch_log.len() >= self.max_log {
            self.dispatch_log.remove(0);
        }
        self.dispatch_log.push((scheme.to_string(), uri.to_string(), handled));
    }

    /// Number of registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    /// All unique schemes with registered handlers.
    pub fn registered_schemes(&self) -> Vec<String> {
        let mut schemes: Vec<String> = self.handlers.iter().map(|h| h.scheme().to_string()).collect();
        schemes.sort();
        schemes.dedup();
        schemes
    }

    /// Check if a scheme has at least one handler.
    pub fn has_handler(&self, scheme: &str) -> bool {
        self.handlers.iter().any(|h| h.scheme() == scheme)
    }

    /// Number of dispatches logged.
    pub fn dispatch_count(&self) -> usize {
        self.dispatch_log.len()
    }

    /// Number of successful dispatches.
    pub fn successful_dispatches(&self) -> usize {
        self.dispatch_log.iter().filter(|(_, _, h)| *h).count()
    }

    /// Clear all handlers.
    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    /// Clear the dispatch log.
    pub fn clear_log(&mut self) {
        self.dispatch_log.clear();
    }

    /// Find handlers for a specific scheme.
    pub fn handlers_for_scheme(&self, scheme: &str) -> Vec<&dyn ProtocolHandler> {
        self.handlers.iter().filter(|h| h.scheme() == scheme).map(|h| h.as_ref()).collect()
    }
}

impl fmt::Display for OpenerProtocolHandlerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ProtocolHandlerRegistry({} handlers, {} schemes)",
            self.handler_count(),
            self.registered_schemes().len()
        )
    }
}

// ---------------------------------------------------------------------------
// OpenerExternalConfirm
// ---------------------------------------------------------------------------

/// Confirmation policy for opening external URIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalConfirmPolicy {
    /// Always allow without prompting.
    AlwaysAllow,
    /// Always deny without prompting.
    AlwaysDeny,
    /// Prompt user for confirmation.
    Prompt,
}

/// Result of a confirmation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmResult {
    Allowed,
    Denied,
    NeedsPrompt { uri: String, scheme: String },
}

/// Manages confirmation dialogs for opening external URIs.
#[derive(Debug, Clone)]
pub struct OpenerExternalConfirm {
    /// Per-scheme policies.
    scheme_policies: HashMap<String, ExternalConfirmPolicy>,
    /// Global default policy.
    default_policy: ExternalConfirmPolicy,
    /// Trusted domains (bypass confirmation).
    trusted_domains: Vec<String>,
    /// Blocked domains.
    blocked_domains: Vec<String>,
}

impl OpenerExternalConfirm {
    /// Create with a default policy.
    pub fn new(default_policy: ExternalConfirmPolicy) -> Self {
        Self {
            scheme_policies: HashMap::new(),
            default_policy,
            trusted_domains: Vec::new(),
            blocked_domains: Vec::new(),
        }
    }

    /// Set policy for a specific scheme.
    pub fn set_scheme_policy(&mut self, scheme: &str, policy: ExternalConfirmPolicy) {
        self.scheme_policies.insert(scheme.to_string(), policy);
    }

    /// Add a trusted domain (will always be allowed).
    pub fn trust_domain(&mut self, domain: &str) {
        if !self.trusted_domains.contains(&domain.to_string()) {
            self.trusted_domains.push(domain.to_string());
        }
    }

    /// Block a domain (will always be denied).
    pub fn block_domain(&mut self, domain: &str) {
        if !self.blocked_domains.contains(&domain.to_string()) {
            self.blocked_domains.push(domain.to_string());
        }
    }

    /// Remove a trusted domain.
    pub fn untrust_domain(&mut self, domain: &str) -> bool {
        let before = self.trusted_domains.len();
        self.trusted_domains.retain(|d| d != domain);
        self.trusted_domains.len() < before
    }

    /// Check if a URI should be allowed, denied, or needs a prompt.
    pub fn check(&self, uri: &str) -> ConfirmResult {
        // Check blocked domains first.
        for domain in &self.blocked_domains {
            if uri.contains(domain.as_str()) {
                return ConfirmResult::Denied;
            }
        }
        // Check trusted domains.
        for domain in &self.trusted_domains {
            if uri.contains(domain.as_str()) {
                return ConfirmResult::Allowed;
            }
        }
        // Check scheme-specific policy.
        let scheme = extract_scheme(uri).unwrap_or("").to_string();
        let policy = self.scheme_policies.get(&scheme).copied().unwrap_or(self.default_policy);
        match policy {
            ExternalConfirmPolicy::AlwaysAllow => ConfirmResult::Allowed,
            ExternalConfirmPolicy::AlwaysDeny => ConfirmResult::Denied,
            ExternalConfirmPolicy::Prompt => ConfirmResult::NeedsPrompt {
                uri: uri.to_string(),
                scheme,
            },
        }
    }

    /// Number of trusted domains.
    pub fn trusted_count(&self) -> usize {
        self.trusted_domains.len()
    }

    /// Number of blocked domains.
    pub fn blocked_count(&self) -> usize {
        self.blocked_domains.len()
    }

    /// Number of scheme-specific policies.
    pub fn scheme_policy_count(&self) -> usize {
        self.scheme_policies.len()
    }

    /// Default policy.
    pub fn default_policy(&self) -> ExternalConfirmPolicy {
        self.default_policy
    }

    /// Check if a domain is trusted.
    pub fn is_trusted(&self, domain: &str) -> bool {
        self.trusted_domains.iter().any(|d| d == domain)
    }

    /// Check if a domain is blocked.
    pub fn is_blocked(&self, domain: &str) -> bool {
        self.blocked_domains.iter().any(|d| d == domain)
    }

    /// Reset all policies, trusts, and blocks.
    pub fn reset(&mut self) {
        self.scheme_policies.clear();
        self.trusted_domains.clear();
        self.blocked_domains.clear();
    }
}

impl fmt::Display for OpenerExternalConfirm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExternalConfirm(default={:?}, {} trusted, {} blocked)",
            self.default_policy,
            self.trusted_count(),
            self.blocked_count()
        )
    }
}



// ─── Open LRU Cache ───────────────────────────────────────

/// A simple LRU cache for opened files.
#[derive(Debug)]
pub struct OpenLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> OpenLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for OpenLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpenLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── Open Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for opener history.
#[derive(Debug, Clone)]
pub struct OpenRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> OpenRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for OpenRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OpenRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}


/// Configuration manager for opener functionality.
pub struct OpenerConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl OpenerConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &OpenerConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for opener operations.
pub struct OpenerRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl OpenerRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for opener.
pub struct OpenerValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl OpenerValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &OpenerValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for opener
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaOpenerRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaOpenerRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaOpenerCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaOpenerCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaOpenerCounter {
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

    #[test]
    fn opener_stats_new_defaults() {
        let stats = OpenerStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn opener_stats_record_success() {
        let mut stats = OpenerStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn opener_stats_record_failure() {
        let mut stats = OpenerStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn opener_stats_reset() {
        let mut stats = OpenerStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn opener_stats_merge() {
        let mut a = OpenerStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = OpenerStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn opener_stats_display() {
        let mut stats = OpenerStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn opener_stats_default() {
        let stats = OpenerStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn opener_validator_accepts_and_rejects() {
        let mut v = OpenerValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad input");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn opener_validator_warnings() {
        let mut v = OpenerValidationCollector::new();
        v.add_warning("deprecated");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn opener_validator_clear_and_merge() {
        let mut v = OpenerValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = OpenerValidationCollector::new();
        a.add_error("a_err");
        let mut b = OpenerValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    #[test]
    fn opener_registry_register_and_find() {
        let mut reg = OpenerRegistry::new();
        reg.register("browser", "https", 10);
        reg.register("editor", "file", 5);
        let entry = reg.find_opener("https://example.com").unwrap();
        assert_eq!(entry.name, "browser");
        let entry = reg.find_opener("file:///path").unwrap();
        assert_eq!(entry.name, "editor");
    }

    #[test]
    fn opener_registry_wildcard() {
        let mut reg = OpenerRegistry::new();
        reg.register("catch-all", "*", 0);
        assert!(reg.find_opener("custom://something").is_some());
    }

    #[test]
    fn opener_registry_unregister() {
        let mut reg = OpenerRegistry::new();
        reg.register("test", "http", 1);
        assert!(reg.unregister("test"));
        assert_eq!(reg.entry_count(), 0);
    }

    #[test]
    fn open_with_default_http() {
        let result = open_with_default("https://example.com");
        assert!(matches!(result, DefaultOpenResult::Dispatched(_)));
    }

    #[test]
    fn open_with_default_unknown_scheme() {
        let result = open_with_default("custom://foo");
        assert!(matches!(result, DefaultOpenResult::NoHandler(_)));
    }

    #[test]
    fn uri_pattern_scheme_match() {
        let p = UriPattern::scheme_only("https");
        let c = parse_uri("https://example.com/path").unwrap();
        assert!(p.matches(&c));
    }

    #[test]
    fn uri_pattern_authority_match() {
        let p = UriPattern::scheme_only("https").with_authority("github");
        let c = parse_uri("https://github.com/repo").unwrap();
        assert!(p.matches(&c));
        let c2 = parse_uri("https://example.com/repo").unwrap();
        assert!(!p.matches(&c2));
    }

    #[test]
    fn opener_match_finds_first() {
        let patterns = vec![
            UriPattern::scheme_only("file"),
            UriPattern::scheme_only("https"),
        ];
        assert_eq!(opener_match("https://x.com", &patterns), Some(1));
        assert_eq!(opener_match("file:///a", &patterns), Some(0));
    }

    #[test]
    fn test_uri_components_parse_full() {
        let c = UriComponents::parse("https://example.com/path?q=1#frag").unwrap();
        assert_eq!(c.scheme, "https");
        assert_eq!(c.authority, "example.com");
        assert_eq!(c.path, "/path");
        assert_eq!(c.query.as_deref(), Some("q=1"));
        assert_eq!(c.fragment.as_deref(), Some("frag"));
    }

    #[test]
    fn test_uri_components_roundtrip() {
        let uri = "https://example.com/path?q=1#frag";
        let c = UriComponents::parse(uri).unwrap();
        assert_eq!(c.to_uri(), uri);
    }

    #[test]
    fn test_uri_components_is_file() {
        let c = UriComponents::parse("file:///tmp/test.rs").unwrap();
        assert!(c.is_file());
        assert!(!c.is_http());
    }

    #[test]
    fn test_uri_components_extension() {
        let c = UriComponents::parse("file:///tmp/test.rs").unwrap();
        assert_eq!(c.extension().as_deref(), Some("rs"));
    }

    #[test]
    fn test_uri_components_display() {
        let c = UriComponents::parse("https://host/path").unwrap();
        assert_eq!(format!("{c}"), "https://host/path");
    }

    #[test]
    fn test_uri_components_default() {
        let c = UriComponents::default();
        assert_eq!(c.scheme, "file");
        assert!(c.authority.is_empty());
    }

    #[test]
    fn test_chain_result_display() {
        assert_eq!(format!("{}", ChainResult::Handled(2)), "handled by opener #2");
        assert_eq!(format!("{}", ChainResult::Unhandled), "unhandled");
    }

    #[test]
    fn test_classify_uri_scheme() {
        assert_eq!(classify_uri_scheme("https://example.com"), "web");
        assert_eq!(classify_uri_scheme("file:///tmp"), "local");
        assert_eq!(classify_uri_scheme("ssh://host"), "remote");
        assert_eq!(classify_uri_scheme("custom://x"), "unknown");
    }

    #[test]
    fn test_count_by_scheme() {
        let uris = vec!["https://a.com", "https://b.com", "file:///c"];
        assert_eq!(count_by_scheme(&uris, "https"), 2);
        assert_eq!(count_by_scheme(&uris, "file"), 1);
    }

    #[test]
    fn test_group_by_scheme() {
        let uris = vec!["https://a.com", "file:///b", "https://c.com"];
        let groups = group_by_scheme(&uris);
        assert_eq!(groups["https"].len(), 2);
        assert_eq!(groups["file"].len(), 1);
    }

    // -------------------------------------------------------------------
    // UriHistory tests
    // -------------------------------------------------------------------

    #[test]
    fn uri_history_dedup_and_count() {
        let mut h = UriHistory::new(10);
        h.record("https://a.com", 100);
        h.record("https://b.com", 200);
        h.record("https://a.com", 300);
        assert_eq!(h.len(), 2, "duplicate URI should not create new entry");
        let entry = h.get("https://a.com").unwrap();
        assert_eq!(entry.open_count, 2);
        assert_eq!(entry.timestamp_ms, 300, "timestamp should be updated");
        assert_eq!(h.total_opens(), 3);
    }

    #[test]
    fn uri_history_capacity_eviction() {
        let mut h = UriHistory::new(2);
        h.record("https://a.com", 100);
        h.record("https://b.com", 200);
        h.record("https://c.com", 300);
        assert_eq!(h.len(), 2);
        assert!(h.get("https://a.com").is_none(), "oldest entry evicted");
        assert!(h.get("https://b.com").is_some());
        assert!(h.get("https://c.com").is_some());
    }

    #[test]
    fn uri_history_most_recent_and_display() {
        let mut h = UriHistory::new(5);
        h.record("https://a.com", 10);
        h.record("https://b.com", 50);
        h.record("https://c.com", 30);
        let mr = h.most_recent().unwrap();
        assert_eq!(mr.uri, "https://b.com");
        let recent = h.recent();
        assert_eq!(recent[0].uri, "https://b.com");
        assert_eq!(recent[1].uri, "https://c.com");
        let display = format!("{h}");
        assert!(display.contains("3 entries"));
    }

    #[test]
    fn uri_history_remove_and_clear() {
        let mut h = UriHistory::new(5);
        h.record("https://a.com", 1);
        h.record("https://b.com", 2);
        assert!(h.remove("https://a.com"));
        assert!(!h.remove("https://a.com"));
        assert_eq!(h.len(), 1);
        h.clear();
        assert!(h.is_empty());
    }

    // -------------------------------------------------------------------
    // OpenerMatcher tests
    // -------------------------------------------------------------------

    #[test]
    fn opener_matcher_scheme_and_host() {
        let mut m = OpenerMatcher::new();
        m.add_rule("browser", Some("https"), None, None);
        m.add_rule("github-app", Some("https"), Some("github.com"), None);
        m.add_rule("editor", Some("file"), None, None);

        assert_eq!(m.match_uri("https://example.com/page"), Some("browser"));
        assert_eq!(m.match_uri("file:///tmp/foo.rs"), Some("editor"));
        assert_eq!(m.match_uri("ftp://x.com"), None);

        // match_all returns all matches
        let all = m.match_all("https://github.com/repo");
        assert!(all.contains(&"browser"));
        assert!(all.contains(&"github-app"));
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn opener_matcher_path_prefix() {
        let mut m = OpenerMatcher::new();
        m.add_rule("api-handler", Some("https"), None, Some("/api/"));

        assert_eq!(
            m.match_uri("https://example.com/api/v1/users"),
            Some("api-handler")
        );
        assert_eq!(m.match_uri("https://example.com/docs"), None);
        assert_eq!(format!("{m}"), "OpenerMatcher(1 rules)");
    }

    // -------------------------------------------------------------------
    // OpenAttempt & OpenAttemptLog tests
    // -------------------------------------------------------------------

    #[test]
    fn open_attempt_success_and_failure() {
        let ok = OpenAttempt::success("https://a.com", "browser", 1000);
        assert!(ok.is_success());
        assert!(ok.error_message.is_none());
        let display = format!("{ok}");
        assert!(display.contains("[ok]"));
        assert!(display.contains("browser"));

        let fail = OpenAttempt::failure("ftp://x.com", "ftp-client", 2000, "connection refused");
        assert!(!fail.is_success());
        assert_eq!(fail.error_message.as_deref(), Some("connection refused"));
        let display = format!("{fail}");
        assert!(display.contains("[fail]"));
        assert!(display.contains("connection refused"));

        // From impl
        let result: OpenResult = ok.clone().into();
        assert_eq!(result, OpenResult::Handled);
    }

    #[test]
    fn open_attempt_log_filtering() {
        let mut log = OpenAttemptLog::new();
        log.record(OpenAttempt::success("https://a.com", "browser", 100));
        log.record(OpenAttempt::failure("ftp://x.com", "ftp-client", 200, "err"));
        log.record(OpenAttempt::success("https://b.com", "browser", 300));

        assert_eq!(log.len(), 3);
        assert_eq!(log.successes().len(), 2);
        assert_eq!(log.failures().len(), 1);
        assert_eq!(log.by_opener("browser").len(), 2);
        assert_eq!(log.by_opener("ftp-client").len(), 1);

        log.clear();
        assert!(log.is_empty());
    }

    // -------------------------------------------------------------------
    // UriSanitizer tests
    // -------------------------------------------------------------------

    #[test]
    fn uri_sanitizer_blocks_dangerous_schemes() {
        let s = UriSanitizer::new();
        assert!(s.is_blocked("javascript:alert(1)"));
        assert!(s.is_blocked("data:text/html,<h1>hi</h1>"));
        assert!(s.is_blocked("vbscript:foo"));
        assert!(!s.is_blocked("https://example.com"));

        let result = s.sanitize("javascript:alert(1)");
        assert_eq!(result, Err(OpenerError::UnsupportedScheme("javascript".into())));
    }

    #[test]
    fn uri_sanitizer_strips_credentials() {
        let s = UriSanitizer::new();
        let cleaned = s.sanitize("https://user:pass@example.com/path").unwrap();
        assert_eq!(cleaned, "https://example.com/path");
        assert!(!cleaned.contains("user"));
        assert!(!cleaned.contains("pass"));
    }

    #[test]
    fn uri_sanitizer_strips_fragments_and_control_chars() {
        let s = UriSanitizer::new().strip_fragments(true);
        let cleaned = s.sanitize("https://example.com/page#section\x00").unwrap();
        assert_eq!(cleaned, "https://example.com/page");

        let display = format!("{}", UriSanitizer::new());
        assert!(display.contains("javascript"));
    }

    #[test]
    fn uri_sanitizer_custom_blocked_scheme() {
        let mut s = UriSanitizer::new();
        s.block_scheme("ftp");
        assert!(s.is_blocked("ftp://evil.com"));
        assert!(s.sanitize("ftp://evil.com").is_err());
    }

    #[test]
    fn uri_sanitizer_preserves_safe_uri() {
        let s = UriSanitizer::new();
        let uri = "https://example.com/path?q=1#frag";
        let cleaned = s.sanitize(uri).unwrap();
        assert_eq!(cleaned, uri);
    }


    #[test]
    fn priority_manager_basic() {
        let mut pm = OpenerPriorityManager::new();
        pm.set_priority("vscode", 10);
        pm.set_priority("browser", 5);
        assert_eq!(pm.get_priority("vscode"), Some(10));
        assert_eq!(pm.sorted_handlers(), vec!["vscode", "browser"]);
    }

    #[test]
    fn priority_manager_update() {
        let mut pm = OpenerPriorityManager::new();
        pm.set_priority("a", 1);
        pm.set_priority("a", 10);
        assert_eq!(pm.get_priority("a"), Some(10));
        assert_eq!(pm.len(), 1);
    }

    #[test]
    fn scheme_router_basic() {
        let mut r = OpenerSchemeRouter::new();
        r.register_scheme("vscode", "internal");
        r.register_scheme("https", "browser");
        assert_eq!(r.route("https://example.com"), Some("browser"));
        assert_eq!(r.route("vscode://ext/cmd"), Some("internal"));
    }

    #[test]
    fn scheme_router_missing() {
        let r = OpenerSchemeRouter::new();
        assert_eq!(r.route("ftp://x"), None);
    }

    #[test]
    fn scheme_router_remove() {
        let mut r = OpenerSchemeRouter::new();
        r.register_scheme("x", "h");
        assert!(r.remove_scheme("x"));
        assert!(!r.has_scheme("x"));
    }

    #[test]
    fn confirm_dialog_external() {
        let d = OpenerConfirmDialog::for_external_link("https://evil.com");
        assert!(d.is_external);
        assert!(d.message.contains("https://evil.com"));
    }

    #[test]
    fn confirm_dialog_should_confirm() {
        assert!(OpenerConfirmDialog::should_confirm("https://example.com"));
        assert!(!OpenerConfirmDialog::should_confirm("file:///tmp"));
    }

    #[test]
    fn metrics_tracker_basic() {
        let mut m = OpenerMetricsTracker::new();
        m.record_open("https://example.com");
        m.record_open("https://other.com");
        m.record_open("file:///tmp");
        assert_eq!(m.total_opens(), 3);
        assert_eq!(m.opens_for_scheme("https"), 2);
    }

    #[test]
    fn metrics_tracker_failure() {
        let mut m = OpenerMetricsTracker::new();
        m.record_open("https://x");
        m.record_failure();
        assert!((m.success_rate() - 0.5).abs() < 0.01);
    }

    #[test]
    fn metrics_tracker_display() {
        let m = OpenerMetricsTracker::new();
        assert!(format!("{m}").contains("opens=0"));
    }

    #[test]
    fn confirm_dialog_file() {
        let d = OpenerConfirmDialog::for_file("file:///tmp/test");
        assert!(!d.is_external);
    }

    #[test]
    fn metrics_tracker_reset() {
        let mut m = OpenerMetricsTracker::new();
        m.record_open("https://x");
        m.reset();
        assert_eq!(m.total_opens(), 0);
    }


    #[derive(Debug)]
    struct TestHandler {
        scheme_name: String,
    }
    impl ProtocolHandler for TestHandler {
        fn scheme(&self) -> &str { &self.scheme_name }
        fn handle(&self, _uri: &str) -> bool { true }
        fn description(&self) -> &str { "test handler" }
    }

    #[test]
    fn protocol_registry_register_and_dispatch() {
        let mut reg = OpenerProtocolHandlerRegistry::new();
        reg.register(Arc::new(TestHandler { scheme_name: "vscode".into() }));
        assert_eq!(reg.handler_count(), 1);
        let result = reg.dispatch("vscode://open/file");
        assert_eq!(result, OpenResult::Handled);
    }

    #[test]
    fn protocol_registry_unhandled() {
        let mut reg = OpenerProtocolHandlerRegistry::new();
        let result = reg.dispatch("mailto:test@example.com");
        assert_eq!(result, OpenResult::NotHandled);
    }

    #[test]
    fn protocol_registry_unregister() {
        let mut reg = OpenerProtocolHandlerRegistry::new();
        reg.register(Arc::new(TestHandler { scheme_name: "vscode".into() }));
        assert_eq!(reg.unregister_scheme("vscode"), 1);
        assert_eq!(reg.handler_count(), 0);
    }

    #[test]
    fn protocol_registry_registered_schemes() {
        let mut reg = OpenerProtocolHandlerRegistry::new();
        reg.register(Arc::new(TestHandler { scheme_name: "http".into() }));
        reg.register(Arc::new(TestHandler { scheme_name: "https".into() }));
        let schemes = reg.registered_schemes();
        assert!(schemes.contains(&"http".to_string()));
        assert!(schemes.contains(&"https".to_string()));
    }

    #[test]
    fn protocol_registry_has_handler() {
        let mut reg = OpenerProtocolHandlerRegistry::new();
        reg.register(Arc::new(TestHandler { scheme_name: "ftp".into() }));
        assert!(reg.has_handler("ftp"));
        assert!(!reg.has_handler("ssh"));
    }

    #[test]
    fn protocol_registry_dispatch_log() {
        let mut reg = OpenerProtocolHandlerRegistry::new();
        reg.register(Arc::new(TestHandler { scheme_name: "http".into() }));
        reg.dispatch("http://example.com");
        reg.dispatch("ftp://unknown");
        assert_eq!(reg.dispatch_count(), 2);
        assert_eq!(reg.successful_dispatches(), 1);
    }

    #[test]
    fn protocol_registry_display() {
        let reg = OpenerProtocolHandlerRegistry::new();
        let s = format!("{reg}");
        assert!(s.contains("0 handlers"));
    }

    #[test]
    fn external_confirm_trusted_domain() {
        let mut confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::Prompt);
        confirm.trust_domain("github.com");
        let result = confirm.check("https://github.com/repo");
        assert_eq!(result, ConfirmResult::Allowed);
    }

    #[test]
    fn external_confirm_blocked_domain() {
        let mut confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::AlwaysAllow);
        confirm.block_domain("malware.com");
        let result = confirm.check("https://malware.com/bad");
        assert_eq!(result, ConfirmResult::Denied);
    }

    #[test]
    fn external_confirm_prompt_policy() {
        let confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::Prompt);
        let result = confirm.check("https://unknown.com");
        assert!(matches!(result, ConfirmResult::NeedsPrompt { .. }));
    }

    #[test]
    fn external_confirm_scheme_policy() {
        let mut confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::Prompt);
        confirm.set_scheme_policy("mailto", ExternalConfirmPolicy::AlwaysAllow);
        let result = confirm.check("mailto:user@example.com");
        assert_eq!(result, ConfirmResult::Allowed);
    }

    #[test]
    fn external_confirm_untrust() {
        let mut confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::Prompt);
        confirm.trust_domain("github.com");
        assert!(confirm.is_trusted("github.com"));
        assert!(confirm.untrust_domain("github.com"));
        assert!(!confirm.is_trusted("github.com"));
    }

    #[test]
    fn external_confirm_reset() {
        let mut confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::AlwaysAllow);
        confirm.trust_domain("a.com");
        confirm.block_domain("b.com");
        confirm.set_scheme_policy("ftp", ExternalConfirmPolicy::AlwaysDeny);
        confirm.reset();
        assert_eq!(confirm.trusted_count(), 0);
        assert_eq!(confirm.blocked_count(), 0);
        assert_eq!(confirm.scheme_policy_count(), 0);
    }

    #[test]
    fn external_confirm_display() {
        let confirm = OpenerExternalConfirm::new(ExternalConfirmPolicy::Prompt);
        let s = format!("{confirm}");
        assert!(s.contains("Prompt"));
        assert!(s.contains("0 trusted"));
    }



    #[test]
    fn open_lru_insert_get() {
        let mut c = OpenLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn open_lru_eviction() {
        let mut c = OpenLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn open_lru_hit_ratio() {
        let mut c = OpenLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn open_lru_clear() {
        let mut c = OpenLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn open_lru_remove() {
        let mut c = OpenLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn open_lru_peek() {
        let mut c = OpenLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn open_ringbuf_push_get() {
        let mut rb = OpenRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn open_ringbuf_overflow() {
        let mut rb = OpenRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn open_ringbuf_clear() {
        let mut rb = OpenRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn open_ringbuf_newest_oldest() {
        let mut rb = OpenRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn open_ringbuf_to_vec() {
        let mut rb = OpenRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn open_ringbuf_is_full() {
        let mut rb = OpenRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }


    #[test]
    fn opener_config_new() {
        let cfg = OpenerConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn opener_config_set_get() {
        let mut cfg = OpenerConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn opener_config_remove() {
        let mut cfg = OpenerConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn opener_config_keys_sorted() {
        let mut cfg = OpenerConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn opener_config_bump_version() {
        let mut cfg = OpenerConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn opener_config_clear() {
        let mut cfg = OpenerConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn opener_config_merge() {
        let mut cfg1 = OpenerConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = OpenerConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn opener_config_disable() {
        let mut cfg = OpenerConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn opener_rate_tracker_empty() {
        let rt = OpenerRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn opener_rate_tracker_record() {
        let mut rt = OpenerRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn opener_rate_tracker_prune() {
        let mut rt = OpenerRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn opener_validator_valid() {
        let v = OpenerValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn opener_validator_errors() {
        let mut v = OpenerValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn opener_validator_clear() {
        let mut v = OpenerValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn opener_validator_merge() {
        let mut v1 = OpenerValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = OpenerValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn opener_rate_tracker_clear() {
        let mut rt = OpenerRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for opener
    #[test]
    fn xa_opener_ring_new() {
        let rb = super::XaOpenerRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_opener_ring_push_len() {
        let mut rb = super::XaOpenerRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_opener_ring_wrap() {
        let mut rb = super::XaOpenerRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_opener_ring_mean_empty() {
        let rb = super::XaOpenerRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_opener_ring_mean_values() {
        let mut rb = super::XaOpenerRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_opener_ring_min_max() {
        let mut rb = super::XaOpenerRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_opener_ring_iter() {
        let mut rb = super::XaOpenerRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_opener_counter_new() {
        let c = super::XaOpenerCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_opener_counter_inc() {
        let mut c = super::XaOpenerCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_opener_counter_inc_by() {
        let mut c = super::XaOpenerCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_opener_counter_reset() {
        let mut c = super::XaOpenerCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_opener_counter_clear() {
        let mut c = super::XaOpenerCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_opener_counter_default() {
        let c = super::XaOpenerCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }

}